//! Хранилище политик доступа на базе SurrealDB (коллекция `permission_policies`).
//!
//! Часть RBAC-подсистемы (ТЗ v3.1, Приложение №7): политики описывают
//! разрешения и запреты на действия. `PermissionManager` читает их через
//! `get_by_codes`, а `seed_system_roles_and_policies` регистрирует системные
//! политики с ensure-семантикой по коду.

use core_application::ports::{BoxFuture, PermissionPolicyRepository};
use core_domain::error::DomainError;
use core_domain::permission::PermissionPolicy;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

const POLICY_FIELDS: &str =
    "record::id(id) AS id, code, name, description, scope_type, entity_type, actions, \
     record_access, deny, priority, module_code, is_system, created_at, updated_at";

/// Хранилище политик доступа в коллекции `permission_policies`.
pub struct SurrealPermissionPolicyRepository {
    db: Surreal<Any>,
}

impl SurrealPermissionPolicyRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт коллекцию `permission_policies` и уникальный индекс по `code`.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS permission_policies SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_policies_code \
                ON permission_policies FIELDS code UNIQUE",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("permission_policies ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("permission_policies schema take: {e}")))?;
        }
        Ok(())
    }
}

impl PermissionPolicyRepository for SurrealPermissionPolicyRepository {
    fn upsert(&self, policy: &PermissionPolicy) -> BoxFuture<'_, Result<(), DomainError>> {
        // Ensure-семантика по коду (см. порт): политика вставляется только если её
        // нет; существующая политика не перезаписывается (системные политики стабильны).
        let db = self.db.clone();
        let policy = policy.clone();
        Box::pin(async move {
            let mut response = db
                .query("SELECT 1 FROM permission_policies WHERE code = $code LIMIT 1")
                .bind(("code", policy.code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("policy exists check: {e}")))?;
            let existing: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("policy exists check take: {e}")))?;
            if existing.is_some() {
                return Ok(());
            }

            let value = serde_json::to_value(&policy)
                .map_err(|e| DomainError::Storage(format!("policy encode: {e}")))?;
            let _: Option<surrealdb::types::Value> = db
                .create(("permission_policies", policy.id.to_string()))
                .content(value)
                .await
                .map_err(|e| DomainError::Storage(format!("policy create: {e}")))?;
            Ok(())
        })
    }

    fn get_by_code(&self, code: &str) -> BoxFuture<'_, Result<PermissionPolicy, DomainError>> {
        let db = self.db.clone();
        let code = code.to_string();
        Box::pin(async move {
            let mut response = db
                .query(format!(
                    "SELECT {POLICY_FIELDS} FROM permission_policies WHERE code = $code LIMIT 1"
                ))
                .bind(("code", code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("policy get_by_code: {e}")))?;
            let row: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("policy get_by_code take: {e}")))?;
            row.map(serde_json::from_value)
                .transpose()
                .map_err(|e| DomainError::Storage(format!("policy get_by_code decode: {e}")))?
                .ok_or_else(|| DomainError::NotFound(format!("Политика {code} не найдена")))
        })
    }

    fn get_by_codes(
        &self,
        codes: &[String],
    ) -> BoxFuture<'_, Result<Vec<PermissionPolicy>, DomainError>> {
        let db = self.db.clone();
        let codes = codes.to_vec();
        Box::pin(async move {
            if codes.is_empty() {
                return Ok(Vec::new());
            }
            let mut response = db
                .query(format!(
                    "SELECT {POLICY_FIELDS} FROM permission_policies WHERE code IN $codes"
                ))
                .bind(("codes", Value::Array(codes.into_iter().map(Value::String).collect())))
                .await
                .map_err(|e| DomainError::Storage(format!("policy get_by_codes: {e}")))?;
            let rows: Vec<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("policy get_by_codes take: {e}")))?;
            serde_json::from_value(Value::Array(rows))
                .map_err(|e| DomainError::Storage(format!("policy get_by_codes decode: {e}")))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_domain::permission::{PermissionScopeType, RecordAccessLevel};
    use uuid::Uuid;

    async fn mem_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        db
    }

    fn sample(code: &str) -> PermissionPolicy {
        let now = chrono::Utc::now();
        PermissionPolicy {
            id: Uuid::new_v4(),
            code: code.to_string(),
            name: code.to_string(),
            description: None,
            scope_type: PermissionScopeType::Platform,
            entity_type: Some("invoice".to_string()),
            actions: vec!["create".to_string(), "read".to_string()],
            record_access: RecordAccessLevel::ByCompany,
            deny: false,
            priority: 0,
            module_code: None,
            is_system: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn upsert_get_by_code_round_trip() {
        let db = mem_db().await;
        let repo = SurrealPermissionPolicyRepository::new(db);
        repo.ensure_schema().await.unwrap();

        let policy = sample("invoice.create");
        repo.upsert(&policy).await.unwrap();

        let got = repo.get_by_code("invoice.create").await.unwrap();
        assert_eq!(got.id, policy.id);
        assert_eq!(got.actions, vec!["create".to_string(), "read".to_string()]);
        assert_eq!(got.scope_type, PermissionScopeType::Platform);
        assert_eq!(got.record_access, RecordAccessLevel::ByCompany);
    }

    #[tokio::test]
    async fn upsert_is_idempotent_and_keeps_existing_policy() {
        let db = mem_db().await;
        let repo = SurrealPermissionPolicyRepository::new(db);
        repo.ensure_schema().await.unwrap();

        let policy = sample("invoice.disable");
        repo.upsert(&policy).await.unwrap();
        repo.upsert(&policy).await.unwrap();

        let got = repo.get_by_code("invoice.disable").await.unwrap();
        assert_eq!(got.id, policy.id);

        // Повторная вставка существующего кода с новым id не перезаписывает политику
        // (ensure-семантика по коду): изменённая «версия» игнорируется.
        let other = sample("invoice.disable");
        repo.upsert(&other).await.unwrap();
        let got = repo.get_by_code("invoice.disable").await.unwrap();
        assert_eq!(got.id, policy.id);
    }

    #[tokio::test]
    async fn get_by_codes_returns_matching_and_skips_missing() {
        let db = mem_db().await;
        let repo = SurrealPermissionPolicyRepository::new(db);
        repo.ensure_schema().await.unwrap();

        repo.upsert(&sample("a1")).await.unwrap();
        repo.upsert(&sample("a2")).await.unwrap();

        let got = repo
            .get_by_codes(&["a1".to_string(), "missing".to_string()])
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].code, "a1");

        let empty = repo.get_by_codes(&[]).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn get_by_code_missing_returns_not_found() {
        let db = mem_db().await;
        let repo = SurrealPermissionPolicyRepository::new(db);
        repo.ensure_schema().await.unwrap();
        let err = repo.get_by_code("nope").await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
    }

    #[tokio::test]
    async fn ensure_schema_is_idempotent() {
        let db = mem_db().await;
        let repo = SurrealPermissionPolicyRepository::new(db);
        repo.ensure_schema().await.unwrap();
        repo.ensure_schema().await.unwrap();
    }
}