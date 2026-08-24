// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use mongodb::IndexModel;
use mongodb::bson::doc;
use tracing::{info, warn};

use crate::db::MongoClient;
use crate::core::PlatformResult;

pub async fn ensure_audit_indexes(db: &MongoClient) -> PlatformResult<()> {
    let col = db.collection::<mongodb::bson::Document>("audit_log");

    let indexes = vec![
        ("company_id + occurred_at",                   doc! { "company_id": 1, "occurred_at": -1 }),
        ("company_id + user_id + occurred_at",        doc! { "company_id": 1, "user_id": 1, "occurred_at": -1 }),
        ("company_id + target_type + target_id + occurred_at", doc! { "company_id": 1, "target_type": 1, "target_id": 1, "occurred_at": -1 }),
        ("company_id + action + occurred_at",          doc! { "company_id": 1, "action": 1, "occurred_at": -1 }),
    ];

    for (name, keys) in indexes {
        if let Err(e) = col.create_index(IndexModel::builder().keys(keys).build()).await {
            warn!("Не удалось создать индекс audit_log/{name}: {e}");
        }
    }

    info!("Audit log indexes ensured");
    Ok(())
}
