use super::{HostData, ModuleInfo, PluginContext, WasmPlugin};
use crate::commands::AppState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, RwLock};
use tauri::State;
use tokio::sync::Mutex;

const PLUGIN_TIMEOUT_MS: u64 = 30_000;

#[tauri::command]
pub async fn wasm_load(
    wasm_bytes: Vec<u8>,
    name: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<ModuleInfo, String> {
    let ctx = {
        let s = state.lock().await;
        Arc::new(RwLock::new(PluginContext {
            company_id: s.current_company_id.clone(),
            user_id: s.current_user.as_ref().map(|u| u._id.to_string()),
            user_login: s.current_user.as_ref().map(|u| u.login.clone()),
            display_name: s.current_user.as_ref().map(|u| u.display_name.clone()),
        }))
    };

    let host_data = HostData {
        db: {
            let s = state.lock().await;
            s.db.clone()
        },
        ctx: ctx.clone(),
    };

    let wasm_plugin = WasmPlugin::load(wasm_bytes, name, host_data)?;
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
        modules.remove(&module_id).ok_or_else(|| format!("Module {} not found", module_id))?;
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
    let (plugin_arc, fresh_company, fresh_user_id, fresh_login, fresh_display) = {
        let s = state.lock().await;
        let modules = s.wasm_modules.as_ref().ok_or("No WASM modules loaded")?;
        let arc = modules.get(&module_id)
            .ok_or_else(|| format!("Module {} not found", module_id))?
            .clone();
        let company = s.current_company_id.clone();
        let uid = s.current_user.as_ref().map(|u| u._id.to_string());
        let login = s.current_user.as_ref().map(|u| u.login.clone());
        let display = s.current_user.as_ref().map(|u| u.display_name.clone());
        (arc, company, uid, login, display)
    };

    {
        let mut plugin = plugin_arc.lock().unwrap();
        plugin.update_context(fresh_company.clone(), fresh_user_id.clone(), fresh_login.clone(), fresh_display.clone());
        let mut ctx = plugin.ctx.write().unwrap();
        ctx.company_id = fresh_company;
        ctx.user_id = fresh_user_id;
        ctx.user_login = fresh_login;
        ctx.display_name = fresh_display;
    }

    let function_clone = function.clone();
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(PLUGIN_TIMEOUT_MS),
        tokio::task::spawn_blocking(move || {
            let mut plugin = plugin_arc.lock().map_err(|e| format!("Plugin lock poisoned: {}", e))?;
            let output = plugin.call(&function_clone, args_json.as_bytes())?;
            String::from_utf8(output).map_err(|e| format!("UTF-8 decode error: {}", e))
        })
    ).await;

    match result {
        Ok(Ok(Ok(output))) => Ok(output),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(join_err)) => Err(format!("Plugin task panicked: {}", join_err)),
        Err(_) => Err(format!("Plugin '{}' timed out after {}ms", function, PLUGIN_TIMEOUT_MS)),
    }
}
