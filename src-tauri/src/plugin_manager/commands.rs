// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use super::{HostData, ModuleInfo, PluginContext, WasmPlugin};
use crate::commands::AppState;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, RwLock};
use tauri::State;
use tokio::sync::Mutex;


/// Все активные роли пользователя в компании (из профилей).
pub(crate) async fn load_user_role_ids(
    db: &crate::db::MongoClient,
    company_id: &str,
    user_id: &str,
) -> Vec<String> {
    let Ok(mut cursor) = db
        .collection::<mongodb::bson::Document>("user_company_profiles")
        .find(mongodb::bson::doc! {
            "company_id": company_id,
            "user_id": user_id,
            "is_active": true,
        })
        .await
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while let Some(Ok(p)) = cursor.next().await {
        if let Ok(r) = p.get_str("role_id") {
            out.push(r.to_string());
        }
    }
    out
}

const PLUGIN_TIMEOUT_MS: u64 = 30_000;

#[tauri::command]
pub async fn wasm_load(
    wasm_bytes: Vec<u8>,
    name: String,
    capabilities: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<ModuleInfo, String> {
    let (ctx, db) = {
        let s = state.lock().await;
        let ctx = Arc::new(RwLock::new(PluginContext {
            company_id: s.current_company_id.clone(),
            user_id: s.current_user.as_ref().map(|u| u._id.to_string()),
            user_login: s.current_user.as_ref().map(|u| u.login.clone()),
            display_name: s.current_user.as_ref().map(|u| u.display_name.clone()),
            role_id: s.current_role_id.clone(),
            role_ids: Vec::new(),
        }));
        // Роли загрузятся при каждом вызове (plugin_call refresh)
        (ctx, s.db.clone())
    };

    let host_data = HostData {
        db,
        ctx: ctx.clone(),
        module_code: Some(name.clone()),
        capabilities: capabilities.clone(),
    };

    let wasm_plugin = WasmPlugin::load(wasm_bytes, name, host_data).await?;
    let info = wasm_plugin.info.clone();
    let plugin_arc = Arc::new(StdMutex::new(wasm_plugin));

    let mut s = state.lock().await;
    if s.wasm_modules.is_none() {
        s.wasm_modules = Some(HashMap::new());
    }
    s.wasm_modules.as_mut().unwrap().insert(info.id.clone(), plugin_arc);

    Ok(info)
}

#[tauri::command]
pub async fn wasm_unload(
    module_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let mut s = state.lock().await;
    if let Some(ref mut modules) = s.wasm_modules {
        modules.remove(&module_id).ok_or_else(|| format!("Модуль {} не найден", module_id))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn wasm_list(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<ModuleInfo>, String> {
    let s = state.lock().await;
    Ok(s.wasm_modules
        .as_ref()
        .map(|m| m.values().map(|p| p.lock().unwrap().info.clone()).collect())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn plugin_call(
    module_id: String,
    function: String,
    args_json: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    invoke_plugin(&state, &module_id, &function, args_json).await
}

/// Внутренний вызов функции загруженного WASM-модуля (для делегирования
/// post_object/cancel_object оркестраторам).
pub(crate) async fn invoke_plugin(
    state: &Mutex<AppState>,
    module_id: &str,
    function: &str,
    args_json: String,
) -> Result<String, String> {
    let module_id = module_id.to_string();
    let function = function.to_string();

    // Ленивая загрузка: если модуля нет в кэше (например, после перезапуска
    // приложения) — загрузить из БД по коду или id.
    {
        let cached = {
            let s = state.lock().await;
            s.wasm_modules.as_ref().map(|m| m.contains_key(&module_id)).unwrap_or(false)
        };
        if !cached {
            let (db, company) = {
                let s = state.lock().await;
                (s.db.clone().ok_or("Нет подключения к БД")?, s.current_company_id.clone())
            };
            let inst = match crate::modules::service::ModuleService::get_by_code(&db, &module_id).await {
                Ok(m) => m,
                Err(_) => crate::modules::service::ModuleService::get(&db, &module_id)
                    .await
                    .map_err(|e| format!("Модуль {module_id} не найден: {e}"))?,
            };
            if let Some(cid) = &company {
                let cid = crate::core::CompanyId(
                    uuid::Uuid::parse_str(cid).map_err(|e| format!("Невалидная компания: {e}"))?,
                );
                let enabled = crate::modules::service::ModuleService::list_enabled(&db, &cid)
                    .await
                    .map(|list| list.iter().any(|m| m.id == inst.id || m.code == inst.code))
                    .unwrap_or(false);
                if !enabled {
                    return Err(format!("Модуль {} отключён для компании", inst.code));
                }
            }
            let ctx = Arc::new(RwLock::new(PluginContext {
                company_id: company.clone(),
                user_id: None,
                user_login: None,
                display_name: None,
                role_id: None,
                role_ids: Vec::new(),
            }));
            let host_data = HostData {
                db: Some(db),
                ctx,
                module_code: Some(inst.code.clone()),
                capabilities: inst.capabilities.clone(),
            };
            let plugin = WasmPlugin::load(inst.wasm_bytes, inst.code.clone(), host_data).await?;
            let arc = Arc::new(StdMutex::new(plugin));
            tracing::info!("[Lazy-load] WASM модуль {} загружен из БД", inst.code);
            let mut s = state.lock().await;
            let map = s.wasm_modules.get_or_insert_with(HashMap::new);
            map.insert(inst.code.clone(), arc.clone());
            map.insert(inst.id.to_string(), arc);
        }
    }

    let (plugin_arc, fresh_company, fresh_user_id, fresh_login, fresh_display, fresh_role, db) = {
        let s = state.lock().await;
        let modules = s.wasm_modules.as_ref().ok_or("Нет загруженных WASM-модулей")?;
        let arc = modules.get(&module_id)
            .ok_or_else(|| format!("Модуль {} не найден", module_id))?
            .clone();
        let company = s.current_company_id.clone();
        let uid = s.current_user.as_ref().map(|u| u._id.to_string());
        let login = s.current_user.as_ref().map(|u| u.login.clone());
        let display = s.current_user.as_ref().map(|u| u.display_name.clone());
        let role = s.current_role_id.clone();
        let db = s.db.clone();
        (arc, company, uid, login, display, role, db)
    };

    let fresh_roles = match (&fresh_company, &fresh_user_id, &db) {
        (Some(c), Some(u), Some(d)) => load_user_role_ids(d, c, u).await,
        _ => Vec::new(),
    };

    {
        let mut plugin = plugin_arc.lock().unwrap();
        plugin.update_context(fresh_company.clone(), fresh_user_id.clone(), fresh_login.clone(), fresh_display.clone(), fresh_role.clone(), fresh_roles.clone());
        let mut ctx = plugin.ctx.write().unwrap();
        ctx.company_id = fresh_company;
        ctx.user_id = fresh_user_id;
        ctx.user_login = fresh_login;
        ctx.display_name = fresh_display;
        ctx.role_id = fresh_role;
        ctx.role_ids = fresh_roles;
    }

    let function_clone = function.clone();
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(PLUGIN_TIMEOUT_MS),
        tokio::task::spawn_blocking(move || {
            let mut plugin = plugin_arc.lock().map_err(|e| format!("Ошибка блокировки плагина: {}", e))?;
            let output = plugin.call(&function_clone, args_json.as_bytes())?;
            String::from_utf8(output).map_err(|e| format!("UTF-8 ошибка: {}", e))
        })
    ).await;

    match result {
        Ok(Ok(Ok(output))) => Ok(output),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(join_err)) => Err(format!("Паника в плагине: {}", join_err)),
        Err(_) => Err(format!("Плагин '{}' таймаут {}ms", function, PLUGIN_TIMEOUT_MS)),
    }
}

// ── Pre-load всех активных модулей компании после логина ──────

#[derive(serde::Serialize)]
pub struct PreloadResult {
    pub loaded: u32,
    pub errors: Vec<PreloadError>,
    pub elapsed_ms: u64,
}

#[derive(serde::Serialize)]
pub struct PreloadError {
    pub code: String,
    pub error: String,
}

#[tauri::command]
pub async fn preload_company_modules(
    state: State<'_, Mutex<AppState>>,
) -> Result<PreloadResult, String> {
    let t0 = std::time::Instant::now();
    let (db, company_id) = {
        let s = state.lock().await;
        let db = s.db.clone().ok_or("Нет подключения к БД")?;
        let cid = s.current_company_id.clone().ok_or("Не выбрана компания")?;
        (db, cid)
    };

    let cid = crate::core::CompanyId(
        uuid::Uuid::parse_str(&company_id).map_err(|e| format!("Невалидный company_id: {e}"))?,
    );
    let enabled = crate::modules::service::ModuleService::list_enabled(&db, &cid)
        .await
        .map_err(|e| format!("Ошибка загрузки списка модулей: {e}"))?;

    let mut loaded = 0u32;
    let mut errors = Vec::new();

    for inst in enabled {
        // Пропустить уже загруженные
        {
            let s = state.lock().await;
            if s.wasm_modules.as_ref().map_or(false, |m| m.contains_key(&inst.code)) {
                loaded += 1;
                continue;
            }
        }

        let ctx = Arc::new(RwLock::new(PluginContext {
            company_id: Some(company_id.clone()),
            user_id: None,
            user_login: None,
            display_name: None,
            role_id: None,
            role_ids: Vec::new(),
        }));
        let host_data = HostData {
            db: Some(db.clone()),
            ctx,
            module_code: Some(inst.code.clone()),
            capabilities: inst.capabilities.clone(),
        };

        match WasmPlugin::load(inst.wasm_bytes, inst.code.clone(), host_data).await {
            Ok(plugin) => {
                let arc = Arc::new(StdMutex::new(plugin));
                let mut s = state.lock().await;
                let map = s.wasm_modules.get_or_insert_with(HashMap::new);
                map.insert(inst.code.clone(), arc.clone());
                map.insert(inst.id.to_string(), arc);
                loaded += 1;
            }
            Err(e) => {
                tracing::error!(
                    "[Pre-load] Ошибка загрузки модуля {}: {}\nBacktrace:\n{}",
                    inst.code, e, std::backtrace::Backtrace::force_capture()
                );
                errors.push(PreloadError { code: inst.code, error: e });
            }
        }
    }

    let elapsed_ms = t0.elapsed().as_millis() as u64;
    tracing::info!("[Pre-load] Загружено модулей: {loaded}, ошибок: {}, за {elapsed_ms}ms", errors.len());
    Ok(PreloadResult { loaded, errors, elapsed_ms })
}
