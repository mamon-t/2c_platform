use mongodb::IndexModel;
use mongodb::bson::doc;
use tracing::{info, warn};

use crate::db::MongoClient;
use crate::core::PlatformResult;

const COLLECTION: &str = "events";

pub async fn ensure_event_indexes(db: &MongoClient) -> PlatformResult<()> {
    let col = db.collection::<mongodb::bson::Document>(COLLECTION);

    let indexes = vec![
        ("stream_type + stream_id + version",  doc! { "stream_type": 1, "stream_id": 1, "version": 1 }),
        ("event_type + occurred_at",           doc! { "event_type": 1, "occurred_at": -1 }),
        ("company_id + occurred_at",           doc! { "company_id": 1, "occurred_at": -1 }),
        ("correlation_id",                     doc! { "correlation_id": 1 }),
    ];

    for (name, keys) in indexes {
        if let Err(e) = col.create_index(IndexModel::builder().keys(keys).build()).await {
            warn!("Не удалось создать индекс events/{name}: {e}");
        }
    }

    info!("Event store indexes ensured");
    Ok(())
}
