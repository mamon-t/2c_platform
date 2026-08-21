use crate::commands::AppState;
use super::{NumberSequence, NumberingService, UpdateNumberFormatInput};
use crate::core::CompanyId;
use tauri::State;
use tokio::sync::Mutex;

macro_rules! get_db {
    ($state:expr) => {
        $state.db.as_ref().ok_or_else(|| "Не подключено к MongoDB".to_string())?
    };
}

#[tauri::command]
pub async fn numbering_list(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<NumberSequence>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let company_id = state.current_company_id.as_ref()
        .ok_or("Не выбрана компания")?;
    let cid = CompanyId(uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?);
    NumberingService::list_sequences(db, &cid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn numbering_get(
    entity_type_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<NumberSequence>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let company_id = state.current_company_id.as_ref()
        .ok_or("Не выбрана компания")?;
    let cid = CompanyId(uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?);
    NumberingService::get_sequence(db, &cid, &entity_type_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn numbering_update_format(
    entity_type_id: String,
    entity_type_name: String,
    input: UpdateNumberFormatInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<NumberSequence, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let company_id = state.current_company_id.as_ref()
        .ok_or("Не выбрана компания")?;
    let cid = CompanyId(uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?);
    NumberingService::update_format(db, &cid, &entity_type_id, &entity_type_name, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn numbering_reset(
    entity_type_id: String,
    new_value: Option<i64>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let company_id = state.current_company_id.as_ref()
        .ok_or("Не выбрана компания")?;
    let cid = CompanyId(uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?);
    NumberingService::reset_sequence(db, &cid, &entity_type_id, new_value).await.map_err(|e| e.to_string())
}
