use super::{HostData, ModuleInfo, WasmPlugin};
use crate::commands::AppState;
use std::collections::HashMap;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn wasm_load(
    wasm_bytes: Vec<u8>,
    name: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<ModuleInfo, String> {
    // 1. Безопасно ждем лок в async-контексте
    let s = state.lock().await;
    
    // 2. Собираем легкий HostData. 
    // s.db.clone() дешевый (Arc под капотом), новых соединений НЕ создает.
    let host_data = HostData {
        db: s.db.clone(), // <-- Просто берем то, что уже есть в AppState
        company_id: s.current_company_id.clone(),
        user_id: s.current_user.as_ref().map(|u| u._id.to_string()),
        user_login: s.current_user.as_ref().map(|u| u.login.clone()),
        display_name: s.current_user.as_ref().map(|u| u.display_name.clone()),
    };

    // 3. Загружаем плагин синхронно, передавая готовые данные
    let mut plugin = WasmPlugin::load(wasm_bytes, name, host_data)?;
    let info = plugin.info.clone();

    // 4. Кладем плагин в общий стейт
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
    Ok(s.wasm_modules
        .as_ref()
        .map(|m| m.values().map(|p| p.info.clone()).collect())
        .unwrap_or_default())
}