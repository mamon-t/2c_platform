//! Хранилище событий на базе SurrealDB — «Труба» архитектуры «Труба и Доска».

use core_application::ports::EventStore;
use core_domain::error::DomainError;
use core_domain::event::{Event, StreamType};
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

use crate::connector::connect_db;
use crate::events::append_events;

/// Журнал событий только с добавлением, хранящийся в коллекции
/// SurrealDB `events`.
///
/// Идентификатор записи сохранённого события равен самому идентификатору
/// события, что делает повторный append идемпотентным: `upsert` перезаписывает
/// ту же запись вместо создания дубля.
pub struct SurrealEventStore {
    db: Surreal<Any>,
}

impl SurrealEventStore {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Подключается по WebSocket, выполняет аутентификацию пользователя root
    /// и выбирает целевые пространство имён и базу данных.
    pub async fn connect(
        host: &str,
        user: &str,
        pass: &str,
        ns: &str,
        db_name: &str,
    ) -> Result<Self, DomainError> {
        let db = connect_db(host, user, pass, ns, db_name).await?;
        Ok(Self { db })
    }

    /// Создаёт индексы коллекции `events`, требуемые спецификацией, идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const INDEXES: &[&str] = &[
            "DEFINE INDEX IF NOT EXISTS events_stream ON events FIELDS stream_type, stream_id, version",
            "DEFINE INDEX IF NOT EXISTS events_type_time ON events FIELDS event_type, occurred_at",
            "DEFINE INDEX IF NOT EXISTS events_company_time ON events FIELDS company_id, occurred_at",
            "DEFINE INDEX IF NOT EXISTS events_correlation ON events FIELDS correlation_id",
        ];
        for stmt in INDEXES {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

impl EventStore for SurrealEventStore {
    async fn append(&self, events: &[Event]) -> Result<(), DomainError> {
        if events.is_empty() {
            return Ok(());
        }
        append_events(&self.db, events).await
    }

    async fn read_stream(
        &self,
        stream_type: StreamType,
        stream_id: &str,
    ) -> Result<Vec<Event>, DomainError> {
        let type_filter = stream_type.as_str();
        let mut response = self
            .db
            .query(
                "SELECT \
                        record::id(id) AS id, stream_type, stream_id, event_type, version, \
                        payload, metadata, company_id, correlation_id, causation_id, occurred_at \
                     FROM events \
                     WHERE stream_type = $stream_type AND stream_id = $stream_id \
                     ORDER BY version ASC",
            )
            .bind(("stream_type", type_filter))
            .bind(("stream_id", stream_id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("read_stream: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("read_stream take: {e}")))?;
        let events: Vec<Event> = serde_json::from_value(Value::Array(rows))
            .map_err(|e| DomainError::Storage(format!("read_stream decode: {e}")))?;
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use core_domain::event::ActorSnapshot;
    use serde_json::json;
    use uuid::Uuid;

    async fn mem_store() -> SurrealEventStore {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        let store = SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();
        store
    }

    fn make_event(id: Uuid, stream_id: &str, version: u64, event_type: &str) -> Event {
        Event {
            id,
            stream_type: StreamType::Object,
            stream_id: stream_id.to_string(),
            event_type: event_type.to_string(),
            version,
            payload: json!({"text": format!("v{version}")}),
            metadata: ActorSnapshot {
                user_id: Some(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
                login: "admin".to_string(),
                full_name: "Admin Adminov".to_string(),
                position: None,
                company_id: None,
                ip_address: None,
            },
            company_id: "c1".to_string(),
            correlation_id: "corr1".to_string(),
            causation_id: None,
            occurred_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn append_read_returns_same_events_in_order() {
        let store = mem_store().await;
        let e1 = make_event(Uuid::new_v4(), "obj-1", 1, "object.created");
        let e2 = make_event(Uuid::new_v4(), "obj-1", 2, "object.created");
        let e3 = make_event(Uuid::new_v4(), "obj-1", 3, "document.posted");

        store.append(&[e1.clone(), e2.clone(), e3.clone()]).await.unwrap();
        let read = store
            .read_stream(StreamType::Object, "obj-1")
            .await
            .unwrap();

        assert_eq!(read.len(), 3);
        assert_eq!(
            read.iter().map(|e| e.version).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(read[0].id, e1.id);
        assert_eq!(read[1].stream_id, "obj-1");
        assert_eq!(read[2].event_type, "document.posted");
        assert_eq!(read[0].metadata.login, "admin");
    }

    #[tokio::test]
    async fn retried_append_is_idempotent() {
        let store = mem_store().await;
        let event = make_event(Uuid::new_v4(), "obj-1", 1, "object.created");

        store.append(std::slice::from_ref(&event)).await.unwrap();
        store.append(std::slice::from_ref(&event)).await.unwrap();

        let read = store
            .read_stream(StreamType::Object, "obj-1")
            .await
            .unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].version, 1);
    }

    #[tokio::test]
    async fn read_stream_filters_by_stream_id() {
        let store = mem_store().await;
        store
            .append(&[
                make_event(Uuid::new_v4(), "obj-1", 1, "object.created"),
                make_event(Uuid::new_v4(), "obj-2", 1, "object.created"),
                make_event(Uuid::new_v4(), "obj-1", 2, "object.created"),
            ])
            .await
            .unwrap();

        let read = store
            .read_stream(StreamType::Object, "obj-1")
            .await
            .unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].version, 1);
        assert_eq!(read[1].version, 2);
    }

    #[tokio::test]
    async fn ensure_schema_is_idempotent() {
        let store = mem_store().await;
        store.ensure_schema().await.unwrap();
        store.ensure_schema().await.unwrap();
    }
}