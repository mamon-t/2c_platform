//! Хранилище скриптов Rhai (`scripts`) на базе SurrealDB («Доска»).
//!
//! Таблица SCHEMALESS, записи идентифицируются через `scripts:<uuid>`.
//! Уникальность кода гарантируется индексом по `(code, company_key)`, где
//! `company_key` — строка компании или `"global"` для глобальных скриптов
//! (`None`): так глобальные скрипты не коллизируют с NULL-значением.
//! Каждый изменяющий метод транзакционно продвигает Трубу (события
//! `script.*`) и Доску вместе.

use core_application::ports::{BoxFuture, ScriptRepository};
use core_domain::error::DomainError;
use core_domain::event::Event;
use core_domain::script::Script;
use serde_json::{json, Value};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

use crate::events::{assign_versions, with_transaction, write_events};

/// Ключ-маркер глобального скрипта в UNIQUE-индексе.
pub const GLOBAL_KEY: &str = "global";

fn company_key(company_id: &Option<Uuid>) -> String {
    company_id
        .map(|uuid| uuid.to_string())
        .unwrap_or_else(|| GLOBAL_KEY.to_string())
}

/// Материализованная коллекция `scripts`.
pub struct SurrealScriptRepository {
    db: Surreal<Any>,
}

impl SurrealScriptRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт таблицу и уникальный индекс идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS scripts SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_scripts_code_company ON scripts FIELDS code, company_key UNIQUE",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("scripts ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("scripts ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

const SCRIPT_FIELDS: &str =
    "record::id(id) AS id, code, name, script_type, source, company_id, module_code, entity_type, is_active, created_at, updated_at";

fn encode_with_key(script: &Script) -> Result<Value, DomainError> {
    let mut value = serde_json::to_value(script)
        .map_err(|e| DomainError::Storage(format!("script encode: {e}")))?;
    value["company_key"] = json!(company_key(&script.company_id));
    Ok(value)
}

fn decode_script(row: Value) -> Result<Script, DomainError> {
    serde_json::from_value(row)
        .map_err(|e| DomainError::Storage(format!("script decode: {e}")))
}

impl ScriptRepository for SurrealScriptRepository {
    fn create<'a>(
        &'a self,
        script: &'a Script,
        events: &'a [Event],
    ) -> BoxFuture<'a, Result<Script, DomainError>> {
        let db = self.db.clone();
        let script = script.clone();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let script = script.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<Script, DomainError> = async {
                        let mut response = txn
                            .query(
                                "SELECT id FROM scripts \
                                 WHERE code = $code AND company_key = $key LIMIT 1",
                            )
                            .bind(("code", script.code.clone()))
                            .bind(("key", company_key(&script.company_id)))
                            .await
                            .map_err(|e| DomainError::Storage(format!("script dup check: {e}")))?;
                        let existing: Option<Value> = response
                            .take(0)
                            .map_err(|e| DomainError::Storage(format!("script dup take: {e}")))?;
                        if existing.is_some() {
                            return Err(DomainError::ValidationError(format!(
                                "Скрипт с кодом {} уже существует",
                                script.code
                            )));
                        }

                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        let value = encode_with_key(&script)?;
                        let _: Option<surrealdb::types::Value> = txn
                            .create(("scripts", script.id.to_string()))
                            .content(value)
                            .await
                            .map_err(|e| DomainError::Storage(format!("script write: {e}")))?;
                        Ok(script)
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn get<'a>(&'a self, id: &'a Uuid) -> BoxFuture<'a, Result<Option<Script>, DomainError>> {
        let db = self.db.clone();
        let id = *id;
        Box::pin(async move {
            let mut response = db
                .query(format!(
                    "SELECT {SCRIPT_FIELDS} FROM scripts WHERE record::id(id) = $id LIMIT 1"
                ))
                .bind(("id", id.to_string()))
                .await
                .map_err(|e| DomainError::Storage(format!("script get: {e}")))?;
            let row: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("script get take: {e}")))?;
            row.map(decode_script).transpose()
        })
    }

    fn get_by_code<'a>(
        &'a self,
        code: &'a str,
        company_id: Option<&'a Uuid>,
    ) -> BoxFuture<'a, Result<Option<Script>, DomainError>> {
        let db = self.db.clone();
        let code = code.to_string();
        let company_id = company_id.copied();
        Box::pin(async move {
            let mut response = db
                .query(
                    "SELECT {SCRIPT_FIELDS} FROM scripts \
                     WHERE code = $code AND company_key = $key LIMIT 1"
                        .replace("{SCRIPT_FIELDS}", SCRIPT_FIELDS),
                )
                .bind(("code", code.clone()))
                .bind(("key", company_key(&company_id)))
                .await
                .map_err(|e| DomainError::Storage(format!("script get_by_code: {e}")))?;
            let row: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("script get_by_code take: {e}")))?;
            row.map(decode_script).transpose()
        })
    }

    fn list<'a>(
        &'a self,
        company_id: Option<&'a Uuid>,
    ) -> BoxFuture<'a, Result<Vec<Script>, DomainError>> {
        let db = self.db.clone();
        let company_id = company_id.copied();
        Box::pin(async move {
            let query = match &company_id {
                Some(_) => format!(
                    "SELECT {SCRIPT_FIELDS} FROM scripts \
                     WHERE company_key = $key OR company_key = '{GLOBAL_KEY}' ORDER BY code"
                ),
                None => format!(
                    "SELECT {SCRIPT_FIELDS} FROM scripts \
                     WHERE company_key = '{GLOBAL_KEY}' ORDER BY code"
                ),
            };
            let mut response = db
                .query(query)
                .bind(("key", company_key(&company_id)))
                .await
                .map_err(|e| DomainError::Storage(format!("script list: {e}")))?;
            let rows: Vec<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("script list take: {e}")))?;
            rows.into_iter()
                .map(decode_script)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn update<'a>(
        &'a self,
        script: &'a Script,
        events: &'a [Event],
    ) -> BoxFuture<'a, Result<Script, DomainError>> {
        let db = self.db.clone();
        let script = script.clone();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let script = script.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<Script, DomainError> = async {
                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        let value = encode_with_key(&script)?;
                        let _: Option<surrealdb::types::Value> = txn
                            .upsert(("scripts", script.id.to_string()))
                            .content(value)
                            .await
                            .map_err(|e| DomainError::Storage(format!("script update: {e}")))?;
                        Ok(script)
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn delete<'a>(
        &'a self,
        id: &'a Uuid,
        events: &'a [Event],
    ) -> BoxFuture<'a, Result<(), DomainError>> {
        let db = self.db.clone();
        let id = *id;
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let events = events.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        let _ = txn
                            .query("DELETE FROM scripts WHERE record::id(id) = $id")
                            .bind(("id", id.to_string()))
                            .await
                            .map_err(|e| DomainError::Storage(format!("script delete: {e}")))?;
                        Ok(())
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn delete_by_module<'a>(
        &'a self,
        module_code: &'a str,
        events: &'a [Event],
    ) -> BoxFuture<'a, Result<(), DomainError>> {
        let db = self.db.clone();
        let module_code = module_code.to_string();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let module_code = module_code.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        txn.query("DELETE FROM scripts WHERE module_code = $module_code")
                            .bind(("module_code", module_code))
                            .await
                            .map_err(|e| {
                                DomainError::Storage(format!("script delete_by_module: {e}"))
                            })?;
                        Ok(())
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SurrealEventStore;
    use core_application::ports::EventStore;
    use core_domain::event::{ActorSnapshot, StreamType};
    use core_domain::script::ScriptType;

    async fn mem_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        db
    }

    async fn repo(db: Surreal<Any>) -> (SurrealEventStore, SurrealScriptRepository) {
        let store = SurrealEventStore::new(db.clone());
        store.ensure_schema().await.unwrap();
        let repo = SurrealScriptRepository::new(db);
        repo.ensure_schema().await.unwrap();
        (store, repo)
    }

    fn event(stream_id: &str, event_type: &str, payload: Value) -> Event {
        Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Script,
            stream_id: stream_id.to_string(),
            event_type: event_type.to_string(),
            version: 0,
            payload,
            metadata: ActorSnapshot::system(),
            company_id: "test".to_string(),
            correlation_id: "corr".to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        }
    }

    fn sample(code: &str, company_id: Option<Uuid>) -> Script {
        Script {
            id: Uuid::new_v4(),
            code: code.to_string(),
            name: format!("Скрипт {code}"),
            script_type: ScriptType::Formula,
            source: "ctx.object.amount * 2".to_string(),
            company_id,
            module_code: None,
            entity_type: None,
            is_active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn create_get_round_trip_and_stream() {
        let db = mem_db().await;
        let (store, repo) = repo(db).await;
        let s = sample("price.double", None);
        let e = event(&s.id.to_string(), "script.created", json!({ "code": s.code }));
        let saved = repo.create(&s, std::slice::from_ref(&e)).await.unwrap();
        assert_eq!(saved.code, "price.double");
        assert_eq!(saved.company_id, None);

        let got = repo.get(&s.id).await.unwrap().unwrap();
        assert_eq!(got.source, "ctx.object.amount * 2");
        assert_eq!(got.script_type, ScriptType::Formula);

        let stream = store.read_stream(StreamType::Script, &s.id.to_string()).await.unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].event_type, "script.created");
        assert_eq!(stream[0].version, 1);
    }

    #[tokio::test]
    async fn duplicate_code_in_company_rejected() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let company = Uuid::new_v4();
        repo.create(
            &sample("dup", Some(company)),
            &[event("a", "script.created", json!({}))],
        )
        .await
        .unwrap();
        let err = repo
            .create(
                &sample("dup", Some(company)),
                &[event("b", "script.created", json!({}))],
            )
            .await;
        assert!(matches!(err, Err(DomainError::ValidationError(_))));
    }

    #[tokio::test]
    async fn same_code_allowed_across_companies_and_global() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let company_a = Uuid::new_v4();
        let company_b = Uuid::new_v4();
        repo.create(
            &sample("shared", Some(company_a)),
            &[event("a", "script.created", json!({}))],
        )
        .await
        .unwrap();
        repo.create(
            &sample("shared", Some(company_b)),
            &[event("b", "script.created", json!({}))],
        )
        .await
        .unwrap();
        repo.create(
            &sample("shared", None),
            &[event("g", "script.created", json!({}))],
        )
        .await
        .unwrap();

        assert!(
            repo.get_by_code("shared", Some(&company_a))
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            repo.get_by_code("shared", None).await.unwrap().is_some()
        );
    }

    #[tokio::test]
    async fn list_filters_by_company_includes_global() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let company = Uuid::new_v4();
        repo.create(
            &sample("company.only", Some(company)),
            &[event("a", "script.created", json!({}))],
        )
        .await
        .unwrap();
        repo.create(
            &sample("sys.template", None),
            &[event("g", "script.created", json!({}))],
        )
        .await
        .unwrap();

        let for_company = repo.list(Some(&company)).await.unwrap();
        assert_eq!(for_company.len(), 2);
        assert!(for_company.iter().any(|s| s.code == "company.only"));
        assert!(for_company.iter().any(|s| s.code == "sys.template"));

        let global = repo.list(None).await.unwrap();
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].code, "sys.template");
    }

    #[tokio::test]
    async fn update_replaces_source() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let company = Uuid::new_v4();
        let mut s = sample("calc", Some(company));
        repo.create(&s, &[event("a", "script.created", json!({}))])
            .await
            .unwrap();

        s.source = "ctx.object.amount * 3".to_string();
        s.name = "Новое имя".to_string();
        let updated = repo
            .update(&s, &[event(&s.id.to_string(), "script.updated", json!({}))])
            .await
            .unwrap();
        assert_eq!(updated.source, "ctx.object.amount * 3");

        let got = repo.get(&s.id).await.unwrap().unwrap();
        assert_eq!(got.name, "Новое имя");
    }

    #[tokio::test]
    async fn delete_removes_record() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let s = sample("gone", None);
        repo.create(&s, &[event("a", "script.created", json!({}))])
            .await
            .unwrap();
        repo.delete(&s.id, &[event(&s.id.to_string(), "script.deleted", json!({}))])
            .await
            .unwrap();
        assert!(repo.get(&s.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_by_module_removes_module_scripts() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let company = Uuid::new_v4();
        let mut s1 = sample("mod.one", Some(company));
        s1.module_code = Some("mod".to_string());
        let mut s2 = sample("mod.two", Some(company));
        s2.module_code = Some("mod".to_string());
        let mut other = sample("other", Some(company));
        other.module_code = Some("other_mod".to_string());
        repo.create(&s1, &[event("a", "script.created", json!({}))])
            .await
            .unwrap();
        repo.create(&s2, &[event("b", "script.created", json!({}))])
            .await
            .unwrap();
        repo.create(&other, &[event("c", "script.created", json!({}))])
            .await
            .unwrap();

        repo.delete_by_module("mod", &[event("d", "script.deleted", json!({}))])
            .await
            .unwrap();
        assert!(repo.get(&s1.id).await.unwrap().is_none());
        assert!(repo.get(&s2.id).await.unwrap().is_none());
        assert!(repo.get(&other.id).await.unwrap().is_some());
    }
}