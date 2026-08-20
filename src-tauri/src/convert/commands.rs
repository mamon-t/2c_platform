use super::{ImportRequest, ImportResult, ExportRequest, ExportResult, ModuleInfo, plugin::ConvertPlugin};
use crate::commands::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn load_wasm_module(
    path: String,
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

    let convert_plugin = ConvertPlugin::load(&path, state_arc)?;
    let info = convert_plugin.info.clone();

    let mut s = state.lock().await;
    if s.wasm_modules.is_none() {
        s.wasm_modules = Some(HashMap::new());
    }
    s.wasm_modules.as_mut().unwrap().insert(info.id.clone(), convert_plugin);

    Ok(info)
}

#[tauri::command]
pub async fn unload_wasm_module(
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
pub async fn list_wasm_modules(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<ModuleInfo>, String> {
    let s = state.lock().await;
    Ok(s.wasm_modules.as_ref().map(|m| m.values().map(|p| p.info.clone()).collect()).unwrap_or_default())
}

#[tauri::command]
pub async fn import_objects_via_wasm(
    module_id: String,
    file: Vec<u8>,
    filename: String,
    entity_type_id: String,
    format: String,
    mapping: Option<HashMap<String, String>>,
    state: State<'_, Mutex<AppState>>,
) -> Result<ImportResult, String> {
    let mut s = state.lock().await;
    let modules = s.wasm_modules.as_mut().ok_or("No WASM modules loaded")?;
    let plugin = modules.get_mut(&module_id).ok_or_else(|| format!("Module {} not found", module_id))?;

    let _ = filename;
    let req = ImportRequest {
        format,
        file_data: file,
        entity_type_id,
        mapping,
    };

    plugin.import_data(&req)
}

#[tauri::command]
pub async fn export_objects_via_wasm(
    module_id: String,
    entity_type_id: String,
    format: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<ExportResult, String> {
    let (db_clone, company_id_str) = {
        let s = state.lock().await;
        let db = s.db.as_ref().ok_or("No database connection")?.clone();
        let cid = s.current_company_id.clone().ok_or("No company selected")?;
        (db, cid)
    };

    let cid = uuid::Uuid::parse_str(&company_id_str).map_err(|_| "Invalid company_id".to_string())?;
    let company_id = crate::core::CompanyId(cid);

    let page = crate::objects::service::ObjectService::list(
        &db_clone,
        company_id,
        crate::objects::ObjectFilters {
            entity_type_id: Some(entity_type_id.clone()),
            state: None,
            parent_id: None,
            search: None,
            limit: Some(200),
            offset: Some(0),
        },
    ).await.map_err(|e| e.to_string())?;

    let req = ExportRequest {
        entity_type_id,
        format,
        objects: page.objects.into_iter().map(|o| {
            serde_json::json!({
                "id": o._id.to_string(),
                "number": o.number,
                "state": format!("{:?}", o.state).to_lowercase(),
                "version": o.version,
                "data": o.data,
            })
        }).collect(),
    };

    let mut s = state.lock().await;
    let modules = s.wasm_modules.as_mut().ok_or("No WASM modules loaded")?;
    let plugin = modules.get_mut(&module_id).ok_or_else(|| format!("Module {} not found", module_id))?;
    plugin.export_data(&req)
}
