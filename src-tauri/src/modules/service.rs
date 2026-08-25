// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use chrono::Utc;
use futures::StreamExt;
use mongodb::bson::{self, doc, Document};

use crate::core::{CompanyId, PlatformResult};
use crate::db::MongoClient;

use super::{
    CompanyModule, InstalledModule, InstalledModuleMeta, ModuleManifest, ModuleStatus,
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
        // Хэш бинарника — ключ локального кэша; сразу греем кэш,
        // чтобы первый вход после установки не качал байты.
        let wasm_hash = Self::sha256_hex(&wasm_bytes);
        Self::put_cache_bytes(&manifest.code, &wasm_hash, &wasm_bytes);

        // ── RBAC-сид: создаём политики из манифеста (permissions: ["subsystem.action"]) ──
        let seeded = Self::seed_permissions(db, &manifest).await;

        let manifest_value = serde_json::to_value(&manifest).unwrap_or_default();

        let installed = InstalledModule {
            id: module_id,
            code: manifest.code.clone(),
            name: manifest.name,
            description: manifest.description,
            version: manifest.version,
            author: manifest.author,
            api_version: manifest.api_version,
            capabilities: manifest.capabilities.clone(),
            functions: manifest.functions.clone(),
            status: ModuleStatus::Enabled,
            wasm_bytes,
            wasm_sha256: Some(wasm_hash),
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
            "[Module installed] {} v{} — capabilities: [{}], permissions seeded: {}",
            installed.code,
            installed.version,
            installed.capabilities.join(", "),
            seeded,
        );

        Ok(installed)
    }

    /// Создать недостающие PermissionPolicy из списка permissions манифеста.
    /// Формат записи: "subsystem.action". Существующие коды не трогаем.
    /// Возвращает количество созданных политик.
    async fn seed_permissions(db: &MongoClient, manifest: &ModuleManifest) -> usize {
        use mongodb::bson::Document;

        let col = db.collection::<Document>("permission_policies");
        let mut created = 0usize;

        for perm in &manifest.permissions {
            let Some((subsystem, action)) = perm.split_once('.') else {
                tracing::warn!(
                    "[Module:{}] Некорректная permission '{}', ожидается 'subsystem.action'",
                    manifest.code,
                    perm
                );
                continue;
            };

            // Пропускаем уже существующие (не перезаписываем настройки админа)
            match col.count_documents(doc! { "code": perm }).await {
                Ok(n) if n > 0 => continue,
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("[Module:{}] Проверка policy '{}': {}", manifest.code, perm, e);
                    continue;
                }
            }

            let now = Utc::now();
            let policy_doc = doc! {
                "_id": uuid::Uuid::new_v4().to_string(),
                "code": perm,
                "name": format!("{} — {}", manifest.name, action),
                "description": bson::Bson::Null,
                "scope_type": "subsystem",
                "subsystem_code": subsystem,
                "entity_type": bson::Bson::Null,
                "actions": [action],
                "record_scope": "company",
                "deny": false,
                "priority": 0,
                "created_at": mongodb::bson::Bson::DateTime(mongodb::bson::DateTime::from_millis(now.timestamp_millis())),
                "updated_at": mongodb::bson::Bson::DateTime(mongodb::bson::DateTime::from_millis(now.timestamp_millis())),
            };

            if let Err(e) = col.insert_one(policy_doc).await {
                tracing::warn!("[Module:{}] Создание policy '{}': {}", manifest.code, perm, e);
            } else {
                created += 1;
            }
        }

        created
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

    // ── Локальный кэш байтов модулей ───────────────────────
    //
    // ~/.cache/2c-platform/modules/{code}-{hash16}.wasm
    // Ключ — sha256 содержимого: повторный вход не тянет бинарники по сети,
    // обновление модуля (новый хэш) докачивает только его.

    pub fn cache_dir() -> std::path::PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("2c-platform")
            .join("modules")
    }

    fn sha256_hex(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(data);
        hex::encode(h.finalize())
    }

    fn cache_path(code: &str, hash: &str) -> std::path::PathBuf {
        let safe: String = code.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        Self::cache_dir().join(format!("{safe}-{}.wasm", &hash[..16.min(hash.len())]))
    }

    /// Записать байты в кэш (tmp + rename) и убрать прочие версии этого кода.
    pub fn put_cache_bytes(code: &str, hash: &str, bytes: &[u8]) {
        let dir = Self::cache_dir();
        if std::fs::create_dir_all(&dir).is_err() { return; }
        let path = Self::cache_path(code, hash);
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
        // Прочие версии кода больше не нужны
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let prefix = format!("{}-", code);
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) && name != *path.file_name().unwrap_or_default().to_string_lossy() {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }

    /// Байты модуля для загрузки WASM: сначала локальный кэш по известному
    /// хэшу, при промахе — разовый fetch из БД + запись в кэш.
    pub async fn get_module_bytes(
        db: &MongoClient,
        meta: &InstalledModuleMeta,
    ) -> PlatformResult<Vec<u8>> {
        // 1. Кэш по известному хэшу
        if let Some(h) = &meta.wasm_sha256 {
            let path = Self::cache_path(&meta.code, h);
            if let Ok(bytes) = std::fs::read(&path) {
                return Ok(bytes);
            }
        }

        // 2. Fetch из БД (только поле байтов)
        let col = db.collection::<Document>(COLLECTION_MODULES);
        let d = col
            .find_one(doc! { "_id": meta.id.to_string() })
            .projection(doc! { "wasm_bytes": 1 })
            .await?
            .ok_or_else(|| module_not_found(&meta.code))?;
        let bytes = d.get_binary_generic("wasm_bytes")
            .map_err(|_| crate::core::PlatformError::NotFound(
                format!("модуль {}: нет wasm_bytes", meta.code)))?
            .to_vec();

        // 3. Хэш → кэш; ленивая миграция поля wasm_sha256
        let h = Self::sha256_hex(&bytes);
        Self::put_cache_bytes(&meta.code, &h, &bytes);
        if meta.wasm_sha256.as_deref() != Some(h.as_str()) {
            let _ = col.update_one(
                doc! { "_id": meta.id.to_string() },
                doc! { "$set": { "wasm_sha256": &h, "updated_at": mongodb::bson::DateTime::now() } },
            ).await;
        }
        Ok(bytes)
    }

    /// Enabled модули компании БЕЗ бинарников (метаданные + хэш).
    pub async fn list_enabled_meta(
        db: &MongoClient,
        company_id: &CompanyId,
    ) -> PlatformResult<Vec<InstalledModuleMeta>> {
        let modules = db.collection::<InstalledModuleMeta>(COLLECTION_MODULES);
        let company_modules = db.collection::<CompanyModule>(COLLECTION_COMPANY_MODULES);

        let mut cm_cursor = company_modules
            .find(doc! { "company_id": company_id.0.to_string(), "enabled": true })
            .await?;
        let mut ids = Vec::new();
        while let Some(cm) = cm_cursor.next().await {
            ids.push(cm?.module_id);
        }
        if ids.is_empty() { return Ok(Vec::new()); }

        let mut cursor = modules
            .find(doc! { "_id": { "$in": ids } })
            .projection(doc! { "wasm_bytes": 0 })
            .await?;
        let mut out = Vec::new();
        while let Some(m) = cursor.next().await {
            out.push(m?);
        }
        Ok(out)
    }
}
