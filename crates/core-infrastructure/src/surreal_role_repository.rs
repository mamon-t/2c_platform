//! SurrealDB-backed storage of roles (the "Board" projection).

use core_application::ports::RoleRepository;
use core_domain::error::DomainError;
use core_domain::event::Event;
use core_domain::role::Role;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

use crate::events::{assign_versions, with_transaction, write_events};

/// Materialized `roles` collection with a unique `code` index.
pub struct SurrealRoleRepository {
    db: Surreal<Any>,
}

impl SurrealRoleRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Creates the `roles` table and indexes idempotently.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS roles SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_roles_code ON roles FIELDS code UNIQUE",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("roles ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("roles ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

const ROLE_FIELDS: &str =
    "record::id(id) AS id, code, name, description, is_system, created_at, updated_at";

impl RoleRepository for SurrealRoleRepository {
    async fn create(&self, role: &Role, events: &[Event]) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let mut response = txn
                    .query("SELECT 1 FROM roles WHERE code = $code LIMIT 1")
                    .bind(("code", role.code.clone()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("role check code: {e}")))?;
                let existing: Option<Value> = response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("role check code take: {e}")))?;
                if existing.is_some() {
                    return Err(DomainError::ValidationError(format!(
                        "Роль с кодом {} уже существует",
                        role.code
                    )));
                }

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;

                let value = serde_json::to_value(role)
                    .map_err(|e| DomainError::Storage(format!("role encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("roles", role.id.to_string()))
                    .content(value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("role write: {e}")))?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn get(&self, id: &Uuid) -> Result<Role, DomainError> {
        let mut response = self
            .db
            .query(format!("SELECT {ROLE_FIELDS} FROM roles WHERE record::id(id) = $id LIMIT 1"))
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("role get: {e}")))?;
        let row: Option<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("role get take: {e}")))?;
        row.map(serde_json::from_value)
            .transpose()
            .map_err(|e| DomainError::Storage(format!("role get decode: {e}")))?
            .ok_or_else(|| DomainError::NotFound(format!("Роль {id} не найдена")))
    }

    async fn list(&self) -> Result<Vec<Role>, DomainError> {
        let mut response = self
            .db
            .query(format!("SELECT {ROLE_FIELDS} FROM roles ORDER BY code"))
            .await
            .map_err(|e| DomainError::Storage(format!("role list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("role list take: {e}")))?;
        serde_json::from_value(Value::Array(rows))
            .map_err(|e| DomainError::Storage(format!("role list decode: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_application::ports::EventStore;
    use core_domain::event::{Event, EventMetadata, StreamType};
    use core_domain::role::Role;
    use uuid::Uuid;

    async fn mem_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        db
    }

    fn empty_event(company_id: Uuid) -> Event {
        Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Role,
            stream_id: company_id.to_string(),
            event_type: "role.created".to_string(),
            version: 0,
            payload: serde_json::json!({}),
            metadata: EventMetadata::system(),
            company_id: company_id.to_string(),
            correlation_id: "corr".to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        }
    }

    fn sample(id: Uuid, code: &str, name: &str) -> Role {
        Role {
            id,
            code: code.to_string(),
            name: name.to_string(),
            description: String::new(),
            is_system: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn create_get_list_round_trip() {
        let db = mem_db().await;
        let repo = SurrealRoleRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();

        let id = Uuid::new_v4();
        repo.create(&sample(id, "accountant", "Бухгалтер"), &[empty_event(id)])
            .await
            .unwrap();

        let got = repo.get(&id).await.unwrap();
        assert_eq!(got.code, "accountant");
        assert_eq!(got.name, "Бухгалтер");

        assert_eq!(repo.list().await.unwrap().len(), 1);

        let stream = store.read_stream(StreamType::Role, &id.to_string()).await.unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].event_type, "role.created");
        assert_eq!(stream[0].version, 1);
    }

    #[tokio::test]
    async fn duplicate_code_rejected() {
        let db = mem_db().await;
        let store = crate::SurrealEventStore::new(db.clone());
        store.ensure_schema().await.unwrap();
        let repo = SurrealRoleRepository::new(db);
        repo.ensure_schema().await.unwrap();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        repo.create(&sample(id1, "accountant", "Бухгалтер"), &[empty_event(id1)])
            .await
            .unwrap();
        let err = repo
            .create(&sample(id2, "accountant", "Другой"), &[empty_event(id2)])
            .await;
        assert!(matches!(err, Err(DomainError::ValidationError(_))));
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_missing_returns_not_found() {
        let db = mem_db().await;
        let repo = SurrealRoleRepository::new(db);
        repo.ensure_schema().await.unwrap();
        let err = repo.get(&Uuid::new_v4()).await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
    }

    #[tokio::test]
    async fn ensure_schema_is_idempotent() {
        let db = mem_db().await;
        let repo = SurrealRoleRepository::new(db);
        repo.ensure_schema().await.unwrap();
        repo.ensure_schema().await.unwrap();
    }
}