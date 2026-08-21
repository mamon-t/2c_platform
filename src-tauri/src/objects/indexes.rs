use mongodb::IndexModel;
use mongodb::bson::doc;
use tracing::{info, warn};

use crate::db::MongoClient;
use crate::core::PlatformResult;

pub async fn ensure_object_indexes(db: &MongoClient) -> PlatformResult<()> {
    let obj = db.collection::<mongodb::bson::Document>("objects");

    let indexes = vec![
        ("entity_type_id + company_id",           doc! { "entity_type_id": 1, "company_id": 1 }),
        ("company_id + state",                     doc! { "company_id": 1, "state": 1 }),
        ("number",                                 doc! { "number": 1 }),
        ("parent_id",                              doc! { "parent_id": 1 }),
        ("company_id + updated_at",                doc! { "company_id": 1, "updated_at": -1 }),
        ("entity_type_id + company_id + updated_at", doc! { "entity_type_id": 1, "company_id": 1, "updated_at": -1 }),
    ];

    for (name, keys) in indexes {
        if let Err(e) = obj.create_index(IndexModel::builder().keys(keys).build()).await {
            warn!("Не удалось создать индекс objects/{name}: {e}");
        }
    }

    let snap = db.collection::<mongodb::bson::Document>("object_snapshots");
    if let Err(e) = snap.create_index(IndexModel::builder().keys(doc! { "object_id": 1, "version": -1 }).build()).await {
        warn!("Не удалось создать индекс object_snapshots/object_id + version: {e}");
    }

    info!("Object indexes ensured");
    Ok(())
}
