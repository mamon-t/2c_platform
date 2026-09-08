//! Хранилище универсальных бизнес-объектов на базе SurrealDB (проекция
//! «Доска» для коллекции `objects`), их истории версий
//! (`object_snapshots`) и атомарной нумерации документов (`document_numbers`).
//!
//! Проверка полей по метатиповой модели выполняется на командном слое через
//! `Object::validate`; этот репозиторий только сохраняет.

use core_application::ports::ObjectRepository;
use core_domain::error::DomainError;
use core_domain::event::Event;
use core_domain::object::{Object, ObjectSnapshot};
use core_domain::types::{AggregateId, Version};
use chrono::Utc;
use serde_json::{json, Value};
use surrealdb::engine::any::Any;
use surrealdb::method::Transaction;
use surrealdb::Surreal;
use uuid::Uuid;

use crate::events::{assign_versions, with_transaction, write_events};

const OBJECT_TABLE: &str = "objects";
const SNAPSHOT_TABLE: &str = "object_snapshots";
const NUMBER_TABLE: &str = "document_numbers";

const OBJECT_FIELDS: &str = "record::id(id) AS id, entity_type, kind, company_id, state, data, \
    computed, number, date, parent_id, version, created_by, updated_by, created_at, updated_at";
const SNAPSHOT_FIELDS: &str = "record::id(id) AS id, object_id, version, data, state, \
    changed_by, changed_at";

/// Фиксирует проекцию «Доска» универсальных объектов.
pub struct SurrealObjectRepository {
    db: Surreal<Any>,
}

impl SurrealObjectRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт таблицы объектов, снимков и счётчиков вместе с индексами
    /// идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS objects SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_objects_entity_company \
                ON objects FIELDS entity_type, company_id",
            "DEFINE INDEX IF NOT EXISTS idx_objects_number \
                ON objects FIELDS entity_type, company_id, number UNIQUE",
            "DEFINE TABLE IF NOT EXISTS object_snapshots SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_snapshots_object_version \
                ON object_snapshots FIELDS object_id, version UNIQUE",
            "DEFINE TABLE IF NOT EXISTS document_numbers SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_document_numbers_key \
                ON document_numbers FIELDS entity_type, company_id UNIQUE",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("objects ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("objects ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

fn decode_row<T: serde::de::DeserializeOwned>(
    kind: &str,
    row: Option<Value>,
) -> Result<T, DomainError> {
    row.map(serde_json::from_value)
        .transpose()
        .map_err(|e| DomainError::Storage(format!("{kind} decode: {e}")))?
        .ok_or_else(|| DomainError::NotFound(kind.to_string()))
}

fn decode_rows<T: serde::de::DeserializeOwned>(
    kind: &str,
    rows: Vec<Value>,
) -> Result<Vec<T>, DomainError> {
    serde_json::from_value(Value::Array(rows))
        .map_err(|e| DomainError::Storage(format!("{kind} list decode: {e}")))
}

/// Формирует запись снимка для предстоящей `version` объекта.
async fn write_snapshot(
    txn: &Transaction<Any>,
    obj: &Object,
) -> Result<(), DomainError> {
    let snapshot = ObjectSnapshot {
        id: Uuid::new_v4(),
        object_id: obj.id,
        version: obj.version,
        data: obj.data.clone(),
        state: obj.state.clone(),
        changed_by: obj.updated_by.clone(),
        changed_at: obj.updated_at,
    };
    let value = serde_json::to_value(&snapshot)
        .map_err(|e| DomainError::Storage(format!("snapshot encode: {e}")))?;
    let _: Option<Value> = txn
        .upsert((SNAPSHOT_TABLE, snapshot.id.to_string()))
        .content(value)
        .await
        .map_err(|e| DomainError::Storage(format!("snapshot write: {e}")))?;
    Ok(())
}

/// Атомарно увеличивает счётчик в разрезе (тип сущности, компания) внутри
/// открытой транзакции. Откаченная операция оставляет пропуск в нумерации,
/// что допустимо для v0.1 (номера остаются уникальными, не обязательно
/// идущими подряд).
async fn assign_document_number(
    txn: &Transaction<Any>,
    entity_type: &str,
    company_id: &str,
) -> Result<String, DomainError> {
    let key = number_key(entity_type, company_id);
    let mut response = txn
        .query(format!(
            "SELECT * FROM {NUMBER_TABLE} \
             WHERE record::id(id) = $id LIMIT 1"
        ))
        .bind(("id", key.clone()))
        .await
        .map_err(|e| DomainError::Storage(format!("document_numbers read: {e}")))?;
    let existing: Option<Value> = response
        .take(0)
        .map_err(|e| DomainError::Storage(format!("document_numbers read take: {e}")))?;

    let next = match existing {
        Some(row) => row
            .get("value")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            + 1,
        None => 1,
    };
    let _: Option<Value> = txn
        .upsert((NUMBER_TABLE, key.clone()))
        .content(json!({
            "id": key,
            "entity_type": entity_type,
            "company_id": company_id,
            "value": next,
        }))
        .await
        .map_err(|e| DomainError::Storage(format!("document_numbers write: {e}")))?;

    let year = Utc::now().format("%Y");
    Ok(format!("{entity_type}-{year}-{next:04}"))
}

fn number_key(entity_type: &str, company_id: &str) -> String {
    format!("document-number-{company_id}-{entity_type}")
}

impl ObjectRepository for SurrealObjectRepository {
    async fn get_with_version(
        &self,
        id: &AggregateId,
    ) -> Result<(Object, Version), DomainError> {
        let obj = self.get(id).await?;
        Ok((obj.clone(), obj.version))
    }

    async fn get(&self, id: &AggregateId) -> Result<Object, DomainError> {
        let mut response = self
            .db
            .query(format!(
                "SELECT {OBJECT_FIELDS} FROM {OBJECT_TABLE} \
                 WHERE record::id(id) = $id LIMIT 1"
            ))
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("objects get: {e}")))?;
        let row: Option<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("objects get take: {e}")))?;
        decode_row("Объект", row)
    }

    async fn create(
        &self,
        obj: &Object,
        events: &[Event],
    ) -> Result<Object, DomainError> {
        let mut stored = obj.clone();
        stored.version = 1;
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<Object, DomainError> = async {
                let mut response = txn
                    .query(format!(
                        "SELECT record::id(id) AS id FROM {OBJECT_TABLE} \
                         WHERE record::id(id) = $id LIMIT 1"
                    ))
                    .bind(("id", stored.id.to_string()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("objects lookup: {e}")))?;
                let existing: Option<Value> = response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("objects lookup take: {e}")))?;
                if existing.is_some() {
                    return Err(DomainError::ValidationError(format!(
                        "Объект {} уже существует",
                        stored.id
                    )));
                }

                if stored.number.is_none() && stored.is_document() {
                    let number =
                        assign_document_number(&txn, &stored.entity_type, &stored.company_id).await?;
                    stored.number = Some(number);
                }

                write_object(&txn, &stored).await?;
                write_snapshot(&txn, &stored).await?;

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;
                Ok(stored)
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn update(
        &self,
        obj: &Object,
        events: &[Event],
    ) -> Result<Object, DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<Object, DomainError> = async {
                let current = load_object(&txn, &obj.id).await?;
                current_version_checked(&current, obj.version)?;
                let mut stored = obj.clone();
                stored.version = current.version + 1;
                write_object(&txn, &stored).await?;
                write_snapshot(&txn, &stored).await?;

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;
                Ok(stored)
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn delete(
        &self,
        id: &AggregateId,
        events: &[Event],
    ) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let current = load_object(&txn, id).await?;
                if current.version > 1 {
                    return Err(DomainError::ValidationError(
                        "Удаление объекта с историей запрещено".to_string(),
                    ));
                }
                let _: Option<Value> = txn
                    .delete((OBJECT_TABLE, id.to_string()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("objects delete: {e}")))?;
                let _: Option<Value> = txn
                    .query(format!(
                        "DELETE FROM {SNAPSHOT_TABLE} WHERE object_id = $id"
                    ))
                    .bind(("id", id.to_string()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("snapshots delete: {e}")))?
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("snapshots delete take: {e}")))?;

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn list(
        &self,
        entity_type: &str,
        company_id: &str,
        limit: usize,
    ) -> Result<Vec<Object>, DomainError> {
        let mut response = self
            .db
            .query(format!(
                "SELECT {OBJECT_FIELDS} FROM {OBJECT_TABLE} \
                 WHERE entity_type = $et AND company_id = $cid \
                 ORDER BY updated_at DESC LIMIT $limit"
            ))
            .bind(("et", entity_type.to_string()))
            .bind(("cid", company_id.to_string()))
            .bind(("limit", limit as u64))
            .await
            .map_err(|e| DomainError::Storage(format!("objects list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("objects list take: {e}")))?;
        decode_rows("objects", rows)
    }

    async fn count(
        &self,
        entity_type: &str,
        company_id: &str,
    ) -> Result<u64, DomainError> {
        let mut response = self
            .db
            .query(format!(
                "SELECT count() AS total FROM {OBJECT_TABLE} \
                 WHERE entity_type = $et AND company_id = $cid GROUP ALL"
            ))
            .bind(("et", entity_type.to_string()))
            .bind(("cid", company_id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("objects count: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("objects count take: {e}")))?;
        Ok(rows
            .first()
            .and_then(|row| row.get("total"))
            .and_then(Value::as_u64)
            .unwrap_or(0))
    }

    async fn get_snapshots(
        &self,
        object_id: &AggregateId,
    ) -> Result<Vec<ObjectSnapshot>, DomainError> {
        let mut response = self
            .db
            .query(format!(
                "SELECT {SNAPSHOT_FIELDS} FROM {SNAPSHOT_TABLE} \
                 WHERE object_id = $id ORDER BY version"
            ))
            .bind(("id", object_id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("snapshots list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("snapshots list take: {e}")))?;
        decode_rows("object_snapshots", rows)
    }

    async fn restore_snapshot(
        &self,
        object_id: &AggregateId,
        version: Version,
        events: &[Event],
    ) -> Result<Object, DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<Object, DomainError> = async {
                let current = load_object(&txn, object_id).await?;

                let mut response = txn
                    .query(format!(
                        "SELECT {SNAPSHOT_FIELDS} FROM {SNAPSHOT_TABLE} \
                         WHERE object_id = $id AND version = $version LIMIT 1"
                    ))
                    .bind(("id", object_id.to_string()))
                    .bind(("version", version))
                    .await
                    .map_err(|e| DomainError::Storage(format!("snapshot read: {e}")))?;
                let row: Option<Value> = response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("snapshot read take: {e}")))?;
                let snapshot: ObjectSnapshot =
                    decode_row("Снимок объекта", row).map_err(|e| match e {
                        DomainError::NotFound(_) => DomainError::NotFound(format!(
                            "Версия {} не найдена у объекта {}",
                            version, object_id
                        )),
                        other => other,
                    })?;

                let mut next = current.clone();
                next.data = snapshot.data;
                next.state = snapshot.state;
                next.version = current.version + 1;
                next.updated_by = "system".to_string();
                next.updated_at = Utc::now();
                write_object(&txn, &next).await?;
                write_snapshot(&txn, &next).await?;

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;
                Ok(next)
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn next_document_number(
        &self,
        entity_type: &str,
        company_id: &str,
    ) -> Result<String, DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<String, DomainError> =
                assign_document_number(&txn, entity_type, company_id).await;
            (txn, outcome)
        })
        .await
    }
}

/// Загружает объект по идентификатору внутри открытой транзакции.
async fn load_object(txn: &Transaction<Any>, id: &Uuid) -> Result<Object, DomainError> {
    let mut response = txn
        .query(format!(
            "SELECT {OBJECT_FIELDS} FROM {OBJECT_TABLE} \
             WHERE record::id(id) = $id LIMIT 1"
        ))
        .bind(("id", id.to_string()))
        .await
        .map_err(|e| DomainError::Storage(format!("objects load: {e}")))?;
    let row: Option<Value> = response
        .take(0)
        .map_err(|e| DomainError::Storage(format!("objects load take: {e}")))?;
    decode_row("Объект", row)
}

fn current_version_checked(current: &Object, expected: Version) -> Result<(), DomainError> {
    if current.version != expected {
        return Err(DomainError::VersionConflict {
            expected,
            actual: current.version,
        });
    }
    Ok(())
}

async fn write_object(txn: &Transaction<Any>, obj: &Object) -> Result<(), DomainError> {
    let value = serde_json::to_value(obj)
        .map_err(|e| DomainError::Storage(format!("object encode: {e}")))?;
    let _: Option<Value> = txn
        .upsert((OBJECT_TABLE, obj.id.to_string()))
        .content(value)
        .await
        .map_err(|e| DomainError::Storage(format!("object write: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_application::ports::EventStore;
    use core_domain::event::{ActorSnapshot, StreamType};
    use core_domain::object::ObjectKind;
    use serde_json::json;

    async fn mem_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        db
    }

    async fn fixtures() -> (SurrealObjectRepository, crate::SurrealEventStore) {
        let db = mem_db().await;
        let repo = SurrealObjectRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();
        (repo, store)
    }

    fn system_event(stream_id: &str, event_type: &str) -> Event {
        Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Object,
            stream_id: stream_id.to_string(),
            event_type: event_type.to_string(),
            version: 0,
            payload: json!({}),
            metadata: ActorSnapshot::system(),
            company_id: String::new(),
            correlation_id: "corr".to_string(),
            causation_id: None,
            occurred_at: Utc::now(),
        }
    }

    fn sample_object(company_id: &str, data: Value) -> Object {
        let now = Utc::now();
        Object {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            kind: ObjectKind::Document,
            company_id: company_id.to_string(),
            state: "draft".to_string(),
            data,
            computed: json!({}),
            number: None,
            date: None,
            parent_id: None,
            version: 1,
            created_by: "system".to_string(),
            updated_by: "system".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn round_trip_create_get_update_and_events() {
        let (repo, store) = fixtures().await;
        let mut obj = sample_object("c1", json!({"sum": 1000}));
        let created = repo
            .create(&obj, &[system_event(&obj.id.to_string(), "object.created")])
            .await
            .unwrap();
        assert_eq!(created.version, 1);
        assert_eq!(created.number, Some("invoice-2026-0001".to_string()));

        let fetched = repo.get(&obj.id).await.unwrap();
        assert_eq!(fetched.data, json!({"sum": 1000}));

        obj.data = json!({"sum": 2500});
        obj.version = 1; // ожидаемая версия
        obj.updated_at = Utc::now();
        let updated = repo
            .update(&obj, &[system_event(&obj.id.to_string(), "object.updated")])
            .await
            .unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.data, json!({"sum": 2500}));

        let stream = store
            .read_stream(StreamType::Object, &obj.id.to_string())
            .await
            .unwrap();
        assert_eq!(stream.len(), 2);
        assert_eq!(stream[0].event_type, "object.created");
        assert_eq!(stream[1].event_type, "object.updated");
        assert_eq!(stream[1].version, 2);
    }

    #[tokio::test]
    async fn occ_conflict_rejected() {
        let (repo, _store) = fixtures().await;
        let mut obj = sample_object("c1", json!({"sum": 100}));
        repo.create(&obj, &[system_event(&obj.id.to_string(), "object.created")])
            .await
            .unwrap();

        // Первое обновление проходит успешно.
        obj.data = json!({"sum": 200});
        obj.version = 1;
        repo.update(&obj, &[system_event(&obj.id.to_string(), "object.updated")])
            .await
            .unwrap();

        // Устаревший писатель всё ещё ожидает версию 1.
        let mut stale = obj.clone();
        stale.data = json!({"sum": 999});
        stale.version = 1;
        let err = repo
            .update(&stale, &[system_event(&stale.id.to_string(), "object.updated")])
            .await
            .unwrap_err();
        match err {
            DomainError::VersionConflict { expected, actual } => {
                assert_eq!(expected, 1);
                assert_eq!(actual, 2);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn document_numbering_is_atomic_and_unique() {
        let (repo, _store) = fixtures().await;
        let first = repo
            .next_document_number("invoice", "c1")
            .await
            .unwrap();
        let second = repo
            .next_document_number("invoice", "c1")
            .await
            .unwrap();
        assert!(first.ends_with("-0001"));
        assert!(second.ends_with("-0002"));

        let obj_a = sample_object("c1", json!({}));
        let mut obj_b = sample_object("c1", json!({}));
        let created_a = repo
            .create(&obj_a, &[system_event(&obj_a.id.to_string(), "object.created")])
            .await
            .unwrap();
        obj_b.id = Uuid::new_v4();
        let created_b = repo
            .create(&obj_b, &[system_event(&obj_b.id.to_string(), "object.created")])
            .await
            .unwrap();
        assert!(created_a.number.is_some());
        assert!(created_b.number.is_some());
        assert_ne!(created_a.number, created_b.number);

        // Номера уникальны в рамках компании.
        let obj_c = sample_object("c2", json!({}));
        let created_c = repo
            .create(&obj_c, &[system_event(&obj_c.id.to_string(), "object.created")])
            .await
            .unwrap();
        assert_eq!(created_c.number, Some("invoice-2026-0001".to_string()));
    }

    #[tokio::test]
    async fn snapshots_and_restore_produce_new_version() {
        let (repo, _store) = fixtures().await;
        let mut obj = sample_object("c1", json!({"sum": 100}));
        repo.create(&obj, &[system_event(&obj.id.to_string(), "object.created")])
            .await
            .unwrap();

        obj.data = json!({"sum": 500});
        obj.version = 1;
        repo.update(&obj, &[system_event(&obj.id.to_string(), "object.updated")])
            .await
            .unwrap();

        let snapshots = repo.get_snapshots(&obj.id).await.unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[1].data, json!({"sum": 500}));

        let restored = repo
            .restore_snapshot(&obj.id, 1, &[system_event(&obj.id.to_string(), "object.restored")])
            .await
            .unwrap();
        assert_eq!(restored.version, 3);
        assert_eq!(restored.data, json!({"sum": 100}));

        let snapshots = repo.get_snapshots(&obj.id).await.unwrap();
        assert_eq!(snapshots.len(), 3);
        assert_eq!(snapshots[2].version, 3);
    }

    #[tokio::test]
    async fn delete_allowed_only_for_draft_without_history() {
        let (repo, store) = fixtures().await;
        let mut obj = sample_object("c1", json!({}));
        repo.create(&obj, &[system_event(&obj.id.to_string(), "object.created")])
            .await
            .unwrap();

        // Обновление создаёт историю → удаление запрещено.
        obj.data = json!({"sum": 1});
        obj.version = 1;
        repo.update(&obj, &[system_event(&obj.id.to_string(), "object.updated")])
            .await
            .unwrap();
        let err = repo
            .delete(&obj.id, &[system_event(&obj.id.to_string(), "object.deleted")])
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));

        // Чистовой черновик (версия 1) можно удалить физически.
        let draft = sample_object("c1", json!({}));
        let created = repo
            .create(&draft, &[system_event(&draft.id.to_string(), "object.created")])
            .await
            .unwrap();
        assert_eq!(created.version, 1);
        repo.delete(
            &draft.id,
            &[system_event(&draft.id.to_string(), "object.deleted")],
        )
        .await
        .unwrap();
        assert!(matches!(repo.get(&draft.id).await, Err(DomainError::NotFound(_))));
        assert_eq!(
            store
                .read_stream(StreamType::Object, &draft.id.to_string())
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn list_is_scoped_by_type_and_company() {
        let (repo, _store) = fixtures().await;
        let obj = sample_object("c1", json!({}));
        repo.create(&obj, &[system_event(&obj.id.to_string(), "object.created")])
            .await
            .unwrap();

        let other = Object {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            kind: ObjectKind::Catalog,
            company_id: "c2".to_string(),
            state: "active".to_string(),
            data: json!({}),
            computed: json!({}),
            number: None,
            date: None,
            parent_id: None,
            version: 1,
            created_by: "system".to_string(),
            updated_by: "system".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.create(&other, &[system_event(&other.id.to_string(), "object.created")])
            .await
            .unwrap();

        assert_eq!(repo.list("invoice", "c1", 10).await.unwrap().len(), 1);
        assert_eq!(repo.list("invoice", "c2", 10).await.unwrap().len(), 1);
        assert_eq!(repo.list("absent", "c1", 10).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn count_is_scoped_by_type_and_company() {
        let (repo, _store) = fixtures().await;
        repo.create(
            &sample_object("c1", json!({})),
            &[system_event("a", "object.created")],
        )
        .await
        .unwrap();
        repo.create(
            &sample_object("c2", json!({})),
            &[system_event("b", "object.created")],
        )
        .await
        .unwrap();

        assert_eq!(repo.count("invoice", "c1").await.unwrap(), 1);
        assert_eq!(repo.count("invoice", "c2").await.unwrap(), 1);
        assert_eq!(repo.count("absent", "c1").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn get_missing_returns_not_found() {
        let (repo, _store) = fixtures().await;
        assert!(matches!(
            repo.get(&Uuid::new_v4()).await,
            Err(DomainError::NotFound(_))
        ));
    }
}