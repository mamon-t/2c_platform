//! Rhai-движок скриптов с песочницей: лимиты операций/строки/вложенности,
//! выполнение на отдельном OS-потоке с жёстким таймаутом, кэш скомпилированных AST.
//!
//! Песочница: `Engine::new_raw()` — стандартные модули (в т.ч. `file`/`net`) не
//! регистрируются вовсе, поэтому доступ файловой системы и сети из скрипта невозможен.
//!
//! Контекст `ctx.*` — обычный Rhai-`Map` (```user/company/entity_type/action/object/changes/settings```),
//! собирается на каждый вызов: движок в Rhai 1.26 не реализует `Clone`, поэтому
//! сконфигурированный движок строится заново из `CoreApiShared`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

use core_application::ports::ScriptEngine;
use core_application::script_context::ScriptContext;
use core_domain::error::DomainError;
use core_domain::{ActorSnapshot};
use rhai::{Array, Dynamic, Engine, EvalAltResult, ImmutableString, Map, Position, Scope, AST};

use crate::rhai_core_api::{CoreApiShared, register_core_api};

/// Таймаут на выполнение одного скрипта.
const SCRIPT_TIMEOUT_SECS: u64 = 10;
/// Лимит операций (fuel) — защита от бесконечных циклов.
const MAX_OPERATIONS: u64 = 10_000_000;
/// Максимальный размер строки/массива/карты — защита от OOM.
const MAX_COLLECTION_SIZE: usize = 1 << 20;
/// Максимальная глубина вложенных вызовов.
const MAX_CALL_LEVELS: usize = 64;
/// Ёмкость кэша скомпилированных AST.
const MAX_CACHE: usize = 128;

/// Сведения о сессии, в которой исполняется скрипт; живут в thread-local,
/// потому что функции Core API регистрируются на общем движке без доступа к Scope.
#[derive(Clone)]
pub(crate) struct ScriptSession {
    pub actor: ActorSnapshot,
    pub company_id: Option<uuid::Uuid>,
    pub settings: serde_json::Value,
    pub test_run: bool,
}

thread_local! {
    static SESSION: RefCell<Option<ScriptSession>> = const { RefCell::new(None) };
}

/// Доступ к сессии из функций Core API (зарегистрированного окружения движка).
pub(crate) fn with_session<R>(f: impl FnOnce(&ScriptSession) -> R) -> Option<R> {
    SESSION.with(|s| s.borrow().as_ref().map(f))
}

/// Собирает контекст `ctx.*` (Rhai-`Map`) из `ScriptContext`.
fn build_ctx_map(context: &ScriptContext) -> Map {
    let mut map = Map::new();

    if let Some(actor) = &context.user {
        let mut user = Map::new();
        user.insert(
            "user_id".into(),
            actor
                .user_id
                .map(|id| Dynamic::from(id.to_string()))
                .unwrap_or(Dynamic::UNIT),
        );
        user.insert("login".into(), Dynamic::from(actor.login.clone()));
        user.insert("full_name".into(), Dynamic::from(actor.full_name.clone()));
        user.insert(
            "position".into(),
            actor
                .position
                .clone()
                .map(Dynamic::from)
                .unwrap_or(Dynamic::UNIT),
        );
        user.insert(
            "company_id".into(),
            actor
                .company_id
                .map(|id| Dynamic::from(id.to_string()))
                .unwrap_or(Dynamic::UNIT),
        );
        map.insert("user".into(), Dynamic::from(user));
    } else {
        map.insert("user".into(), Dynamic::UNIT);
    }

    map.insert(
        "company".into(),
        context
            .company_id
            .map(|id| Dynamic::from(id.to_string()))
            .unwrap_or(Dynamic::UNIT),
    );
    map.insert(
        "entity_type".into(),
        context
            .entity_type
            .clone()
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT),
    );
    map.insert(
        "action".into(),
        context
            .action
            .clone()
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT),
    );
    map.insert(
        "object".into(),
        context
            .object
            .as_ref()
            .map(json_to_dynamic)
            .unwrap_or(Dynamic::UNIT),
    );
    map.insert(
        "args".into(),
        context
            .args
            .as_ref()
            .map(json_to_dynamic)
            .unwrap_or(Dynamic::UNIT),
    );
    map.insert(
        "changes".into(),
        context
            .changes
            .as_ref()
            .map(json_to_dynamic)
            .unwrap_or(Dynamic::UNIT),
    );
    map.insert("settings".into(), json_to_dynamic(&context.settings));

    map
}

/// Реализация [`ScriptEngine`] на базе Rhai.
pub struct RhaiScriptEngine {
    shared: CoreApiShared,
    cache: RwLock<VecDeque<(u64, Arc<AST>)>>,
}

impl RhaiScriptEngine {
    /// Создаёт движок с песочницей и Core API (функции регистрируются при каждой
    /// компиляции/выполнении через `build_configured_engine`). Асинхронные операции
    /// Core API выполняются посредством `CoreApiShared.runtime` — рантайма, владеющего
    /// соединениями, через паттерн `spawn + std-канал` (см. `await_core_api`).
    pub fn new(shared: CoreApiShared) -> Result<Self, DomainError> {
        Ok(Self {
            shared,
            cache: RwLock::new(VecDeque::new()),
        })
    }

    /// Строит сконфигурированный движок: `new_raw()` (без стандартных модулей),
    /// лимиты, функции Core API.
    fn engine(&self) -> Engine {
        build_configured_engine(self.shared.clone())
    }

    /// Компилирует исходник (с кэшем) и возвращает упакованный AST.
    fn get_ast(&self, source: &str) -> Result<Arc<AST>, DomainError> {
        let key = cache_key(source);
        {
            let cache = self.cache.read().map_err(|_| {
                DomainError::Storage("кэш AST заблокирован (read)".to_string())
            })?;
            if let Some(ast) = cache.iter().find(|(k, _)| *k == key) {
                return Ok(ast.1.clone());
            }
        }
        let ast = self
            .engine()
            .compile(source)
            .map_err(|e| DomainError::ValidationError(format!("синтаксическая ошибка скрипта: {e}")))?;
        {
            let mut cache = self.cache.write().map_err(|_| {
                DomainError::Storage("кэш AST заблокирован (write)".to_string())
            })?;
            if cache.len() >= MAX_CACHE {
                cache.pop_front();
            }
            cache.push_back((key, Arc::new(ast)));
        }
        self.cache
            .read()
            .map_err(|_| DomainError::Storage("кэш AST заблокирован (read)".to_string()))?
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, ast)| ast.clone())
            .ok_or_else(|| DomainError::Storage("сломан кэш AST".to_string()))
    }
}

impl ScriptEngine for RhaiScriptEngine {
    fn execute<'a>(
        &'a self,
        source: String,
        context: &'a ScriptContext,
    ) -> core_application::ports::BoxFuture<'a, Result<serde_json::Value, DomainError>> {
        Box::pin(async move { self.run(source, context).await })
    }

    fn validate(&self, source: &str) -> Result<(), DomainError> {
        self.get_ast(source).map(|_| ())
    }
}

impl RhaiScriptEngine {
    /// Фактическое выполнение на OS-потоке с таймаутом.
    async fn run(
        &self,
        source: String,
        context: &ScriptContext,
    ) -> Result<serde_json::Value, DomainError> {
        let ast = self.get_ast(&source)?;
        let ctx = build_ctx_map(context);
        let session = ScriptSession {
            actor: context
                .user
                .clone()
                .unwrap_or_else(ActorSnapshot::system),
            company_id: context.company_id,
            settings: context.settings.clone(),
            test_run: context.test_run,
        };
        let engine = self.engine();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<Dynamic, Box<EvalAltResult>>>(1);
        thread::Builder::new()
            .name("rhai-script".to_string())
            .spawn(move || {
                SESSION.with(|s| *s.borrow_mut() = Some(session));
                let mut scope = Scope::new();
                scope.push_constant("ctx", Dynamic::from(ctx));
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
                }));
                SESSION.with(|s| *s.borrow_mut() = None);
                let out = match out {
                    Ok(res) => res,
                    Err(payload) => {
                        let msg = if let Some(s) = payload.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "паника движка скрипта".to_string()
                        };
                        Err(Box::new(EvalAltResult::ErrorRuntime(
                            format!("внутренняя ошибка движка скрипта при выполнении: {msg}").into(),
                            Position::NONE,
                        )))
                    }
                };
                let _ = tx.blocking_send(out);
            })
            .map_err(|e| DomainError::Storage(format!("не удалось создать поток скрипта: {e}")))?;

        match tokio::time::timeout(Duration::from_secs(SCRIPT_TIMEOUT_SECS), rx.recv()).await {
            Ok(Some(Ok(value))) => dynamic_to_json(value),
            Ok(Some(Err(err))) => Err(classify_runtime_error(*err)),
            Ok(None) => Err(DomainError::Storage(
                "поток скрипта завершился без результата".to_string(),
            )),
            Err(_) => Err(DomainError::Storage(format!(
                "таймаут выполнения скрипта ({} с)",
                SCRIPT_TIMEOUT_SECS
            ))),
        }
        // JoinHandle намеренно отцепляется: fuel-лимит гарантирует завершение потока.
    }
}

/// Строит движок с лимитами и Core API.
fn build_configured_engine(shared: CoreApiShared) -> Engine {
    let mut engine = Engine::new_raw();
    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_string_size(MAX_COLLECTION_SIZE);
    engine.set_max_array_size(MAX_COLLECTION_SIZE);
    engine.set_max_map_size(MAX_COLLECTION_SIZE);
    engine.set_max_call_levels(MAX_CALL_LEVELS);
    register_core_api(&mut engine, shared.clone());
    engine
}

/// Классификация ошибки выполнения: лимиты/переполнение — Storage, остальное — ValidationError.
fn classify_runtime_error(err: EvalAltResult) -> DomainError {
    let msg = err.to_string();
    let lower = msg.to_lowercase();
    if lower.contains("too many operations")
        || lower.contains("operations limit")
        || lower.contains("stack overflow")
        || lower.contains("too deep")
        || lower.contains("call stack")
    {
        DomainError::Storage(format!("скрипт превысил лимит ресурсов: {msg}"))
    } else {
        DomainError::ValidationError(format!("ошибка выполнения скрипта: {msg}"))
    }
}

fn cache_key(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

/// Приводит результат Rhai к JSON. Объекты с float-ключами не поддерживаются.
pub(crate) fn dynamic_to_json(value: Dynamic) -> Result<serde_json::Value, DomainError> {
    if value.is_unit() || value.type_name() == "()" {
        return Ok(serde_json::Value::Null);
    }
    if value.is_bool() {
        if let Ok(b) = value.as_bool() {
            return Ok(serde_json::Value::Bool(b));
        }
    }
    if value.is_int() {
        if let Ok(i) = value.as_int() {
            return Ok(serde_json::Value::from(i));
        }
    }
    if value.is_float() {
        if let Ok(f) = value.as_float() {
            return Ok(serde_json::Value::from(f));
        }
    }
    if value.is_char() {
        if let Ok(c) = value.as_char() {
            return Ok(serde_json::Value::from(c.to_string()));
        }
    }
    if value.is_string() {
        if let Some(s) = value.clone().try_cast::<ImmutableString>() {
            return Ok(serde_json::Value::from(s.to_string()));
        }
    }
    if value.is_map() {
        if let Some(map) = value.clone().try_cast::<Map>() {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.to_string(), dynamic_to_json(v)?);
            }
            return Ok(serde_json::Value::Object(obj));
        }
    }
    if value.is_array() {
        if let Some(arr) = value.clone().try_cast::<Array>() {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                out.push(dynamic_to_json(v)?);
            }
            return Ok(serde_json::Value::Array(out));
        }
    }
    Err(DomainError::Storage(format!(
        "не удалось привести результат скрипта к JSON: тип {}",
        value.type_name()
    )))
}

/// Приводит JSON-значение к Rhai `Dynamic` (для `ctx.*` и ответов Core API).
pub(crate) fn json_to_dynamic(value: &serde_json::Value) -> Dynamic {
    match value {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(u) = n.as_u64() {
                Dynamic::from(u.min(i64::MAX as u64) as i64)
            } else {
                Dynamic::from(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s.to_owned()),
        serde_json::Value::Array(items) => {
            let arr: Array = items.iter().map(json_to_dynamic).collect();
            Dynamic::from(arr)
        }
        serde_json::Value::Object(obj) => {
            let mut map = Map::new();
            for (k, v) in obj {
                map.insert(k.as_str().into(), json_to_dynamic(v));
            }
            Dynamic::from(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhai_core_api::CoreApiShared;
    use crate::surreal_audit_repository::SurrealAuditRepository;
    use crate::surreal_event_store::SurrealEventStore;
    use core_domain::audit::AuditFilter;
    use serde_json::json;
    use surrealdb::engine::any::connect;
    use tokio::runtime::Runtime;

    fn test_shared() -> (CoreApiShared, Runtime) {
        let rt = Runtime::new().unwrap();
        let db = rt.block_on(async {
            let db = connect("mem://").await.unwrap();
            db.use_ns("test")
                .use_db(uuid::Uuid::new_v4().to_string())
                .await
                .unwrap();
            db
        });
        let store = SurrealEventStore::new(db.clone());
        let audit = SurrealAuditRepository::new(db.clone());
        rt.block_on(async {
            store.ensure_schema().await.unwrap();
            audit.ensure_schema().await.unwrap();
        });
        let shared = CoreApiShared {
            store: Arc::new(store),
            db: db.clone(),
            audit: Arc::new(audit),
            runtime: rt.handle().clone(),
        };
        (shared, rt)
    }

    fn ctx_object(object: serde_json::Value) -> ScriptContext {
        ScriptContext {
            user: None,
            company_id: None,
            entity_type: Some("object".to_string()),
            action: Some("test".to_string()),
            object: Some(object),
            changes: None,
            args: None,
            settings: json!({ "locale": "ru-RU" }),
            test_run: false,
        }
    }

    #[test]
    fn engine_executes_valid_script_with_context() {
        let (shared, rt) = test_shared();
        let engine = RhaiScriptEngine::new(shared).unwrap();
        let ctx = ctx_object(json!({ "amount": 100 }));
        let result = rt.block_on(engine.execute("ctx.object.amount * 2".to_string(), &ctx));
        assert_eq!(result.unwrap(), json!(200));
    }

    #[test]
    fn engine_rejects_bad_syntax() {
        let (shared, _rt) = test_shared();
        let engine = RhaiScriptEngine::new(shared).unwrap();
        let err = engine
            .validate("let x = ;")
            .expect_err("ожидали ошибку синтаксиса");
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[test]
    fn engine_enforces_operations_limit() {
        let (shared, rt) = test_shared();
        let engine = RhaiScriptEngine::new(shared).unwrap();
        let ctx = ScriptContext {
            settings: json!({}),
            ..ctx_object(json!({}))
        };
        let result = rt.block_on(engine.execute("loop {}".to_string(), &ctx));
        let err = result.expect_err("ожидали лимит операций");
        assert!(matches!(err, DomainError::Storage(_)), "{err}");
    }

    #[test]
    fn engine_enforces_call_depth_limit() {
        let (shared, rt) = test_shared();
        let engine = RhaiScriptEngine::new(shared).unwrap();
        let ctx = ctx_object(json!({}));
        let result = rt.block_on(engine.execute("fn f() { f() } f()".to_string(), &ctx));
        assert!(result.is_err(), "ожидали ошибку переполнения стека");
    }

    #[test]
    fn engine_blocks_fs_access() {
        let (shared, rt) = test_shared();
        let engine = RhaiScriptEngine::new(shared).unwrap();
        let ctx = ctx_object(json!({}));
        let result = rt.block_on(engine.execute(
            "let x = read_file(\"/etc/passwd\"); 1".to_string(),
            &ctx,
        ));
        let err = result.expect_err("ожидали отказ доступа к ФС");
        assert!(matches!(err, DomainError::ValidationError(_)), "{err}");
    }

    #[test]
    fn engine_format_money() {
        let (shared, rt) = test_shared();
        let engine = RhaiScriptEngine::new(shared).unwrap();
        let ctx = ctx_object(json!({}));
        let result = rt
            .block_on(engine.execute("format_money(1234.5, \"RUB\")".to_string(), &ctx))
            .unwrap();
        assert_eq!(result, json!("1234.50 RUB"));
    }

    #[test]
    fn engine_get_setting() {
        let (shared, rt) = test_shared();
        let engine = RhaiScriptEngine::new(shared).unwrap();
        let ctx = ctx_object(json!({}));
        let result = rt
            .block_on(engine.execute("get_setting(\"locale\")".to_string(), &ctx))
            .unwrap();
        assert_eq!(result, json!("ru-RU"));
    }

    #[test]
    fn engine_log_info_writes_audit() {
        let (shared, rt) = test_shared();
        let engine = RhaiScriptEngine::new(shared.clone()).unwrap();
        let ctx = ctx_object(json!({}));
        rt.block_on(engine.execute("log_info(\"hello from script\")".to_string(), &ctx))
            .unwrap();
        let filter = AuditFilter {
            action: Some("script.log".to_string()),
            ..AuditFilter::default()
        };
        let rows = rt.block_on(shared.audit.query(filter)).unwrap();
        assert!(!rows.is_empty());
        let details = rows[0].details.as_ref().unwrap();
        assert_eq!(details["level"], json!("info"));
        assert_eq!(details["message"], json!("hello from script"));
    }
}