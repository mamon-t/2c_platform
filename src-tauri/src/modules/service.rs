use chrono::Utc;
use futures::StreamExt;
use mongodb::bson::{self, doc};

use crate::core::{CompanyId, PlatformResult};
use crate::db::MongoClient;

use super::{
    CompanyModule, InstalledModule, ModuleManifest, ModuleStatus,
    COLLECTION_COMPANY_MODULES, COLLECTION_MODULES, CURRENT_API_VERSION,
    already_installed, invalid_manifest, module_not_found, api_version_mismatch,
};

pub struct ModuleService;

impl ModuleService {
    // ── Install ────────────────────────────────────────────

    /// Установить WASM-модуль: загрузить → вызвать get_info() (внешне)
    /// → передать манифест → сохранить в MongoDB.
    ///
    /// wasm_bytes и manifest передаются уже распарсенные извне
    /// (в commands.rs вызывается WasmPlugin::load → get_info).
    pub async fn install(
        db: &MongoClient,
        manifest: ModuleManifest,
        wasm_bytes: Vec<u8>,
        company_id: &CompanyId,
    ) -> PlatformResult<InstalledModule> {
        let modules = db.collection::<InstalledModule>(COLLECTION_MODULES);
        let company_modules = db.collection::<CompanyModule>(COLLECTION_COMPANY_MODULES);

        // Проверяем уникальность code
        let existing = modules
            .count_documents(doc! { "code": &manifest.code })
            .await?;
        if existing > 0 {
            return Err(already_installed(&manifest.code));
        }

        // Проверяем api_version
        if manifest.api_version != CURRENT_API_VERSION {
            return Err(api_version_mismatch(CURRENT_API_VERSION, &manifest.api_version));
        }

        // Валидируем capabilities
        for cap in &manifest.capabilities {
            if !super::VALID_CAPABILITIES.contains(&cap.as_str()) {
                return Err(invalid_manifest(&format!(
                    "Неизвестная capability: '{}'. Допустимые: {}",
                    cap,
                    super::VALID_CAPABILITIES.join(", ")
                )));
            }
        }

        if manifest.code.is_empty() || manifest.name.is_empty() {
            return Err(invalid_manifest("code и name обязательны"));
        }

        let now = Utc::now();
        let module_id = uuid::Uuid::new_v4();

        let manifest_value = serde_json::to_value(&manifest).unwrap_or_default();

        let installed = InstalledModule {
            id: module_id,
            code: manifest.code.clone(),
            name: manifest.name,
            description: manifest.description,
            version: manifest.version,
            author: manifest.author,
            api_version: manifest.api_version,
            capabilities: manifest.capabilities,
            functions: manifest.functions,
            status: ModuleStatus::Enabled,
            wasm_bytes,
            manifest: manifest_value,
            installed_at: now,
            updated_at: now,
        };

        modules.insert_one(&installed).await?;

        // Автоматически привязываем к компании (включён)
        let company_module = CompanyModule {
            id: uuid::Uuid::new_v4(),
            company_id: company_id.0.to_string(),
            module_id: module_id.to_string(),
            enabled: true,
            settings: serde_json::json!({}),
            installed_at: now,
        };

        company_modules.insert_one(&company_module).await?;

        tracing::info!(
            "[Module installed] {} v{} — capabilities: [{}]",
            installed.code,
            installed.version,
            installed.capabilities.join(", ")
        );

        Ok(installed)
    }

    // ── Uninstall ──────────────────────────────────────────

    pub async fn uninstall(
        db: &MongoClient,
        module_id: &str,
        company_id: &CompanyId,
    ) -> PlatformResult<()> {
        let modules = db.collection::<InstalledModule>(COLLECTION_MODULES);
        let company_modules = db.collection::<CompanyModule>(COLLECTION_COMPANY_MODULES);

        let id = uuid::Uuid::parse_str(module_id)
            .map_err(|_| module_not_found(module_id))?;

        // Проверяем существование
        let module = modules
            .find_one(doc! { "_id": id.to_string() })
            .await?
            .ok_or_else(|| module_not_found(module_id))?;

        // Удаляем привязки company_modules
        company_modules
            .delete_many(doc! { "module_id": module_id })
            .await?;

        // Удаляем сам модуль
        modules
            .delete_one(doc! { "_id": id.to_string() })
            .await?;

        tracing::info!("[Module uninstalled] {} v{}", module.code, module.version);
        Ok(())
    }

    // ── Enable / Disable ───────────────────────────────────

    pub async fn enable(
        db: &MongoClient,
        module_id: &str,
        company_id: &CompanyId,
    ) -> PlatformResult<()> {
        let company_modules = db.collection::<CompanyModule>(COLLECTION_COMPANY_MODULES);

        let result = company_modules
            .update_one(
                doc! {
                    "module_id": module_id,
                    "company_id": &company_id.0.to_string(),
                },
                doc! { "$set": { "enabled": true } },
            )
            .await?;

        if result.matched_count == 0 {
            // Создаём запись если нет
            let cm = CompanyModule {
                id: uuid::Uuid::new_v4(),
                company_id: company_id.0.to_string(),
                module_id: module_id.to_string(),
                enabled: true,
                settings: serde_json::json!({}),
                installed_at: Utc::now(),
            };
            company_modules.insert_one(&cm).await?;
        }

        tracing::info!("[Module enabled] {} for company {}", module_id, &company_id.0);
        Ok(())
    }

    pub async fn disable(
        db: &MongoClient,
        module_id: &str,
        company_id: &CompanyId,
    ) -> PlatformResult<()> {
        let company_modules = db.collection::<CompanyModule>(COLLECTION_COMPANY_MODULES);

        company_modules
            .update_one(
                doc! {
                    "module_id": module_id,
                    "company_id": &company_id.0.to_string(),
                },
                doc! { "$set": { "enabled": false } },
            )
            .await?;

        tracing::info!("[Module disabled] {} for company {}", module_id, &company_id.0);
        Ok(())
    }

    // ── Read ───────────────────────────────────────────────

    pub async fn get(db: &MongoClient, module_id: &str) -> PlatformResult<InstalledModule> {
        let modules = db.collection::<InstalledModule>(COLLECTION_MODULES);

        let id = uuid::Uuid::parse_str(module_id)
            .map_err(|_| module_not_found(module_id))?;

        modules
            .find_one(doc! { "_id": id.to_string() })
            .await?
            .ok_or_else(|| module_not_found(module_id))
    }

    pub async fn get_by_code(db: &MongoClient, code: &str) -> PlatformResult<InstalledModule> {
        let modules = db.collection::<InstalledModule>(COLLECTION_MODULES);

        modules
            .find_one(doc! { "code": code })
            .await?
            .ok_or_else(|| module_not_found(code))
    }

    pub async fn list(db: &MongoClient, company_id: &CompanyId) -> PlatformResult<Vec<InstalledModule>> {
        let modules = db.collection::<InstalledModule>(COLLECTION_MODULES);
        let company_modules = db.collection::<CompanyModule>(COLLECTION_COMPANY_MODULES);

        // Получаем все модули
        let mut cursor = modules.find(doc! {}).await?;
        let mut all_modules: Vec<InstalledModule> = Vec::new();
        while let Some(result) = cursor.next().await {
            all_modules.push(result?);
        }

        // Получаем привязки для компании
        let company_id_str = company_id.0.to_string();
        let mut cm_cursor = company_modules
            .find(doc! { "company_id": &company_id_str })
            .await?;
        let mut company_bindings: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
        while let Some(cm) = cm_cursor.next().await {
            let cm = cm?;
            company_bindings.insert(cm.module_id, cm.enabled);
        }

        // Мержим: статус из company_modules, если нет привязки — disabled
        for module in &mut all_modules {
            if let Some(&enabled) = company_bindings.get(&module.id.to_string()) {
                module.status = if enabled { ModuleStatus::Enabled } else { ModuleStatus::Disabled };
            } else {
                module.status = ModuleStatus::Disabled;
            }
        }

        Ok(all_modules)
    }

    /// Получить список enabled модулей для компании.
    /// Используется при старте приложения для загрузки в память.
    pub async fn list_enabled(
        db: &MongoClient,
        company_id: &CompanyId,
    ) -> PlatformResult<Vec<InstalledModule>> {
        let all = Self::list(db, company_id).await?;
        Ok(all.into_iter().filter(|m| m.status == ModuleStatus::Enabled).collect())
    }

    // ── Settings ───────────────────────────────────────────

    pub async fn update_settings(
        db: &MongoClient,
        module_id: &str,
        company_id: &CompanyId,
        settings: serde_json::Value,
    ) -> PlatformResult<()> {
        let company_modules = db.collection::<CompanyModule>(COLLECTION_COMPANY_MODULES);

        company_modules
            .update_one(
                doc! {
                    "module_id": module_id,
                    "company_id": &company_id.0.to_string(),
                },
                doc! { "$set": { "settings": bson::to_bson(&settings).map_err(|e| {
                    crate::core::PlatformError::Internal(format!("Ошибка сериализации settings: {}", e))
                })? } },
            )
            .await?;

        Ok(())
    }
}
