//! Изолированное KV-хранилище модуля (host-функции `kv_*`, подфаза 8a).
//!
//! Коллекция `module_kv`. Ключ различается по компании, коду модуля и
//! собственному ключу; уникальность обеспечивается компаунд-индексом и
//! составным record id. Значение хранится как произвольный JSON.

use core_domain::error::DomainError;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

use crate::events::with_transaction;

/// KV-хранилище module — хранилище настроек и промежуточных данных модуля,
/// изолированное по `(company_id, module_code)`.
#[derive(Clone)]
pub struct ModuleKv {
    db: Surreal<Any>,
}

impl ModuleKv {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт коллекцию `module_kv` и индексы идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS module_kv SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_module_kv_key \
                ON module_kv FIELDS company_id, module_code, key UNIQUE",
            "DEFINE INDEX IF NOT EXISTS idx_module_kv_module \
                ON module_kv FIELDS company_id, module_code",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("module_kv ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("module_kv ensure_schema take: {e}")))?;
        }
        Ok(())
    }

    /// Составляет составной id записи `{company}:{module}:{key}`.
    fn record_id(company_id: &str, module_code: &str, key: &str) -> String {
        format!("{company_id}:{module_code}:{key}")
    }

    /// Сохраняет значение по ключу, перезаписывая существующее.
    pub async fn put(
        &self,
        company_id: &str,
        module_code: &str,
        key: &str,
        value: Value,
    ) -> Result<(), DomainError> {
        let record = serde_json::json!({
            "company_id": company_id,
            "module_code": module_code,
            "key": key,
            "value": value,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });
        let id = Self::record_id(company_id, module_code, key);
        let _: Option<surrealdb::types::Value> = self
            .db
            .upsert(("module_kv", id))
            .content(record)
            .await
            .map_err(|e| DomainError::Storage(format!("module_kv put: {e}")))?;
        Ok(())
    }

    /// Сохраняет значение по ключу, только если ключ ещё не существует.
    /// Возвращает `true`, если значение вставлено, иначе `false`.
    pub async fn put_if_absent(
        &self,
        company_id: &str,
        module_code: &str,
        key: &str,
        value: Value,
    ) -> Result<bool, DomainError> {
        let db = self.db.clone();
        with_transaction(&db, |txn| {
            let company_id = company_id.to_string();
            let module_code = module_code.to_string();
            let key = key.to_string();
            let value = value.clone();
            let id = Self::record_id(&company_id, &module_code, &key);
            let updated_at = chrono::Utc::now().to_rfc3339();
            async move {
                let outcome: Result<bool, DomainError> = async {
                    let mut response = txn
                        .query(
                            "SELECT `value` FROM module_kv \
                             WHERE company_id = $c AND module_code = $m AND key = $k LIMIT 1",
                        )
                        .bind(("c", company_id.clone()))
                        .bind(("m", module_code.clone()))
                        .bind(("k", key.clone()))
                        .await
                        .map_err(|e| DomainError::Storage(format!("module_kv put_if_absent select: {e}")))?;
                    let rows: Vec<Value> = response
                        .take(0)
                        .map_err(|e| DomainError::Storage(format!("module_kv put_if_absent take: {e}")))?;

                    if !rows.is_empty() {
                        return Ok(false);
                    }

                    let record = serde_json::json!({
                        "company_id": company_id,
                        "module_code": module_code,
                        "key": key,
                        "value": value,
                        "updated_at": updated_at,
                    });
                    let _: Option<surrealdb::types::Value> = txn
                        .upsert(("module_kv", id))
                        .content(record)
                        .await
                        .map_err(|e| DomainError::Storage(format!("module_kv put_if_absent insert: {e}")))?;
                    Ok(true)
                }
                .await;
                (txn, outcome)
            }
        })
        .await
    }

    /// Получает значение по ключу; `Ok(None)`, если ключ отсутствует.
    pub async fn get(
        &self,
        company_id: &str,
        module_code: &str,
        key: &str,
    ) -> Result<Option<Value>, DomainError> {
        let mut response = self
            .db
            .query(
                "SELECT `value` FROM module_kv \
                 WHERE company_id = $c AND module_code = $m AND key = $k LIMIT 1",
            )
            .bind(("c", company_id.to_string()))
            .bind(("m", module_code.to_string()))
            .bind(("k", key.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("module_kv get: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("module_kv get take: {e}")))?;
        Ok(rows
            .into_iter()
            .next()
            .and_then(|row| row.get("value").cloned()))
    }

    /// Перечисляет пары `(key, value)` с префиксом ключа по возрастанию ключа.
    pub async fn list(
        &self,
        company_id: &str,
        module_code: &str,
        prefix: &str,
    ) -> Result<Vec<(String, Value)>, DomainError> {
        let mut response = self
            .db
            .query(
                "SELECT key, `value` FROM module_kv \
                 WHERE company_id = $c AND module_code = $m AND string::starts_with(key, $prefix) \
                 ORDER BY key",
            )
            .bind(("c", company_id.to_string()))
            .bind(("m", module_code.to_string()))
            .bind(("prefix", prefix.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("module_kv list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("module_kv list take: {e}")))?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let ok_key = row
                .get("key")
                .and_then(Value::as_str)
                .map(str::to_string);
            let ok_value = row.get("value").cloned();
            if let (Some(key), Some(value)) = (ok_key, ok_value) {
                out.push((key, value));
            }
        }
        Ok(out)
    }

    /// Удаляет ключ. Возвращает `true`, если ключ существовал.
    pub async fn delete(
        &self,
        company_id: &str,
        module_code: &str,
        key: &str,
    ) -> Result<bool, DomainError> {
        let mut response = self
            .db
            .query(
                "DELETE module_kv \
                 WHERE company_id = $c AND module_code = $m AND key = $k RETURN BEFORE",
            )
            .bind(("c", company_id.to_string()))
            .bind(("m", module_code.to_string()))
            .bind(("k", key.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("module_kv delete: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("module_kv delete take: {e}")))?;
        Ok(!rows.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db
    }

    #[tokio::test]
    async fn kv_roundtrip_and_list() {
        let kv = ModuleKv::new(test_db().await);
        kv.ensure_schema().await.unwrap();

        kv.put("c1", "stock", "settings.theme", Value::String("dark".into()))
            .await
            .unwrap();
        kv.put("c1", "stock", "counter", Value::from(42))
            .await
            .unwrap();
        kv.put("c2", "stock", "settings.theme", Value::String("light".into()))
            .await
            .unwrap();

        assert_eq!(
            kv.get("c1", "stock", "settings.theme").await.unwrap(),
            Some(Value::String("dark".into()))
        );
        assert_eq!(
            kv.get("c2", "stock", "settings.theme").await.unwrap(),
            Some(Value::String("light".into()))
        );
        assert_eq!(kv.get("c1", "trade", "settings.theme").await.unwrap(), None);

        let rows = kv.list("c1", "stock", "set").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "settings.theme");

        // Изоляция по модулю и компании соблюдается.
        assert_eq!(kv.list("c1", "stock", "").await.unwrap().len(), 2);
        assert_eq!(kv.list("c1", "trade", "").await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn put_if_absent_updates_nothing() {
        let kv = ModuleKv::new(test_db().await);
        kv.ensure_schema().await.unwrap();

        let inserted = kv
            .put_if_absent("c1", "stock", "lock", Value::from(true))
            .await
            .unwrap();
        assert!(inserted);
        let again = kv
            .put_if_absent("c1", "stock", "lock", Value::from(false))
            .await
            .unwrap();
        assert!(!again);
        assert_eq!(
            kv.get("c1", "stock", "lock").await.unwrap(),
            Some(Value::from(true))
        );
    }

    #[tokio::test]
    async fn delete_removes_key() {
        let kv = ModuleKv::new(test_db().await);
        kv.ensure_schema().await.unwrap();
        kv.put("c1", "stock", "a", Value::from(1)).await.unwrap();

        assert!(kv.delete("c1", "stock", "a").await.unwrap());
        assert!(!kv.delete("c1", "stock", "a").await.unwrap());
        assert_eq!(kv.get("c1", "stock", "a").await.unwrap(), None);
    }
}