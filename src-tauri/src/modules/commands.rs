use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::{CompanyId, PlatformResult};
use crate::db::MongoClient;

use super::{InstalledModule, ModuleManifest, service::ModuleService, InstallModuleInput};

// ── Helpers ────────────────────────────────────────────────

fn get_db(state: &AppState) -> Result<&MongoClient, String> {
    state.db.as_ref().ok_or_else(|| "База данных не подключена".into())
}

fn get_company_id(state: &AppState) -> Result<CompanyId, String> {
    state.current_company_id.as_ref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(CompanyId)
        .ok_or_else(|| "Не выбрана компания".into())
}

// ── Commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn modules_list(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<InstalledModule>, String> {
    let (db, company_id) = {
        let s = state.lock().await;
        (get_db(&s)?.clone(), get_company_id(&s)?)
    };
    ModuleService::list(&db, &company_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn modules_get(
    module_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<InstalledModule, String> {
    let db = {
        let s = state.lock().await;
        get_db(&s)?.clone()
    };
    ModuleService::get(&db, &module_id).await.map_err(|e| e.to_string())
}

/// Установить WASM-модуль.
/// 1. Загружаем WASM через Extism → вызываем get_info() → парсим манифест
/// 2. Сохраняем в MongoDB (modules + company_modules)
#[tauri::command]
pub async fn modules_install(
    input: InstallModuleInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<InstalledModule, String> {
    let (db, company_id) = {
        let s = state.lock().await;
        (get_db(&s)?.clone(), get_company_id(&s)?)
    };

    let wasm_bytes = input.wasm_bytes;

    // Валидация WASM: загружаем минимально и вызываем get_info()
    let mut extism_manifest = extism::Manifest::new([extism::Wasm::data(wasm_bytes.clone())]);
    extism_manifest.timeout_ms = Some(10_000);
    extism_manifest.memory.max_pages = Some(256);

    let mut plugin = extism::PluginBuilder::new(&extism_manifest)
        .with_fuel_limit(10_000)
        .build()
        .map_err(|e| format!("Невалидный WASM-модуль: {}", e))?;

    let info_json = plugin.call::<&[u8], String>("get_info", b"")
        .map_err(|e| format!("Модуль не экспортирует get_info(): {}", e))?;

    let wasm_info: super::super::plugin_manager::WasmModuleInfo = serde_json::from_str(&info_json)
        .map_err(|e| format!("get_info() вернул невалидный JSON: {}", e))?;

    // Собираем манифест ИСКЛЮЧИТЕЛЬНО из get_info() модуля
    // (модуль сам декларирует capabilities и permissions)
    let manifest = ModuleManifest {
        code: wasm_info.code.clone().unwrap_or_else(|| wasm_info.name.clone()),
        name: wasm_info.name.clone(),
        version: wasm_info.version.clone(),
        api_version: wasm_info.api_version.clone()
            .unwrap_or_else(|| crate::modules::CURRENT_API_VERSION.into()),
        author: wasm_info.author.clone().unwrap_or_else(|| "Unknown".into()),
        description: wasm_info.description.clone()
            .unwrap_or_else(|| format!("WASM модуль {} v{}", wasm_info.name, wasm_info.version)),
        capabilities: wasm_info.capabilities.clone(),
        permissions: wasm_info.permissions.clone(),
        handles_documents: wasm_info.handled_documents.clone(),
        functions: wasm_info.functions.into_iter().map(|f| super::ModuleFunction {
            name: f.name,
            label: f.label,
            description: f.description,
            input_schema: f.input_schema,
        }).collect(),
    };

    ModuleService::install(&db, manifest, wasm_bytes, &company_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn modules_uninstall(
    module_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let (db, company_id) = {
        let s = state.lock().await;
        (get_db(&s)?.clone(), get_company_id(&s)?)
    };
    ModuleService::uninstall(&db, &module_id, &company_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn modules_enable(
    module_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let (db, company_id) = {
        let s = state.lock().await;
        (get_db(&s)?.clone(), get_company_id(&s)?)
    };
    ModuleService::enable(&db, &module_id, &company_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn modules_disable(
    module_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let (db, company_id) = {
        let s = state.lock().await;
        (get_db(&s)?.clone(), get_company_id(&s)?)
    };
    ModuleService::disable(&db, &module_id, &company_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn modules_update_settings(
    module_id: String,
    settings: serde_json::Value,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let (db, company_id) = {
        let s = state.lock().await;
        (get_db(&s)?.clone(), get_company_id(&s)?)
    };
    ModuleService::update_settings(&db, &module_id, &company_id, settings)
        .await
        .map_err(|e| e.to_string())
}
