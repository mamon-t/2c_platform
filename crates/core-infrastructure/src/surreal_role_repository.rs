//! Хранилище ролей на базе SurrealDB (проекция «Доска»).

use core_application::ports::{BoxFuture, RoleRepository};
use core_domain::error::DomainError;
use core_domain::event::Event;
use core_domain::role::Role;
use serde_json::{json, Value};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

use crate::events::{assign_versions, with_transaction, write_events};

/// Материализованная коллекция `roles` с уникальным индексом по `(company_id, code)`.
pub struct SurrealRoleRepository {
    db: Surreal<Any>,
}

impl SurrealRoleRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт таблицу `roles` и индексы идемпотентно.
    ///
    /// В Фазе 5 индекс сменился с UNIQUE по `code` на UNIQUE по `(company_id, code)`:
    /// системные роли (`admin`, `staff`, …) присутствуют в каждой компании.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "REMOVE INDEX IF EXISTS idx_roles_code ON roles",
            "DEFINE TABLE IF NOT EXISTS roles SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_roles_company_code ON roles FIELDS company_id, code UNIQUE",
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
    "record::id(id) AS id, company_id, code, name, description, permission_policy_codes, is_system, created_at, updated_at";

impl RoleRepository for SurrealRoleRepository {
    fn create(&self, role: &Role, events: &[Event]) -> BoxFuture<'_, Result<(), DomainError>> {
        let db = self.db.clone();
        let role = role.clone();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let role = role.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        let mut response = txn
                            .query("SELECT 1 FROM roles WHERE company_id = $company_id AND code = $code LIMIT 1")
                            .bind(("company_id", role.company_id.to_string()))
                            .bind(("code", role.code.clone()))
                            .await
                            .map_err(|e| DomainError::Storage(format!("role check code: {e}")))?;
                        let existing: Option<Value> = response
                            .take(0)
                            .map_err(|e| DomainError::Storage(format!("role check code take: {e}")))?;
                        if existing.is_some() {
                            return Err(DomainError::ValidationError(format!(
                                "Роль с кодом {} уже существует в компании",
                                role.code
                            )));
                        }

                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        let value = serde_json::to_value(&role)
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
                }
            })
            .await
        })
    }

    fn get(&self, id: &Uuid) -> BoxFuture<'_, Result<Role, DomainError>> {
        let db = self.db.clone();
        let id = *id;
        Box::pin(async move {
            let mut response = db
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
        })
    }

    fn get_by_code(
        &self,
        company_id: &Uuid,
        code: &str,
    ) -> BoxFuture<'_, Result<Role, DomainError>> {
        let db = self.db.clone();
        let company_id = *company_id;
        let code = code.to_string();
        Box::pin(async move {
            let mut response = db
                .query(format!(
                    "SELECT {ROLE_FIELDS} FROM roles WHERE company_id = $company_id AND code = $code LIMIT 1"
                ))
                .bind(("company_id", company_id.to_string()))
                .bind(("code", code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("role get_by_code: {e}")))?;
            let row: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("role get_by_code take: {e}")))?;
            row.map(serde_json::from_value)
                .transpose()
                .map_err(|e| DomainError::Storage(format!("role get_by_code decode: {e}")))?
                .ok_or_else(|| {
                    DomainError::NotFound(format!("Роль {code} не найдена в компании"))
                })
        })
    }

    fn list(&self) -> BoxFuture<'_, Result<Vec<Role>, DomainError>> {
        let db = self.db.clone();
        Box::pin(async move {
            let mut response = db
                .query(format!("SELECT {ROLE_FIELDS} FROM roles ORDER BY code"))
                .await
                .map_err(|e| DomainError::Storage(format!("role list: {e}")))?;
            let rows: Vec<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("role list take: {e}")))?;
            serde_json::from_value(Value::Array(rows))
                .map_err(|e| DomainError::Storage(format!("role list decode: {e}")))
        })
    }

    fn get_policies_for_user(
        &self,
        user_id: &Uuid,
        company_id: &Uuid,
    ) -> BoxFuture<'_, Result<Vec<String>, DomainError>> {
        let db = self.db.clone();
        let user_id = *user_id;
        let company_id = *company_id;
        Box::pin(async move {
            let mut response = db
                .query("SELECT role_ids FROM users WHERE record::id(id) = $user_id LIMIT 1")
                .bind(("user_id", user_id.to_string()))
                .await
                .map_err(|e| DomainError::Storage(format!("role policies: {e}")))?;
            let row: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("role policies take: {e}")))?;
            let Some(row) = row else {
                return Err(DomainError::NotFound(format!(
                    "Пользователь {user_id} не найден"
                )));
            };
            let role_ids: Vec<String> = row
                .get("role_ids")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            if role_ids.is_empty() {
                return Ok(Vec::new());
            }

            let mut response = db
                .query(
                    "SELECT permission_policy_codes FROM roles \
                     WHERE company_id = $company_id AND record::id(id) IN $ids",
                )
                .bind(("company_id", company_id.to_string()))
                .bind(("ids", json!(role_ids)))
                .await
                .map_err(|e| DomainError::Storage(format!("role policies query: {e}")))?;
            let rows: Vec<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("role policies query take: {e}")))?;

            let mut codes: Vec<String> = rows
                .iter()
                .filter_map(|row| row.get("permission_policy_codes"))
                .filter_map(|v| v.as_array())
                .flat_map(|arr| {
                    arr.iter().filter_map(|v| v.as_str().map(str::to_string))
                })
                .collect();
            codes.sort();
            codes.dedup();
            Ok(codes)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_application::ports::EventStore;
    use core_domain::event::{ActorSnapshot, StreamType};
    use uuid::Uuid;

    async fn mem_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        db
    }

    fn empty_event(stream_id: &str, company_id: Uuid) -> Event {
        Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Role,
            stream_id: stream_id.to_string(),
            event_type: "role.created".to_string(),
            version: 0,
            payload: serde_json::json!({}),
            metadata: ActorSnapshot::system(),
            company_id: company_id.to_string(),
            correlation_id: "corr".to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        }
    }

    fn sample(company_id: Uuid, code: &str, name: &str) -> Role {
        Role {
            id: Uuid::new_v4(),
            company_id,
            code: code.to_string(),
            name: name.to_string(),
            description: String::new(),
            permission_policy_codes: vec!["test.policy".to_string()],
            is_system: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn create_get_list_round_trip() {
        let db = mem_db().await;
        let store = crate::SurrealEventStore::new(db.clone());
        store.ensure_schema().await.unwrap();
        let repo = SurrealRoleRepository::new(db);
        repo.ensure_schema().await.unwrap();

        let company_id = Uuid::new_v4();
        let role = sample(company_id, "accountant", "Бухгалтер");
        repo.create(&role, &[empty_event(&role.id.to_string(), company_id)])
            .await
            .unwrap();

        let got = repo.get(&role.id).await.unwrap();
        assert_eq!(got.code, "accountant");
        assert_eq!(got.company_id, company_id);
        assert_eq!(got.permission_policy_codes, vec!["test.policy".to_string()]);

        assert_eq!(repo.list().await.unwrap().len(), 1);

        let stream = store
            .read_stream(StreamType::Role, &role.id.to_string())
            .await
            .unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].event_type, "role.created");
        assert_eq!(stream[0].version, 1);

        let by_code = repo.get_by_code(&company_id, "accountant").await.unwrap();
        assert_eq!(by_code.id, role.id);
    }

    #[tokio::test]
    async fn duplicate_code_in_same_company_rejected_but_other_company_ok() {
        let db = mem_db().await;
        let store = crate::SurrealEventStore::new(db.clone());
        store.ensure_schema().await.unwrap();
        let repo = SurrealRoleRepository::new(db);
        repo.ensure_schema().await.unwrap();

        let company_a = Uuid::new_v4();
        let company_b = Uuid::new_v4();
        let admin_a = sample(company_a, "admin", "Админ");
        repo.create(&admin_a, &[empty_event(&admin_a.id.to_string(), company_a)])
            .await
            .unwrap();

        let err = repo
            .create(
                &sample(company_a, "admin", "Другой админ"),
                &[empty_event(&Uuid::new_v4().to_string(), company_a)],
            )
            .await;
        assert!(matches!(err, Err(DomainError::ValidationError(_))));

        let admin_b = sample(company_b, "admin", "Админ B");
        repo.create(&admin_b, &[empty_event(&admin_b.id.to_string(), company_b)])
            .await
            .unwrap();
        assert_eq!(repo.list().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn get_policies_for_user_collects_and_dedups() {
        let db = mem_db().await;
        let users = crate::SurrealUserRepository::new(db.clone());
        users.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db.clone());
        store.ensure_schema().await.unwrap();
        let repo = SurrealRoleRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();

        let company_id = Uuid::new_v4();
        let admin = sample(company_id, "admin", "Админ");
        repo.create(&admin, &[empty_event(&admin.id.to_string(), company_id)])
            .await
            .unwrap();
        let staff = sample(company_id, "staff", "Сотрудник");
        repo.create(&staff, &[empty_event(&staff.id.to_string(), company_id)])
            .await
            .unwrap();

        let user_id = Uuid::new_v4();
        let person = core_domain::user::Person {
            id: user_id,
            user_id,
            last_name: "Тестов".to_string(),
            first_name: "Тест".to_string(),
            middle_name: None,
            display_name: "Тест Тестов".to_string(),
        };
        let user = core_domain::user::User {
            id: user_id,
            login: "tester".to_string(),
            password_hash: "x".to_string(),
            status: core_domain::user::UserStatus::Active,
            role_ids: vec![admin.id.to_string(), staff.id.to_string()],
            failed_login_count: 0,
            locked_until: None,
            must_change_password: false,
            locale: "ru-RU".to_string(),
            timezone: "Europe/Moscow".to_string(),
            person_id: Some(user_id),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let user_event = Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::User,
            stream_id: user_id.to_string(),
            event_type: "user.created".to_string(),
            version: 0,
            payload: serde_json::to_value(&user).unwrap(),
            metadata: ActorSnapshot::system(),
            company_id: company_id.to_string(),
            correlation_id: "c".to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        };
        use core_application::ports::UserRepository;
        users.create(&user, &person, &[user_event]).await.unwrap();

        let codes = repo
            .get_policies_for_user(&user_id, &company_id)
            .await
            .unwrap();
        assert_eq!(codes, vec!["test.policy".to_string()]);
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
    async fn ensure_schema_is_idempotent_and_migrates_index() {
        let db = mem_db().await;
        let repo = SurrealRoleRepository::new(db);
        repo.ensure_schema().await.unwrap();
        repo.ensure_schema().await.unwrap();
    }
}