// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::CompanyId;
use crate::db::MongoClient;

use super::{InstalledModule, ModuleManifest, service::ModuleService, InstallModuleInput};

// ── Helpers ────────────────────────────────────────────────

fn get_db(state: &AppState) -> Result<&MongoClient, String> {
    state.db.as_ref().ok_or_else(|| "База данных не подключена".into())
}


/// Выгрузить модуль из кэша WASM (по id и по коду).
/// Без этого удалённый/отключённый модуль продолжал бы работать до перезапуска.
fn evict_cached_module(state: &mut AppState, keys: &[&str]) {
    if let Some(map) = state.wasm_modules.as_mut() {
        for k in keys {
            map.remove(*k);
        }
        // выгрузить и записи, чей внутренний id совпадает (двойное кэширование code+uuid)
        let stale: Vec<String> = map
            .iter()
            .filter(|(_, p)| p.lock().map(|p| keys.contains(&p.info.id.as_str())).unwrap_or(false))
            .map(|(k, _)| k.clone())
            .collect();
        for k in stale {
            map.remove(&k);
        }
    }
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

    // Валидация WASM: полная загрузка со всеми host-fn
    // (модуль импортирует их на уровне линковки), затем get_info()
    let plugin_ctx = std::sync::Arc::new(std::sync::RwLock::new(
        crate::plugin_manager::PluginContext {
            company_id: Some(company_id.0.to_string()),
            user_id: None,
            user_login: None,
            display_name: None,
            role_id: None,
            role_ids: Vec::new(),
        },
    ));
    let host_data = crate::plugin_manager::HostData {
        db: Some(db.clone()),
        ctx: plugin_ctx,
        module_code: None,
        capabilities: Vec::new(),
    };
    let mut plugin = crate::plugin_manager::WasmPlugin::load(wasm_bytes.clone(), "install-validate".into(), host_data)
        .await
        .map_err(|e| format!("Невалидный WASM-модуль: {e}"))?;

    let info_bytes = plugin
        .call("get_info", b"")
        .map_err(|e| format!("Модуль не экспортирует get_info(): {e}"))?;
    let info_json = String::from_utf8(info_bytes)
        .map_err(|e| format!("get_info() вернул не-UTF8: {e}"))?;

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

    let installed = ModuleService::install(&db, manifest, wasm_bytes.clone(), &company_id)
        .await
        .map_err(|e| e.to_string())?;

    // ── «Установил → сразу использует»: модуль в память текущей сессии ──
    // Отдельная загрузка с РЕАЛЬНЫМИ capabilities: валидационный экземпляр
    // выше собран с пустыми, а host-fn захватывают HostData при билде.
    {
        let plugin_ctx = Arc::new(std::sync::RwLock::new(
            crate::plugin_manager::PluginContext {
                company_id: Some(company_id.0.to_string()),
                ..Default::default()
            },
        ));
        let host_data = crate::plugin_manager::HostData {
            db: Some(db.clone()),
            ctx: plugin_ctx,
            module_code: Some(installed.code.clone()),
            capabilities: installed.capabilities.clone(),
        };
        match crate::plugin_manager::WasmPlugin::load(wasm_bytes, installed.code.clone(), host_data).await {
            Ok(plugin) => {
                let arc = Arc::new(StdMutex::new(plugin));
                let mut s = state.lock().await;
                let map = s.wasm_modules.get_or_insert_with(HashMap::new);
                // Защитно: ключ кода не должен был пережить установку
                // (повторный install того же кода отклонён бы ранее)
                map.remove(&installed.code);
                map.insert(installed.code.clone(), arc.clone());
                map.insert(installed.id.to_string(), arc);
                tracing::info!("[Module installed] {} доступен сразу (без перезапуска)", installed.code);
            }
            Err(e) => {
                tracing::warn!(
                    "[Module installed] {} установлен, но не загружен в сессию: {e}. Подхватится при следующем входе",
                    installed.code
                );
            }
        }
    }

    Ok(installed)
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
    let code = ModuleService::get(&db, &module_id).await.ok().map(|m| m.code);
    ModuleService::uninstall(&db, &module_id, &company_id)
        .await
        .map_err(|e| e.to_string())?;
    let mut s = state.lock().await;
    match &code {
        Some(c) => evict_cached_module(&mut s, &[module_id.as_str(), c.as_str()]),
        None => evict_cached_module(&mut s, &[module_id.as_str()]),
    }
    Ok(())
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
    let code = ModuleService::get(&db, &module_id).await.ok().map(|m| m.code);
    ModuleService::disable(&db, &module_id, &company_id)
        .await
        .map_err(|e| e.to_string())?;
    let mut s = state.lock().await;
    match &code {
        Some(c) => evict_cached_module(&mut s, &[module_id.as_str(), c.as_str()]),
        None => evict_cached_module(&mut s, &[module_id.as_str()]),
    }
    Ok(())
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
