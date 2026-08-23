use mongodb::bson::doc;
use mongodb::IndexModel;
use tracing::warn;

use crate::db::MongoClient;
use super::{COL_BALANCES, COL_BATCHES, COL_MOVEMENTS};

pub async fn ensure_indexes(db: &MongoClient) {
    let batches = db.collection::<mongodb::bson::Document>(COL_BATCHES);
    let balances = db.collection::<mongodb::bson::Document>(COL_BALANCES);
    let movements = db.collection::<mongodb::bson::Document>(COL_MOVEMENTS);

    // FIFO-выборка живых партий
    if let Err(e) = batches
        .create_index(IndexModel::builder().keys(doc! {
            "company_id": 1, "location_id": 1, "nomenclature_id": 1, "receipt_date": 1,
        }).build())
        .await
    { warn!("stock_batches FIFO: {e}"); }

    // Частичный: только живые партии
    if let Err(e) = batches
        .create_index(IndexModel::builder()
            .keys(doc! { "company_id": 1, "location_id": 1, "nomenclature_id": 1 })
            .options(
                mongodb::options::IndexOptions::builder()
                    .name("partial_active".to_string())
                    .partial_filter_expression(mongodb::bson::doc! { "qty_remaining": { "$gt": 0 } })
                    .build(),
            )
            .build())
        .await
    { warn!("stock_batches partial_active: {e}"); }

    // Уникальный ключ апсерта баланса
    if let Err(e) = balances
        .create_index(IndexModel::builder()
            .keys(doc! { "company_id": 1, "location_id": 1, "nomenclature_id": 1 })
            .options(mongodb::options::IndexOptions::builder().unique(true).build())
            .build())
        .await
    { warn!("stock_balances unique: {e}"); }

    if let Err(e) = balances
        .create_index(IndexModel::builder().keys(doc! { "company_id": 1, "nomenclature_id": 1 }).build())
        .await
    { warn!("stock_balances by_nom: {e}"); }

    // Карточка движения товара
    if let Err(e) = movements
        .create_index(IndexModel::builder().keys(doc! { "company_id": 1, "nomenclature_id": 1, "created_at": -1 }).build())
        .await
    { warn!("stock_movements card: {e}"); }

    if let Err(e) = movements
        .create_index(IndexModel::builder().keys(doc! { "company_id": 1, "location_id": 1, "created_at": -1 }).build())
        .await
    { warn!("stock_movements by_loc: {e}"); }

    if let Err(e) = movements
        .create_index(IndexModel::builder().keys(doc! { "doc_id": 1 }).build())
        .await
    { warn!("stock_movements doc_id: {e}"); }

    // Подотчёт: просроченные возвраты
    if let Err(e) = movements
        .create_index(IndexModel::builder().keys(doc! { "responsible_user_id": 1, "expected_return_date": 1 }).build())
        .await
    { warn!("stock_movements handover: {e}"); }
}
