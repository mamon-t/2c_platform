//! Быстрые Rust-функции, доступные скриптам Rhai (Core API из ТЗ §15).
//!
//! Все функции регистрируются на общем движке; доступ к сессии (исполнитель,
//! компания, настройки, флаг тестового запуска) — через thread-local `with_session`.
//! Функции не паникуют: ошибки возвращаются через `Result<T, EvalAltResult>`,
//! движок автоматически превращает их в ошибку выполнения скрипта.
//!
//! `db_query` и `emit_event` при `test_run` ничего не пишут (возвращают `null`).

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use core_application::ports::{AuditRepository, EventStore};
use core_domain::audit::{AuditEntry, AuditResult, AuditTarget};
use core_domain::error::DomainError;
use core_domain::{Event, StreamType};
use rhai::{Dynamic, Engine, EvalAltResult, ImmutableString, Position};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

use crate::rhai_script_engine::{with_session, ScriptSession};

/// Общие зависимости Core API, передаваемые движку при создании.
#[derive(Clone)]
pub struct CoreApiShared {
    /// Порт записи в Трубу для `emit_event`.
    pub store: Arc<dyn EventStore>,
    /// Прямой доступ к SurrealDB для `db_query` (только SELECT/INFO).
    pub db: Surreal<Any>,
    /// Репозиторий аудита для `log_*`.
    pub audit: Arc<dyn AuditRepository>,
    /// Рантайм, владеющий соединениями SurrealDB/Store, на котором выполняются
    /// асинхронные операции Core API (см. [`await_core_api`]).
    pub runtime: tokio::runtime::Handle,
}

/// Таймаут одной асинхронной операции Core API.
const CORE_API_TIMEOUT_SECS: u64 = 8;

/// Выполняет асинхронную операцию на рантайме, владеющем соединениями, а поток
/// скрипта блокирует на std-канале до результата. Прямой `Handle::block_on` из
/// потока скрипта в блокирующем WASM host-вызове повисал, поэтому используется
/// паттерн `spawn + канал` (аналогично `block_on_db` в extism_wasm_host).
fn await_core_api<T>(
    shared: &CoreApiShared,
    fut: impl Future<Output = T> + Send + 'static,
) -> Result<T, DomainError>
where
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel::<T>();
    std::mem::drop(shared.runtime.spawn(async move {
        let v = fut.await;
        let _ = tx.send(v);
    }));
    let r = rx.recv_timeout(Duration::from_secs(CORE_API_TIMEOUT_SECS));
    r.map_err(|_| DomainError::Storage("таймаут операции Core API".to_string()))
}

/// Имя действия аудита для логов из скриптов.
const AUDIT_ACTION: &str = "script.log";

fn session_company_key(session: &ScriptSession) -> String {
    session
        .company_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "global".to_string())
}

/// Регистрирует функции Core API в движке.
pub fn register_core_api(engine: &mut Engine, shared: CoreApiShared) {
    let store = shared.store.clone();
    let db = shared.db.clone();

    let log_fn = |level: &str,
                  shared: CoreApiShared,
                  msg: ImmutableString|
     -> Result<(), Box<EvalAltResult>> {
        let session = match with_session(|s| s.clone()) {
            Some(s) => s,
            None => {
                return Err(Box::new(EvalAltResult::ErrorRuntime(
                    Dynamic::from("нет сессии скрипта"),
                    Position::NONE,
                )))
            }
        };
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4(),
            action: AUDIT_ACTION.to_string(),
            actor: session.actor.clone(),
            target: Some(AuditTarget {
                entity_type: None,
                entity_id: None,
                entity_code: None,
                company_id: session.company_id,
            }),
            result: AuditResult::Success,
            details: Some(serde_json::json!({ "level": level, "message": msg.to_string() })),
            ip_address: None,
            user_agent: None,
            company_id: session.company_id,
            timestamp: chrono::Utc::now(),
        };
        let audit = shared.audit.clone();
        match await_core_api(&shared, async move { audit.log(entry).await }) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) | Err(e) => Err(Box::new(EvalAltResult::ErrorRuntime(
                Dynamic::from(e.to_string()),
                Position::NONE,
            ))),
        }
    };

    engine.register_fn("log_info", {
        let shared = shared.clone();
        move |msg: ImmutableString| log_fn("info", shared.clone(), msg)
    });
    engine.register_fn("log_warn", {
        let shared = shared.clone();
        move |msg: ImmutableString| log_fn("warn", shared.clone(), msg)
    });
    engine.register_fn("log_error", {
        let shared = shared.clone();
        move |msg: ImmutableString| log_fn("error", shared.clone(), msg)
    });

    engine
        .register_fn("db_query", {
            let shared = shared.clone();
            move |sql: ImmutableString| {
            let sql = sql.to_string();
            let session = match with_session(|s| s.clone()) {
                Some(s) => s,
                None => {
                    return Err(Box::new(EvalAltResult::ErrorRuntime(
                        Dynamic::from("нет сессии скрипта"),
                        Position::NONE,
                    )))
                }
            };
            let trimmed = sql.trim_start().to_lowercase();
            if !(trimmed.starts_with("select ") || trimmed.starts_with("info ")) {
                return Err(Box::new(EvalAltResult::ErrorRuntime(
                    Dynamic::from("разрешены только SELECT и INFO"),
                    Position::NONE,
                )));
            }
            if session.test_run {
                return Ok(Dynamic::UNIT);
            }
            let db = db.clone();
            let rows: Vec<serde_json::Value> = match await_core_api(&shared, async move {
                let mut response = db
                    .query(sql)
                    .await
                    .map_err(|e| DomainError::Storage(e.to_string()))?;
                response
                    .take(0)
                    .map_err(|e| DomainError::Storage(e.to_string()))
            }) {
                Ok(Ok(rows)) => rows,
                Ok(Err(e)) | Err(e) => {
                    return Err(Box::new(EvalAltResult::ErrorRuntime(
                        Dynamic::from(e.to_string()),
                        Position::NONE,
                    )))
                }
            };
            Ok(crate::rhai_script_engine::json_to_dynamic(
                &serde_json::Value::Array(rows),
            ))
            }
        })

        .register_fn(
            "emit_event",
            {
                let shared = shared.clone();
                move |stream_id: ImmutableString, event_type: ImmutableString, payload: Dynamic| {
                let session = match with_session(|s| s.clone()) {
                    Some(s) => s,
                    None => {
                        return Err(Box::new(EvalAltResult::ErrorRuntime(
                            Dynamic::from("нет сессии скрипта"),
                            Position::NONE,
                        )))
                    }
                };
                let payload = match crate::rhai_script_engine::dynamic_to_json(payload) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(Box::new(EvalAltResult::ErrorRuntime(
                            Dynamic::from(e.to_string()),
                            Position::NONE,
                        )))
                    }
                };
                if session.test_run {
                    return Ok(Dynamic::UNIT);
                }
                let event = Event {
                    id: uuid::Uuid::new_v4(),
                    stream_type: StreamType::Object,
                    stream_id: stream_id.to_string(),
                    event_type: event_type.to_string(),
                    version: 0,
                    payload,
                    metadata: session.actor.clone(),
                    company_id: session_company_key(&session),
                    correlation_id: "script.emit_event".to_string(),
                    causation_id: None,
                    occurred_at: chrono::Utc::now(),
                };
                let id = event.id.to_string();
                let store = store.clone();
                match await_core_api(&shared, async move { store.append(&[event]).await }) {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) | Err(e) => {
                        return Err(Box::new(EvalAltResult::ErrorRuntime(
                            Dynamic::from(e.to_string()),
                            Position::NONE,
                        )))
                    }
                }
                Ok(Dynamic::from(id))
                }
            },
        );

    engine.register_fn(
        "notify_user",
        |_user_id: ImmutableString, _subject: ImmutableString, _body: ImmutableString| -> Dynamic {
            // Заглушка до Фазы 16 (уведомления).
            crate::rhai_script_engine::json_to_dynamic(&serde_json::json!({ "queued": true }))
        },
    );
    engine.register_fn("get_setting", |key: ImmutableString| {
        with_session(|s| {
            s.settings
                .get(key.as_str())
                .cloned()
                .map(|v| crate::rhai_script_engine::json_to_dynamic(&v))
                .unwrap_or(Dynamic::UNIT)
        })
        .unwrap_or(Dynamic::UNIT)
    });
    engine.register_fn(
        "format_money",
        |amount: f64, currency: ImmutableString| -> ImmutableString {
            ImmutableString::from(format!("{amount:.2} {}", currency.as_str()))
        },
    );
    engine.register_fn("parse_date", |date: ImmutableString| -> Dynamic {
        let parsed = chrono::DateTime::parse_from_rfc3339(date.as_str())
            .map(|d| d.with_timezone(&chrono::Utc))
            .or_else(|_| {
                chrono::NaiveDate::parse_from_str(date.as_str(), "%Y-%m-%d")
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            });
        match parsed {
            Ok(dt) => Dynamic::from(dt.to_rfc3339()),
            Err(_) => Dynamic::UNIT,
        }
    });
}