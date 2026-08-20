use super::{ModuleInfo, WasmPlugin};
use crate::commands::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn wasm_load(
    wasm_bytes: Vec<u8>,
    name: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<ModuleInfo, String> {
    let state_arc = Arc::new(tokio::sync::Mutex::new({
        let s = state.lock().await;
        AppState {
            db: s.db.clone(),
            auth: crate::auth::AuthService::new("2c-platform-dev-secret-key-change-in-production"),
            config: s.config.clone(),
            current_user: s.current_user.clone(),
            current_company_id: s.current_company_id.clone(),
            current_role_id: s.current_role_id.clone(),
            wasm_modules: None,
        }
    }));

    let plugin = WasmPlugin::load(wasm_bytes, name, state_arc)?;
    let info = plugin.info.clone();

    let mut s = state.lock().await;
    if s.wasm_modules.is_none() {
        s.wasm_modules = Some(HashMap::new());
    }
    s.wasm_modules.as_mut().unwrap().insert(info.id.clone(), plugin);

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
    Ok(s.wasm_modules.as_ref()
        .map(|m| m.values().map(|p| p.info.clone()).collect())
        .unwrap_or_default())
}
