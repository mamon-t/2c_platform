//! Хранилище операционного аудита на базе SurrealDB (коллекция `audit_log`).
//!
//! Отдельная подсистема, не связанная с Event Store (Труба, коллекция
//! `events`): здесь хранятся действия пользователей и системы для
//! безопасности, compliance и отладки (раздел 8.5 и Приложение №6 ТЗ v3.1).

use core_application::ports::{AuditRepository, BoxFuture};
use core_domain::audit::{AuditEntry, AuditFilter};
use core_domain::error::DomainError;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

use crate::events::with_transaction;

const AUDIT_FIELDS: &str = "record::id(id) AS id, action, actor, target, `result`, details, \
    ip_address, user_agent, company_id, timestamp";

/// Хранилище записей операционного аудита.
pub struct SurrealAuditRepository {
    db: Surreal<Any>,
}

impl SurrealAuditRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт коллекцию `audit_log` и пять требуемых индексов идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS audit_log SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_audit_action_timestamp \
                ON audit_log FIELDS action, timestamp",
            "DEFINE INDEX IF NOT EXISTS idx_audit_actor_timestamp \
                ON audit_log FIELDS actor.user_id, timestamp",
            "DEFINE INDEX IF NOT EXISTS idx_audit_target_timestamp \
                ON audit_log FIELDS target.entity_type, target.entity_id, timestamp",
            "DEFINE INDEX IF NOT EXISTS idx_audit_company_timestamp \
                ON audit_log FIELDS company_id, timestamp",
            "DEFINE INDEX IF NOT EXISTS idx_audit_result_timestamp \
                ON audit_log FIELDS `result`, timestamp",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("audit_log ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("audit_log ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

/// Кодирует запись аудита для вставки, убирая отдельное поле `id`
/// (идентификатор записи задаётся самим upsert).
fn audit_record(entry: &AuditEntry) -> Result<Value, DomainError> {
    let mut value = serde_json::to_value(entry)
        .map_err(|e| DomainError::Storage(format!("audit encode: {e}")))?;
    value
        .as_object_mut()
        .ok_or_else(|| DomainError::Storage("audit entry is not an object".into()))?
        .remove("id");
    Ok(value)
}

fn decode_rows(rows: Vec<Value>) -> Result<Vec<AuditEntry>, DomainError> {
    serde_json::from_value(Value::Array(rows))
        .map_err(|e| DomainError::Storage(format!("audit list decode: {e}")))
}

impl AuditRepository for SurrealAuditRepository {
    fn log(&self, entry: AuditEntry) -> BoxFuture<'_, Result<(), DomainError>> {
        let db = self.db.clone();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let entry = entry.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        let value = audit_record(&entry)?;
                        let _: Option<surrealdb::types::Value> = txn
                            .upsert(("audit_log", entry.id.to_string()))
                            .content(value)
                            .await
                            .map_err(|e| DomainError::Storage(format!("audit write: {e}")))?;
                        Ok(())
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn query(&self, filter: AuditFilter) -> BoxFuture<'_, Result<Vec<AuditEntry>, DomainError>> {
        let db = self.db.clone();
        Box::pin(async move {
            let (where_clause, limit_clause, binds) = build_query(&filter);
            let sql = format!(
                "SELECT {AUDIT_FIELDS} FROM audit_log \
                 {where_clause} ORDER BY timestamp DESC {limit_clause}"
            );
            let mut query = db.query(sql);
            for (name, value) in binds {
                query = query.bind((name, value));
            }
            let mut response = query
                .await
                .map_err(|e| DomainError::Storage(format!("audit query: {e}")))?;
            let rows: Vec<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("audit query take: {e}")))?;
            decode_rows(rows)
        })
    }
}

/// Строит WHERE/предел и параметры запроса выборки по фильтру. Все значения
/// передаются параметрами (биндами), без интерполяции пользовательских данных
/// в строку запроса.
fn build_query(filter: &AuditFilter) -> (String, String, Vec<(&str, Value)>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut binds: Vec<(&str, Value)> = Vec::new();

    if let Some(action) = &filter.action {
        conditions.push("action = $action".to_string());
        binds.push(("action", Value::String(action.clone())));
    }
    if let Some(actor_user_id) = filter.actor_user_id {
        conditions.push("actor.user_id = $actor_user_id".to_string());
        binds.push(("actor_user_id", Value::String(actor_user_id.to_string())));
    }
    if let Some(entity_type) = &filter.target_entity_type {
        conditions.push("target.entity_type = $target_entity_type".to_string());
        binds.push(("target_entity_type", Value::String(entity_type.clone())));
    }
    if let Some(entity_id) = filter.target_entity_id {
        conditions.push("target.entity_id = $target_entity_id".to_string());
        binds.push(("target_entity_id", Value::String(entity_id.to_string())));
    }
    if let Some(company_id) = filter.company_id {
        conditions.push("company_id = $company_id".to_string());
        binds.push(("company_id", Value::String(company_id.to_string())));
    }
    match filter.result_success {
        Some(true) => {
            conditions.push("`result` = $result".to_string());
            binds.push(("result", Value::String("success".to_string())));
        }
        Some(false) => {
            // Успех сохраняется строкой "success"; любое другое значение — неуспех.
            conditions.push("`result` != $result".to_string());
            binds.push(("result", Value::String("success".to_string())));
        }
        None => {}
    }
    if let Some(from) = filter.from {
        conditions.push("timestamp >= $from".to_string());
        binds.push(("from", Value::String(from.to_rfc3339())));
    }
    if let Some(to) = filter.to {
        conditions.push("timestamp <= $to".to_string());
        binds.push(("to", Value::String(to.to_rfc3339())));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let limit_clause = match filter.limit {
        Some(limit) => format!("LIMIT {limit}"),
        None => String::new(),
    };
    (where_clause, limit_clause, binds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use core_domain::audit::AuditResult;
    use core_domain::event::ActorSnapshot;
    use uuid::Uuid;

    async fn mem_repo() -> SurrealAuditRepository {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        let repo = SurrealAuditRepository::new(db);
        repo.ensure_schema().await.unwrap();
        repo
    }

    fn ts(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn entry(
        action: &str,
        actor_user_id: Option<Uuid>,
        company_id: Option<Uuid>,
        timestamp: DateTime<Utc>,
        result: AuditResult,
    ) -> AuditEntry {
        AuditEntry {
            id: Uuid::new_v4(),
            action: action.to_string(),
            actor: ActorSnapshot {
                user_id: actor_user_id,
                login: "user".to_string(),
                full_name: "Пользователь".to_string(),
                position: None,
                company_id,
                ip_address: None,
            },
            target: None,
            result,
            details: None,
            ip_address: None,
            user_agent: None,
            company_id,
            timestamp,
        }
    }

    #[tokio::test]
    async fn log_round_trip_without_filters() {
        let repo = mem_repo().await;
        let record = entry("user.login", None, None, Utc::now(), AuditResult::Success);
        repo.log(record.clone()).await.unwrap();

        let rows = repo.query(AuditFilter::default()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, record.id);
        assert_eq!(rows[0].action, "user.login");
        assert_eq!(rows[0].actor.login, "user");
        assert_eq!(rows[0].result, AuditResult::Success);
    }

    #[tokio::test]
    async fn query_filters_by_action() {
        let repo = mem_repo().await;
        for action in ["a.one", "a.two", "a.three"] {
            repo.log(entry(action, None, None, Utc::now(), AuditResult::Success))
                .await
                .unwrap();
        }

        let rows = repo
            .query(AuditFilter {
                action: Some("a.two".to_string()),
                ..AuditFilter::default()
            })
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "a.two");
    }

    #[tokio::test]
    async fn query_filters_by_company() {
        let repo = mem_repo().await;
        let c1 = Uuid::new_v4();
        let c2 = Uuid::new_v4();
        repo.log(entry("a.one", None, Some(c1), Utc::now(), AuditResult::Success))
            .await
            .unwrap();
        repo.log(entry("a.two", None, Some(c2), Utc::now(), AuditResult::Success))
            .await
            .unwrap();

        let rows = repo
            .query(AuditFilter {
                company_id: Some(c1),
                ..AuditFilter::default()
            })
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].company_id, Some(c1));
    }

    #[tokio::test]
    async fn query_filters_by_time_window() {
        let repo = mem_repo().await;
        repo.log(entry("a.one", None, None, ts("2026-01-01T10:00:00Z"), AuditResult::Success))
            .await
            .unwrap();
        repo.log(entry("a.two", None, None, ts("2026-01-02T10:00:00Z"), AuditResult::Success))
            .await
            .unwrap();
        repo.log(entry("a.three", None, None, ts("2026-01-03T10:00:00Z"), AuditResult::Success))
            .await
            .unwrap();

        let rows = repo
            .query(AuditFilter {
                from: Some(ts("2026-01-02T00:00:00Z")),
                to: Some(ts("2026-01-02T23:59:59Z")),
                ..AuditFilter::default()
            })
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "a.two");
    }

    #[tokio::test]
    async fn query_filters_by_actor_user_id() {
        let repo = mem_repo().await;
        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();
        repo.log(entry("a.one", Some(u1), None, Utc::now(), AuditResult::Success))
            .await
            .unwrap();
        repo.log(entry("a.two", Some(u2), None, Utc::now(), AuditResult::Success))
            .await
            .unwrap();

        let rows = repo
            .query(AuditFilter {
                actor_user_id: Some(u1),
                ..AuditFilter::default()
            })
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].actor.user_id, Some(u1));
    }

    #[tokio::test]
    async fn query_filters_by_result() {
        let repo = mem_repo().await;
        repo.log(entry("a.one", None, None, Utc::now(), AuditResult::Success))
            .await
            .unwrap();
        repo.log(entry(
            "a.two",
            None,
            None,
            Utc::now(),
            AuditResult::Failure {
                reason: "test".to_string(),
            },
        ))
        .await
        .unwrap();

        let failed = repo
            .query(AuditFilter {
                result_success: Some(false),
                ..AuditFilter::default()
            })
            .await
            .unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].action, "a.two");

        let succeeding = repo
            .query(AuditFilter {
                result_success: Some(true),
                ..AuditFilter::default()
            })
            .await
            .unwrap();
        assert_eq!(succeeding.len(), 1);
        assert_eq!(succeeding[0].action, "a.one");
    }

    #[tokio::test]
    async fn query_orders_by_timestamp_descending() {
        let repo = mem_repo().await;
        repo.log(entry("a.one", None, None, ts("2026-01-01T10:00:00Z"), AuditResult::Success))
            .await
            .unwrap();
        repo.log(entry("a.two", None, None, ts("2026-01-02T10:00:00Z"), AuditResult::Success))
            .await
            .unwrap();
        repo.log(entry("a.three", None, None, ts("2026-01-03T10:00:00Z"), AuditResult::Success))
            .await
            .unwrap();

        let rows = repo.query(AuditFilter::default()).await.unwrap();
        let actions: Vec<&str> = rows.iter().map(|r| r.action.as_str()).collect();
        assert_eq!(actions, vec!["a.three", "a.two", "a.one"]);
    }

    #[tokio::test]
    async fn query_respects_limit() {
        let repo = mem_repo().await;
        for n in 1..=5 {
            repo.log(entry(
                &format!("action.{n}"),
                None,
                None,
                ts(&format!("2026-01-0{n}T10:00:00Z")),
                AuditResult::Success,
            ))
            .await
            .unwrap();
        }

        let rows = repo
            .query(AuditFilter {
                limit: Some(2),
                ..AuditFilter::default()
            })
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].action, "action.5");
        assert_eq!(rows[1].action, "action.4");
    }
}