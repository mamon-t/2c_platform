use super::*;
use super::service::PrintService;
use crate::commands::AppState;
use tauri::State;
use tokio::sync::Mutex;

macro_rules! get_db {
    ($state:expr) => {
        $state.db.as_ref().ok_or_else(|| "Не подключено к MongoDB".to_string())?
    };
}

#[tauri::command]
pub async fn print_list_templates(
    entity_type: String,
    form_code: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<PrintTemplate>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    PrintService::list(db, &entity_type, form_code.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn print_get_template(
    id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<PrintTemplate, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    PrintService::get(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn print_create_template(
    input: CreatePrintTemplateInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<PrintTemplate, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let user = state.current_user.as_ref().map(|u| u._id.to_string());
    PrintService::create(db, input, user).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn print_update_template(
    id: String,
    input: UpdatePrintTemplateInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<PrintTemplate, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    PrintService::update(db, uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn print_delete_template(
    id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    PrintService::delete(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn print_render(
    template_id: String,
    object_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let tid = uuid::Uuid::parse_str(&template_id).map_err(|e| e.to_string())?;
    let oid = uuid::Uuid::parse_str(&object_id).map_err(|e| e.to_string())?;
    PrintService::render(db, tid, oid).await.map_err(|e| e.to_string())
}
