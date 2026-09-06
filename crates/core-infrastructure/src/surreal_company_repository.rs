//! SurrealDB-backed storage of companies (the "Board" projection).

use core_application::ports::CompanyRepository;
use core_domain::company::Company;
use core_domain::error::DomainError;
use core_domain::event::Event;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

use crate::events::{assign_versions, with_transaction, write_events};

/// Materialized `companies` collection with a unique `code` index.
pub struct SurrealCompanyRepository {
    db: Surreal<Any>,
}

impl SurrealCompanyRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Creates the `companies` table and indexes idempotently.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS companies SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_companies_code ON companies FIELDS code UNIQUE",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("companies ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("companies ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

const COMPANY_FIELDS: &str = "record::id(id) AS id, code, name, is_active, created_at, updated_at";

fn decode_company(row: Option<Value>) -> Result<Company, DomainError> {
    row.map(serde_json::from_value)
        .transpose()
        .map_err(|e| DomainError::Storage(format!("company decode: {e}")))?
        .ok_or_else(|| DomainError::NotFound("Компания не найдена".to_string()))
}

fn decode_companies(rows: Vec<Value>) -> Result<Vec<Company>, DomainError> {
    serde_json::from_value(Value::Array(rows))
        .map_err(|e| DomainError::Storage(format!("company list decode: {e}")))
}

impl CompanyRepository for SurrealCompanyRepository {
    async fn create(&self, company: &Company, events: &[Event]) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let mut code_response = txn
                    .query("SELECT 1 FROM companies WHERE code = $code LIMIT 1")
                    .bind(("code", company.code.clone()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("company check code: {e}")))?;
                let existing: Option<Value> = code_response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("company check code take: {e}")))?;
                if existing.is_some() {
                    return Err(DomainError::ValidationError(format!(
                        "Компания с кодом {} уже существует",
                        company.code
                    )));
                }

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;

                let value = serde_json::to_value(company)
                    .map_err(|e| DomainError::Storage(format!("company encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("companies", company.id.to_string()))
                    .content(value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("company write: {e}")))?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn get(&self, id: &Uuid) -> Result<Company, DomainError> {
        let mut response = self
            .db
            .query(format!(
                "SELECT {COMPANY_FIELDS} FROM companies WHERE record::id(id) = $id LIMIT 1"
            ))
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("company get: {e}")))?;
        let row: Option<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("company get take: {e}")))?;
        decode_company(row)
    }

    async fn list(&self) -> Result<Vec<Company>, DomainError> {
        let mut response = self
            .db
            .query(format!("SELECT {COMPANY_FIELDS} FROM companies ORDER BY code"))
            .await
            .map_err(|e| DomainError::Storage(format!("company list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("company list take: {e}")))?;
        decode_companies(rows)
    }

    async fn update(&self, company: &Company, events: &[Event]) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let mut id_response = txn
                    .query("SELECT 1 FROM companies WHERE record::id(id) = $id LIMIT 1")
                    .bind(("id", company.id.to_string()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("company check id: {e}")))?;
                let existing: Option<Value> = id_response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("company check id take: {e}")))?;
                if existing.is_none() {
                    return Err(DomainError::NotFound(format!(
                        "Компания {} не найдена",
                        company.id
                    )));
                }

                let mut collision_response = txn
                    .query(
                        "SELECT 1 FROM companies \
                         WHERE code = $code AND record::id(id) != $id LIMIT 1",
                    )
                    .bind(("code", company.code.clone()))
                    .bind(("id", company.id.to_string()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("company check code: {e}")))?;
                let collision: Option<Value> = collision_response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("company check code take: {e}")))?;
                if collision.is_some() {
                    return Err(DomainError::ValidationError(format!(
                        "Другая компания уже использует код {}",
                        company.code
                    )));
                }

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;

                let value = serde_json::to_value(company)
                    .map_err(|e| DomainError::Storage(format!("company encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("companies", company.id.to_string()))
                    .content(value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("company write: {e}")))?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_application::ports::EventStore;
    use core_domain::company::Company;
    use core_domain::event::{ActorSnapshot, Event, StreamType};
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
            stream_type: StreamType::Company,
            stream_id: company_id.to_string(),
            event_type: "company.created".to_string(),
            version: 0,
            payload: serde_json::json!({}),
            metadata: ActorSnapshot::system(),
            company_id: company_id.to_string(),
            correlation_id: "corr".to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        }
    }

    fn sample(id: Uuid, code: &str, name: &str) -> Company {
        Company {
            id,
            code: code.to_string(),
            name: name.to_string(),
            is_active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn create_get_list_round_trip() {
        let db = mem_db().await;
        let repo = SurrealCompanyRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();

        let id = Uuid::new_v4();
        repo.create(&sample(id, "acme", "ООО «Акме»"), &[empty_event(id)])
            .await
            .unwrap();

        let got = repo.get(&id).await.unwrap();
        assert_eq!(got.code, "acme");
        assert_eq!(got.name, "ООО «Акме»");
        assert!(got.is_active);

        assert_eq!(repo.list().await.unwrap().len(), 1);

        let stream = store.read_stream(StreamType::Company, &id.to_string()).await.unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].event_type, "company.created");
        assert_eq!(stream[0].version, 1);
    }

    #[tokio::test]
    async fn duplicate_code_rejected() {
        let db = mem_db().await;
        let store = crate::SurrealEventStore::new(db.clone());
        store.ensure_schema().await.unwrap();
        let repo = SurrealCompanyRepository::new(db);
        repo.ensure_schema().await.unwrap();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        repo.create(&sample(id1, "acme", "Акме"), &[empty_event(id1)])
            .await
            .unwrap();
        let err = repo.create(&sample(id2, "acme", "Другой"), &[empty_event(id2)]).await;
        assert!(matches!(err, Err(DomainError::ValidationError(_))));
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_missing_returns_not_found() {
        let db = mem_db().await;
        let repo = SurrealCompanyRepository::new(db);
        repo.ensure_schema().await.unwrap();
        let err = repo.get(&Uuid::new_v4()).await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
    }

    #[tokio::test]
    async fn update_persists_changes_and_appends_event() {
        let db = mem_db().await;
        let repo = SurrealCompanyRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();

        let id = Uuid::new_v4();
        repo.create(&sample(id, "acme", "Акме"), &[empty_event(id)])
            .await
            .unwrap();

        let updated = Company {
            name: "Акме Холдинг".to_string(),
            is_active: false,
            ..sample(id, "acme", "Акме")
        };
        repo.update(&updated, &[empty_event(id)]).await.unwrap();

        let got = repo.get(&id).await.unwrap();
        assert_eq!(got.name, "Акме Холдинг");
        assert!(!got.is_active);

        let stream = store.read_stream(StreamType::Company, &id.to_string()).await.unwrap();
        assert_eq!(stream.len(), 2);
        assert_eq!(stream[1].version, 2);
    }

    #[tokio::test]
    async fn update_missing_returns_not_found() {
        let db = mem_db().await;
        let repo = SurrealCompanyRepository::new(db);
        repo.ensure_schema().await.unwrap();
        let id = Uuid::new_v4();
        let err = repo.update(&sample(id, "ghost", "Призрак"), &[empty_event(id)]).await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
    }

    #[tokio::test]
    async fn ensure_schema_is_idempotent() {
        let db = mem_db().await;
        let repo = SurrealCompanyRepository::new(db);
        repo.ensure_schema().await.unwrap();
        repo.ensure_schema().await.unwrap();
    }
}