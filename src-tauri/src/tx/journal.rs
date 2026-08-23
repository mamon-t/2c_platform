//! Журнал транзакций (основа идемпотентности).
//!
//! Запись журнала вставляется ВНУТРИ той же транзакции, что и бизнес-
//! операции: сбой между коммитом бизнеса и журналом привёл бы к двойному
//! применению при ретрае. Уникальный индекс (company_id, idempotency_key)
//! разрешает конкурентные повторы: проигравший ловит 11000 и возвращает
//! результат победителя.

use mongodb::bson::{doc, Document};

use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;

pub const COLLECTION: &str = "tx_journal";

pub struct TxJournal;

impl TxJournal {
    /// Найти результат успешно выполненной пачки по ключу.
    pub async fn find_committed(
        db: &MongoClient,
        company_id: &CompanyId,
        idempotency_key: &str,
    ) -> PlatformResult<Option<serde_json::Value>> {
        let col = db.collection::<Document>(COLLECTION);
        let rec = col
            .find_one(doc! {
                "company_id": company_id.0.to_string(),
                "idempotency_key": idempotency_key,
                "status": "committed",
            })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        Ok(rec.and_then(|d| {
            d.get("result")
                .cloned()
                .and_then(|b| mongodb::bson::from_bson::<serde_json::Value>(b).ok())
        }))
    }

    /// Вставить запись о коммите ВНУТРИ транзакции.
    ///
    /// Конфликт уникального индекса (код 11000) означает, что параллельный
    /// вызов с тем же ключом закоммитился первым — исполнитель обязан
    /// откатиться и вернуть результат победителя.
    pub async fn insert_committed_in_session(
        db: &MongoClient,
        session: &mut mongodb::ClientSession,
        company_id: &CompanyId,
        idempotency_key: &str,
        ops_count: usize,
        result: &serde_json::Value,
    ) -> PlatformResult<()> {
        let now = chrono::Utc::now();
        let rec = doc! {
            "_id": uuid::Uuid::new_v4().to_string(),
            "company_id": company_id.0.to_string(),
            "idempotency_key": idempotency_key,
            "status": "committed",
            "ops_count": ops_count as i64,
            "result": mongodb::bson::to_bson(result)
                .map_err(|e| PlatformError::Internal(format!("BSON результата: {e}")))?,
            "executed_at": mongodb::bson::DateTime::from_millis(now.timestamp_millis()),
        };

        db.collection::<Document>(COLLECTION)
            .insert_one(rec)
            .session(session)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }
}
