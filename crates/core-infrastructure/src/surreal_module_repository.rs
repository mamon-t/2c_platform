//! Хранилище установленных WASM-модулей на базе SurrealDB: глобальный
//! каталог `modules` и проекция включения `company_modules` («Доска»).
//!
//! Установка — глобальная операция (запись в `modules` с UNIQUE по коду).
//! Повторная установка удалённого модуля (reinstall) перезаписывает запись
//! новым манифестом и байтами; установка активного модуля отклоняется.
//! Включение для компании управляется отдельной таблицей `company_modules`
//! с UNIQUE по `(company_id, code)`. Каждый изменяющий метод транзакционно
//! продвигает Трубу (события `module.*`) и Доску вместе.

use core_application::ports::{BoxFuture, ModuleRepository};
use core_domain::error::DomainError;
use core_domain::event::Event;
use core_domain::module::{CompanyModule, ModuleRecord, ModuleState};
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

use crate::events::{assign_versions, with_transaction, write_events};

/// Материализованный каталог `modules` + `company_modules`.
pub struct SurrealModuleRepository {
    db: Surreal<Any>,
}

impl SurrealModuleRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт таблицы и уникальные индексы идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS modules SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_modules_code ON modules FIELDS code UNIQUE",
            "DEFINE TABLE IF NOT EXISTS company_modules SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_company_modules_company_code ON company_modules FIELDS company_id, module_code UNIQUE",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("modules ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("modules ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

const MODULE_FIELDS: &str =
    "code, name, description, version, api_version, capabilities, metadata_version, state, wasm_sha256, manifest, installed_at";

fn company_module_record(company_id: &str, module_code: &str) -> String {
    format!("{company_id}:{module_code}")
}

fn decode_module(row: Value) -> Result<ModuleRecord, DomainError> {
    serde_json::from_value(row).map_err(|e| DomainError::Storage(format!("module decode: {e}")))
}

impl ModuleRepository for SurrealModuleRepository {
    fn install(&self, record: &ModuleRecord, events: &[Event]) -> BoxFuture<'_, Result<(), DomainError>> {
        let db = self.db.clone();
        let record = record.clone();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let record = record.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        let mut response = txn
                            .query("SELECT state FROM modules WHERE code = $code LIMIT 1")
                            .bind(("code", record.code.clone()))
                            .await
                            .map_err(|e| DomainError::Storage(format!("module check code: {e}")))?;
                        let existing: Option<Value> = response
                            .take(0)
                            .map_err(|e| DomainError::Storage(format!("module check take: {e}")))?;
                        if let Some(row) = existing {
                            let state: ModuleState = serde_json::from_value(
                                row.get("state").cloned().unwrap_or(Value::Null),
                            )
                            .map_err(|e| DomainError::Storage(format!("module state decode: {e}")))?;
                            if state == ModuleState::Installed {
                                return Err(DomainError::ValidationError(format!(
                                    "Модуль с кодом {} уже установлен",
                                    record.code
                                )));
                            }
                            // Переустановка удалённого модуля (reinstall): снимаем
                            // остатки включений для всех компаний, запись в каталоге
                            // перезаписывается новым манифестом и байтами.
                            txn.query("DELETE FROM company_modules WHERE module_code = $code")
                                .bind(("code", record.code.clone()))
                                .await
                                .map_err(|e| {
                                    DomainError::Storage(format!(
                                        "company modules reinstall cleanup: {e}"
                                    ))
                                })?;
                        }

                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        let value = serde_json::to_value(&record)
                            .map_err(|e| DomainError::Storage(format!("module encode: {e}")))?;
                        let _: Option<surrealdb::types::Value> = txn
                            .upsert(("modules", record.code.clone()))
                            .content(value)
                            .await
                            .map_err(|e| DomainError::Storage(format!("module write: {e}")))?;
                        Ok(())
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn uninstall(&self, code: &str, events: &[Event]) -> BoxFuture<'_, Result<ModuleRecord, DomainError>> {
        let db = self.db.clone();
        let code = code.to_string();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let code = code.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<ModuleRecord, DomainError> = async {
                        let mut response = txn
                            .query(format!(
                                "SELECT {MODULE_FIELDS}, id FROM modules WHERE code = $code LIMIT 1"
                            ))
                            .bind(("code", code.clone()))
                            .await
                            .map_err(|e| DomainError::Storage(format!("module get uninstall: {e}")))?;
                        let row: Option<Value> = response
                            .take(0)
                            .map_err(|e| DomainError::Storage(format!("module get take: {e}")))?;
                        let mut record = row
                            .map(decode_module)
                            .transpose()?
                            .ok_or_else(|| DomainError::NotFound(format!("Модуль {code} не установлен")))?;
                        if record.state == ModuleState::Uninstalled {
                            return Err(DomainError::ValidationError(format!(
                                "Модуль {code} уже удалён"
                            )));
                        }

                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        record.state = ModuleState::Uninstalled;
                        let value = serde_json::to_value(&record)
                            .map_err(|e| DomainError::Storage(format!("module encode: {e}")))?;
                        let _: Option<surrealdb::types::Value> = txn
                            .upsert(("modules", code.clone()))
                            .content(value)
                            .await
                            .map_err(|e| DomainError::Storage(format!("module uninstall write: {e}")))?;

                        // Снятие включений модуля для всех компаний: запись в каталоге
                        // остаётся как «след», а проекции компании очищаются.
                        txn.query("DELETE FROM company_modules WHERE module_code = $code")
                        .bind(("code", code.clone()))
                        .await
                        .map_err(|e| DomainError::Storage(format!("company modules cleanup: {e}")))?;

                        Ok(record)
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn get(&self, code: &str) -> BoxFuture<'_, Result<ModuleRecord, DomainError>> {
        let db = self.db.clone();
        let code = code.to_string();
        Box::pin(async move {
            let mut response = db
                .query(format!("SELECT {MODULE_FIELDS} FROM modules WHERE code = $code LIMIT 1"))
                .bind(("code", code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("module get: {e}")))?;
            let row: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("module get take: {e}")))?;
            row.map(decode_module)
                .transpose()?
                .ok_or_else(|| DomainError::NotFound(format!("Модуль {code} не установлен")))
        })
    }

    fn list(&self) -> BoxFuture<'_, Result<Vec<ModuleRecord>, DomainError>> {
        let db = self.db.clone();
        Box::pin(async move {
            let mut response = db
                .query(format!("SELECT {MODULE_FIELDS} FROM modules ORDER BY code"))
                .await
                .map_err(|e| DomainError::Storage(format!("module list: {e}")))?;
            let rows: Vec<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("module list take: {e}")))?;
            rows.into_iter()
                .map(decode_module)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn enable_for_company(
        &self,
        company_id: &str,
        code: &str,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>> {
        let db = self.db.clone();
        let company_id = company_id.to_string();
        let code = code.to_string();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let company_id = company_id.clone();
                let code = code.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        let mut response = txn
                            .query("SELECT code FROM modules WHERE code = $code LIMIT 1")
                            .bind(("code", code.clone()))
                            .await
                            .map_err(|e| DomainError::Storage(format!("module enable check: {e}")))?;
                        let existing: Option<Value> = response
                            .take(0)
                            .map_err(|e| DomainError::Storage(format!("module enable check take: {e}")))?;
                        if existing.is_none() {
                            return Err(DomainError::NotFound(format!("Модуль {code} не установлен")));
                        }

                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        let rec = CompanyModule {
                            company_id: company_id.clone(),
                            module_code: code.clone(),
                            enabled: true,
                            updated_at: chrono::Utc::now(),
                        };
                        let value = serde_json::to_value(&rec)
                            .map_err(|e| DomainError::Storage(format!("company_module encode: {e}")))?;
                        let _: Option<surrealdb::types::Value> = txn
                            .upsert(("company_modules", company_module_record(&company_id, &code)))
                            .content(value)
                            .await
                            .map_err(|e| DomainError::Storage(format!("company_module write: {e}")))?;
                        Ok(())
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn disable_for_company(
        &self,
        company_id: &str,
        code: &str,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>> {
        let db = self.db.clone();
        let company_id = company_id.to_string();
        let code = code.to_string();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let company_id = company_id.clone();
                let code = code.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        let mut events = events.clone();
                        assign_versions(&txn, &mut events).await?;
                        write_events(&txn, &events).await?;

                        let rec = CompanyModule {
                            company_id: company_id.clone(),
                            module_code: code.clone(),
                            enabled: false,
                            updated_at: chrono::Utc::now(),
                        };
                        let value = serde_json::to_value(&rec)
                            .map_err(|e| DomainError::Storage(format!("company_module encode: {e}")))?;
                        let _: Option<surrealdb::types::Value> = txn
                            .upsert(("company_modules", company_module_record(&company_id, &code)))
                            .content(value)
                            .await
                            .map_err(|e| DomainError::Storage(format!("company_module write: {e}")))?;
                        Ok(())
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn list_enabled_for_company(
        &self,
        company_id: &str,
    ) -> BoxFuture<'_, Result<Vec<ModuleRecord>, DomainError>> {
        let db = self.db.clone();
        let company_id = company_id.to_string();
        Box::pin(async move {
            let mut response = db
                .query(format!(
                    "SELECT {MODULE_FIELDS} FROM modules \
                     WHERE code IN (SELECT VALUE module_code FROM company_modules \
                     WHERE company_id = $company_id AND enabled = true) ORDER BY code"
                ))
                .bind(("company_id", company_id.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("module list_enabled: {e}")))?;
            let rows: Vec<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("module list_enabled take: {e}")))?;
            rows.into_iter()
                .map(decode_module)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn is_enabled_for_company(
        &self,
        company_id: &str,
        code: &str,
    ) -> BoxFuture<'_, Result<bool, DomainError>> {
        let db = self.db.clone();
        let company_id = company_id.to_string();
        let code = code.to_string();
        Box::pin(async move {
            let mut response = db
                .query(
                    "SELECT VALUE enabled FROM company_modules \
                     WHERE company_id = $company_id AND module_code = $code LIMIT 1",
                )
                .bind(("company_id", company_id.clone()))
                .bind(("code", code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("module is_enabled: {e}")))?;
            let enabled: Option<bool> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("module is_enabled take: {e}")))?;
            Ok(enabled.unwrap_or(false))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SurrealEventStore;
    use core_application::ports::EventStore;
    use core_domain::event::{ActorSnapshot, StreamType};
    use core_domain::module::ModuleLifecyclePayload;
    use serde_json::json;
    use uuid::Uuid;

    async fn mem_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        db
    }

    fn event(stream_id: &str, company_id: &str, event_type: &str, payload: Value) -> Event {
        Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Module,
            stream_id: stream_id.to_string(),
            event_type: event_type.to_string(),
            version: 0,
            payload,
            metadata: ActorSnapshot::system(),
            company_id: company_id.to_string(),
            correlation_id: "corr".to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        }
    }

    fn sample(code: &str) -> ModuleRecord {
        ModuleRecord {
            code: code.to_string(),
            name: format!("Модуль {code}"),
            description: None,
            version: "1.0.0".to_string(),
            api_version: "2.0".to_string(),
            capabilities: vec!["objects.read".to_string()],
            metadata_version: 1,
            state: ModuleState::Installed,
            wasm_sha256: "0123456789abcdef".to_string(),
            manifest: serde_json::from_value(serde_json::json!({
                "code": code,
                "version": "1.0.0",
                "display_name": format!("Модуль {code}"),
                "api_version": "2.0",
                "capabilities": ["objects.read"]
            }))
            .unwrap(),
            installed_at: chrono::Utc::now(),
        }
    }

    async fn repo(db: Surreal<Any>) -> (SurrealEventStore, SurrealModuleRepository) {
        let store = SurrealEventStore::new(db.clone());
        store.ensure_schema().await.unwrap();
        let repo = SurrealModuleRepository::new(db);
        repo.ensure_schema().await.unwrap();
        (store, repo)
    }

    #[tokio::test]
    async fn install_get_list_round_trip() {
        let db = mem_db().await;
        let (store, repo) = repo(db).await;
        let company = Uuid::new_v4().to_string();
        let m = sample("hello");
        let e = event(
            &m.code,
            &company,
            "module.installed",
            serde_json::to_value(ModuleLifecyclePayload {
                reason: "install".to_string(),
            })
            .unwrap(),
        );
        repo.install(&m, std::slice::from_ref(&e)).await.unwrap();

        let got = repo.get("hello").await.unwrap();
        assert_eq!(got.code, "hello");
        assert_eq!(got.wasm_sha256, "0123456789abcdef");
        assert_eq!(repo.list().await.unwrap().len(), 1);

        let stream = store
            .read_stream(StreamType::Module, "hello")
            .await
            .unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].event_type, "module.installed");
        assert_eq!(stream[0].version, 1);
    }

    #[tokio::test]
    async fn duplicate_code_rejected() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let company = Uuid::new_v4().to_string();
        repo.install(&sample("hello"), &[event("hello", &company, "module.installed", json!({}))]).await.unwrap();
        let err = repo.install(&sample("hello"), &[event("hello", &company, "module.installed", json!({}))]).await;
        assert!(matches!(err, Err(DomainError::ValidationError(_))));
    }

    #[tokio::test]
    async fn enable_disable_for_company_and_list() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let company_a = Uuid::new_v4().to_string();
        let company_b = Uuid::new_v4().to_string();
        repo.install(&sample("hello"), &[event("hello", &company_a, "module.installed", json!({}))]).await.unwrap();

        repo.enable_for_company(&company_a, "hello", &[event("hello", &company_a, "module.enabled", json!({}))]).await.unwrap();
        assert!(repo.is_enabled_for_company(&company_a, "hello").await.unwrap());
        assert!(!repo.is_enabled_for_company(&company_b, "hello").await.unwrap());

        let enabled_a = repo.list_enabled_for_company(&company_a).await.unwrap();
        assert_eq!(enabled_a.len(), 1);
        assert_eq!(enabled_a[0].code, "hello");
        assert!(repo.list_enabled_for_company(&company_b).await.unwrap().is_empty());

        repo.disable_for_company(&company_a, "hello", &[event("hello", &company_a, "module.disabled", json!({}))]).await.unwrap();
        assert!(!repo.is_enabled_for_company(&company_a, "hello").await.unwrap());
        assert!(repo.list_enabled_for_company(&company_a).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn uninstall_marks_uninstalled_does_not_delete() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let company = Uuid::new_v4().to_string();
        repo.install(&sample("hello"), &[event("hello", &company, "module.installed", json!({}))]).await.unwrap();
        let rec = repo.uninstall("hello", &[event("hello", &company, "module.uninstalled", json!({}))]).await.unwrap();
        assert_eq!(rec.state, ModuleState::Uninstalled);
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn enable_missing_module_returns_not_found() {
        let db = mem_db().await;
        let (_store, repo) = repo(db).await;
        let err = repo.enable_for_company("c", "missing", &[]).await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
    }
}