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
    if !state.check_access("print", None, "read") {
        return Err("Доступ запрещён: нет права print.read".into());
    }
    let db = get_db!(state);
    PrintService::list(db, &entity_type, form_code.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn print_get_template(
    id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<PrintTemplate, String> {
    let state = state.lock().await;
    if !state.check_access("print", None, "read") {
        return Err("Доступ запрещён: нет права print.read".into());
    }
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
    if !state.check_access("print", None, "create") {
        return Err("Доступ запрещён: нет права print.create".into());
    }
    let db = get_db!(state);
    let user_id = state.current_user.as_ref().map(|u| u._id.to_string());
    let actor = crate::commands::build_actor(&state);
    let outcome = PrintService::create(db, input, user_id, actor).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, crate::audit::AuditableAction::CreatePrintTemplate,
        target_id = outcome.result._id.to_string());
    Ok(outcome.result)
}

#[tauri::command]
pub async fn print_update_template(
    id: String,
    input: UpdatePrintTemplateInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<PrintTemplate, String> {
    let state = state.lock().await;
    if !state.check_access("print", None, "update") {
        return Err("Доступ запрещён: нет права print.update".into());
    }
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
    if !state.check_access("print", None, "delete") {
        return Err("Доступ запрещён: нет права print.delete".into());
    }
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let actor = crate::commands::build_actor(&state);
    let _outcome = PrintService::delete(db, uid, actor).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, crate::audit::AuditableAction::DeletePrintTemplate,
        target_id = id);
    Ok(())
}

#[tauri::command]
pub async fn print_render(
    template_id: String,
    object_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let state = state.lock().await;
    if !state.check_access("print", None, "read") {
        return Err("Доступ запрещён: нет права print.read".into());
    }
    let db = get_db!(state);
    let tid = uuid::Uuid::parse_str(&template_id).map_err(|e| e.to_string())?;
    let oid = uuid::Uuid::parse_str(&object_id).map_err(|e| e.to_string())?;
    PrintService::render(db, tid, oid).await.map_err(|e| e.to_string())
}
