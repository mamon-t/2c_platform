use mongodb::IndexModel;
use mongodb::bson::doc;
use tracing::info;

use crate::db::MongoClient;
use crate::core::PlatformResult;

const COLLECTION: &str = "events";

pub async fn ensure_event_indexes(db: &MongoClient) -> PlatformResult<()> {
    let col = db.collection::<mongodb::bson::Document>(COLLECTION);

    let indexes = vec![
        // Основной индекс для чтения потока (object version history)
        IndexModel::builder()
            .keys(doc! { "stream_type": 1, "stream_id": 1, "version": 1 })
            .build(),
        // Поиск по типу события и времени
        IndexModel::builder()
            .keys(doc! { "event_type": 1, "occurred_at": -1 })
            .build(),
        // Поиск по компании и времени
        IndexModel::builder()
            .keys(doc! { "company_id": 1, "occurred_at": -1 })
            .build(),
        // Correlation ID для трассировки бизнес-операций
        IndexModel::builder()
            .keys(doc! { "correlation_id": 1 })
            .build(),
    ];

    for idx in indexes {
        let _ = col.create_index(idx).await;
    }

    info!("Event store indexes ensured");
    Ok(())
}
