use mongodb::IndexModel;
use mongodb::bson::doc;
use tracing::info;

use crate::db::MongoClient;
use crate::core::PlatformResult;

pub async fn ensure_audit_indexes(db: &MongoClient) -> PlatformResult<()> {
    let col = db.collection::<mongodb::bson::Document>("audit_log");

    let indexes = vec![
        IndexModel::builder().keys(doc! { "company_id": 1, "occurred_at": -1 }).build(),
        IndexModel::builder().keys(doc! { "company_id": 1, "user_id": 1, "occurred_at": -1 }).build(),
        IndexModel::builder().keys(doc! { "company_id": 1, "target_type": 1, "target_id": 1, "occurred_at": -1 }).build(),
        IndexModel::builder().keys(doc! { "company_id": 1, "action": 1, "occurred_at": -1 }).build(),
    ];

    for idx in indexes {
        let _ = col.create_index(idx).await;
    }

    info!("Audit log indexes ensured");
    Ok(())
}
