// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use mongodb::bson::doc;
use mongodb::IndexModel;
use tracing::warn;

use crate::db::MongoClient;
use super::{COL_ACCOUNTS, COL_BALANCES, COL_ENTRIES, COL_PERIODS};

pub async fn ensure_indexes(db: &MongoClient) {
    let accounts = db.collection::<mongodb::bson::Document>(COL_ACCOUNTS);
    let entries = db.collection::<mongodb::bson::Document>(COL_ENTRIES);
    let balances = db.collection::<mongodb::bson::Document>(COL_BALANCES);
    let periods = db.collection::<mongodb::bson::Document>(COL_PERIODS);

    if let Err(e) = accounts
        .create_index(IndexModel::builder()
            .keys(doc! { "company_id": 1, "code": 1 })
            .options(mongodb::options::IndexOptions::builder().unique(true).build())
            .build())
        .await
    { warn!("ledger_accounts uniq: {e}"); }

    // Журнал проводок: документ / период+дата / стороны пар (карточка счёта)
    for keys in [
        doc! { "company_id": 1, "doc_id": 1 },
        doc! { "company_id": 1, "period_key": 1, "date": 1 },
        doc! { "company_id": 1, "debit_code": 1, "date": -1 },
        doc! { "company_id": 1, "credit_code": 1, "date": -1 },
        doc! { "posting_id": 1 },
        doc! { "nomenclature_id": 1 },
    ] {
        if let Err(e) = entries.create_index(IndexModel::builder().keys(keys).build()).await {
            warn!("ledger_entries index: {e}");
        }
    }

    // Обороты периода по счёту — уникальны
    if let Err(e) = balances
        .create_index(IndexModel::builder()
            .keys(doc! { "company_id": 1, "period_key": 1, "account_id": 1 })
            .options(mongodb::options::IndexOptions::builder().unique(true).build())
            .build())
        .await
    { warn!("ledger_balances uniq: {e}"); }

    if let Err(e) = periods
        .create_index(IndexModel::builder()
            .keys(doc! { "company_id": 1, "period_key": 1 })
            .options(mongodb::options::IndexOptions::builder().unique(true).build())
            .build())
        .await
    { warn!("accounting_periods uniq: {e}"); }
}
