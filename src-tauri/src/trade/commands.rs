//! IPC-команды торговли.

use serde::Deserialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::middleware::CommandContext;
use crate::audit::AuditableAction;
use crate::db::MongoClient;

/// Seed метаданных торговли (settings.manage).
#[tauri::command]
pub async fn trade_seed_metadata(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("settings.manage").map_err(|e| e.to_string())?;
    let db: MongoClient = s.db.as_ref().ok_or("Не подключено к MongoDB")?.clone();
    drop(s);

    let result = super::seed::seed(&db).await?;

    // Индексы после seed (нужны UUID типов)
    super::indexes::ensure_indexes(&db).await;

    Ok(result)
}

#[derive(serde::Serialize)]
pub struct PriceOnDate {
    pub object_id: String,
    pub nomenclature_id: String,
    pub price_type_id: String,
    pub value: f64,
    pub valid_from: String,
}

/// Цена на дату: последняя запись, где valid_from ≤ date
/// и (valid_to пусто или ≥ date). Нативное чтение по частичному индексу.
#[tauri::command]
pub async fn trade_get_price(
    nomenclature_id: String,
    price_type_id: String,
    on_date: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<PriceOnDate>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("trade.read").map_err(|e| e.to_string())?;
    let db = s.db.as_ref().ok_or("Не подключено к MongoDB")?.clone();
    drop(s);

    // Резолвим entity_type_id по коду PRICE
    let et = db.collection::<mongodb::bson::Document>("entity_types")
        .find_one(mongodb::bson::doc! { "code": super::ET_PRICE })
        .await.map_err(|e| e.to_string())?
        .ok_or_else(|| "Тип PRICE не найден".to_string())?;
    let et_id = et.get_str("_id").map_err(|e| e.to_string())?;

    let date = on_date.unwrap_or_else(|| chrono::Utc::now().date_naive().to_string());

    let filter = mongodb::bson::doc! {
        "entity_type_id": &et_id,
        "company_id": ctx.company_id.0.to_string(),
        "state": { "$in": ["draft", "active"] },
        "data.nomenclature_id": &nomenclature_id,
        "data.price_type_id": &price_type_id,
        "data.valid_from": { "$lte": &date },
        "$or": [
            { "data.valid_to": null },
            { "data.valid_to": "" },
            { "data.valid_to": { "$gte": &date } },
        ],
    };

    let rec = db.collection::<mongodb::bson::Document>("objects")
        .find_one(filter)
        .sort(mongodb::bson::doc! { "data.valid_from": -1 })
        .await
        .map_err(|e| e.to_string())?;

    let Some(d) = rec else { return Ok(None) };
    let data = d.get_document("data").map_err(|e| format!("data: {e}"))?;

    Ok(Some(PriceOnDate {
        object_id: d.get_str("_id").unwrap_or("").into(),
        nomenclature_id: data.get_str("nomenclature_id").unwrap_or("").into(),
        price_type_id: data.get_str("price_type_id").unwrap_or("").into(),
        value: data.get_f64("value").unwrap_or(0.0),
        valid_from: data.get_str("valid_from").unwrap_or("").into(),
    }))
}
