use mongodb::bson::doc;
use mongodb::IndexModel;
use tracing::warn;

use crate::db::MongoClient;
use super::journal::COLLECTION;

pub async fn ensure_indexes(db: &MongoClient) {
    let col = db.collection::<mongodb::bson::Document>(COLLECTION);

    // Основа конкурентной идемпотентности: проигравший параллельный вызов
    // ловит дубликат на вставке журнала и возвращает результат победителя.
    if let Err(e) = col
        .create_index(
            IndexModel::builder()
                .keys(doc! { "company_id": 1, "idempotency_key": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .unique(true)
                        .name("uniq_company_idempotency".to_string())
                        .build(),
                )
                .build(),
        )
        .await
    {
        warn!("Индекс tx_journal.company_id+idempotency_key: {e}");
    }

    // Прочистка старых записей — на будущее (TTL), сейчас журнал вечен.
}
