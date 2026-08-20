use super::{ImportRequest, ImportResult, ExportRequest, ExportResult};
use crate::commands::AppState;
use std::collections::HashMap;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn convert_import(
    module_id: String,
    file: Vec<u8>,
    filename: String,
    entity_type_id: String,
    format: String,
    mapping: Option<HashMap<String, String>>,
    state: State<'_, Mutex<AppState>>,
) -> Result<ImportResult, String> {
    let req = ImportRequest {
        format,
        file_data: file,
        entity_type_id,
        mapping,
    };
    let input = serde_json::to_vec(&req).map_err(|e| format!("Serialize error: {}", e))?;
    let _ = filename;

    let mut s = state.lock().await;
    let modules = s.wasm_modules.as_mut().ok_or("No WASM modules loaded")?;
    let plugin = modules.get_mut(&module_id)
        .ok_or_else(|| format!("Module {} not found", module_id))?;

    let output = plugin.call("import_data", &input)?;
    serde_json::from_slice(&output).map_err(|e| format!("Deserialize error: {}", e))
}

#[tauri::command]
pub async fn convert_export(
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
    let input = serde_json::to_vec(&req).map_err(|e| format!("Serialize error: {}", e))?;

    let mut s = state.lock().await;
    let modules = s.wasm_modules.as_mut().ok_or("No WASM modules loaded")?;
    let plugin = modules.get_mut(&module_id)
        .ok_or_else(|| format!("Module {} not found", module_id))?;

    let output = plugin.call("export_data", &input)?;
    serde_json::from_slice(&output).map_err(|e| format!("Deserialize error: {}", e))
}
