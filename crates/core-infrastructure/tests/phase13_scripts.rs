//! Интеграционные приёмочные тесты скриптов Rhai (Фаза 13 по ТЗ v3.1 §15):
//! политика `platform.scripts` в seed, RBAC-границы команд `script.*` через
//! `CommandExecutionPipeline` и валидации запуска `execute_script` на полном
//! стеке хранилищ `mem://`.
//!
//! Сценарии приёмки:
//! 1. Seed содержит политику `platform.scripts` (namespaced действия, priority 90,
//!    scope Platform), не привязанную ни к одной системной роли напрямую.
//! 2. Администратор через pipeline исполняет script.create → script.execute → script.get.
//! 3. Сотрудник (staff.objects) получает отказ на script.execute и script.create.
//! 4. Гость получает отказ на script.get с аудитом `permission.denied`.
//! 5. `execute_script`: object обязателен для привязанного типа; отключённый скрипт;
//!    object без привязки запрещён; успешный запуск с args+object; NotFound.

use std::sync::Arc;

use core_application::command_registry::{CommandExecutionCtx, CommandMetadata, CommandRegistry};
use core_application::permission_manager::PermissionManager;
use core_application::ports::{
    AuditRepository, PermissionPolicyRepository, RoleRepository, ScriptRepository, UserRepository,
};
use core_application::script_runner::execute_script;
use core_application::seed::seed_system_roles_and_policies;
use core_domain::audit::AuditFilter;
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::permission::PermissionScopeType;
use core_domain::script::{Script, ScriptType};
use core_domain::user::{Person, User, UserStatus};
use core_infrastructure::rhai_core_api::CoreApiShared;
use core_infrastructure::rhai_script_engine::RhaiScriptEngine;
use core_infrastructure::surreal_audit_repository::SurrealAuditRepository;
use core_infrastructure::surreal_event_store::SurrealEventStore;
use core_infrastructure::surreal_permission_policy_repository::SurrealPermissionPolicyRepository;
use core_infrastructure::surreal_role_repository::SurrealRoleRepository;
use core_infrastructure::surreal_script_repository::SurrealScriptRepository;
use core_infrastructure::surreal_user_repository::SurrealUserRepository;
use serde_json::{json, Value};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

async fn mem_db() -> Surreal<Any> {
    let db = surrealdb::engine::any::connect("mem://").await.unwrap();
    db.use_ns("test")
        .use_db(Uuid::new_v4().to_string())
        .await
        .unwrap();
    db
}

fn user_event(user: &User, company_id: Uuid) -> Event {
    Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::User,
        stream_id: user.id.to_string(),
        event_type: "user.created".to_string(),
        version: 0,
        payload: serde_json::to_value(user).unwrap(),
        metadata: ActorSnapshot::system(),
        company_id: company_id.to_string(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    }
}

fn actor(user_id: Uuid, company_id: Uuid) -> ActorSnapshot {
    ActorSnapshot {
        user_id: Some(user_id),
        login: "tester".to_string(),
        full_name: "Тест Тестов".to_string(),
        position: None,
        company_id: Some(company_id),
        ip_address: None,
    }
}

/// Событие `script.*` для Трубы (аналог `commands::script_event`).
fn script_event(script: &Script, event_type: &str) -> Event {
    let company_id = script.company_id.map(|u| u.to_string()).unwrap_or_default();
    Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::Script,
        stream_id: script.id.to_string(),
        event_type: event_type.to_string(),
        version: 0,
        payload: serde_json::to_value(script).unwrap_or_default(),
        metadata: ActorSnapshot::system(),
        company_id,
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    }
}

fn encode<T: serde::Serialize>(value: &T) -> Result<Value, DomainError> {
    serde_json::to_value(value)
        .map_err(|e| DomainError::ValidationError(format!("сериализация: {e}")))
}

fn require(value: &Value, key: &str) -> Result<String, DomainError> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| DomainError::ValidationError(format!("отсутствует обязательный параметр '{key}'")))
}

fn err_contains(err: &DomainError, needle: &str) -> bool {
    err.to_string().contains(needle)
}

fn ctx(user_id: Uuid, company_id: Uuid) -> CommandExecutionCtx {
    CommandExecutionCtx {
        actor: Some(actor(user_id, company_id)),
        module_code: None,
        entity_type: None,
    }
}

/// Строит движок Rhai на `mem://` с Core API (логирование/emit в тест-БД).
fn build_engine(db: &Surreal<Any>) -> RhaiScriptEngine {
    let engine_shared = CoreApiShared {
        store: Arc::new(SurrealEventStore::new(db.clone())),
        db: db.clone(),
        audit: Arc::new(SurrealAuditRepository::new(db.clone())),
        runtime: tokio::runtime::Handle::current(),
    };
    RhaiScriptEngine::new(engine_shared).unwrap()
}

/// Среда RBAC: seed ролей/политик, пользователь с ролью, pipeline и команды
/// `script.create/get/execute` (тонкие обёртки над репозиторием/движком).
struct TestEnv {
    registry: CommandRegistry,
    _engine: Arc<RhaiScriptEngine>,
    audit: Arc<SurrealAuditRepository>,
}

async fn pipeline_env(
    db: &Surreal<Any>,
    company_id: Uuid,
    user_id: Uuid,
    role_codes: &[&str],
) -> TestEnv {
    let store = SurrealEventStore::new(db.clone());
    store.ensure_schema().await.unwrap();
    let role_repo = SurrealRoleRepository::new(db.clone());
    role_repo.ensure_schema().await.unwrap();
    let policy_repo = SurrealPermissionPolicyRepository::new(db.clone());
    policy_repo.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();
    seed_system_roles_and_policies(&company_id, &role_repo, &policy_repo, audit.as_ref())
        .await
        .unwrap();

    let scripts = Arc::new(SurrealScriptRepository::new(db.clone()));
    scripts.ensure_schema().await.unwrap();

    let roles = role_repo.list().await.unwrap();
    let role_ids: Vec<String> = roles
        .iter()
        .filter(|r| role_codes.contains(&r.code.as_str()))
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(role_ids.len(), role_codes.len(), "все роли найдены");

    let person = Person {
        id: user_id,
        user_id,
        last_name: "Тестов".to_string(),
        first_name: "Тест".to_string(),
        middle_name: None,
        display_name: "Тест Тестов".to_string(),
    };
    let user = User {
        id: user_id,
        login: format!("user{user_id}"),
        password_hash: "x".to_string(),
        status: UserStatus::Active,
        role_ids,
        failed_login_count: 0,
        locked_until: None,
        must_change_password: false,
        locale: "ru-RU".to_string(),
        timezone: "Europe/Moscow".to_string(),
        person_id: Some(user_id),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let users = SurrealUserRepository::new(db.clone());
    users.ensure_schema().await.unwrap();
    users
        .create(&user, &person, &[user_event(&user, company_id)])
        .await
        .unwrap();

    let permissions = Arc::new(PermissionManager::new(
        Arc::new(role_repo),
        Arc::new(policy_repo),
    ));
    let registry = CommandRegistry::new();
    registry.attach_pipeline(audit.clone(), permissions).await;

    let engine = Arc::new(build_engine(db));

    let scripts_c = scripts.clone();
    registry
        .register_with_metadata(
            "script.create",
            CommandMetadata::requires("script.manage"),
            move |params: Value, _ctx: CommandExecutionCtx| {
                let scripts = scripts_c.clone();
                async move {
                    let code = require(&params, "code")?;
                    let name = require(&params, "name")?;
                    let source = require(&params, "source")?;
                    let company_id = params
                        .get("company_id")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            Uuid::parse_str(s).map_err(|e| {
                                DomainError::ValidationError(format!("некорректный 'company_id': {e}"))
                            })
                        })
                        .transpose()?;
                    let entity_type = params
                        .get("entity_type")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    let script_type = match params.get("script_type").and_then(|v| v.as_str()) {
                        Some(raw) => ScriptType::try_from(raw).map_err(DomainError::ValidationError)?,
                        None => ScriptType::Formula,
                    };
                    let now = chrono::Utc::now();
                    let record = Script {
                        id: Uuid::new_v4(),
                        code,
                        name,
                        script_type,
                        source,
                        company_id,
                        module_code: None,
                        entity_type,
                        is_active: true,
                        created_at: now,
                        updated_at: now,
                    };
                    let event = script_event(&record, "script.created");
                    let created = scripts.create(&record, &[event]).await?;
                    encode(&created)
                }
            },
        )
        .await;

    let scripts_g = scripts.clone();
    registry
        .register_with_metadata(
            "script.get",
            CommandMetadata::requires("script.read"),
            move |params: Value, _ctx: CommandExecutionCtx| {
                let scripts = scripts_g.clone();
                async move {
                    let company_id = params
                        .get("company_id")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            Uuid::parse_str(s).map_err(|e| {
                                DomainError::ValidationError(format!("некорректный 'company_id': {e}"))
                            })
                        })
                        .transpose()?;
                    let code = require(&params, "code")?;
                    let record = scripts.get_by_code(&code, company_id.as_ref()).await?;
                    record
                        .ok_or_else(|| DomainError::NotFound(format!("скрипт с кодом '{code}' не найден")))
                        .and_then(|s| encode(&s))
                }
            },
        )
        .await;

    let scripts_e = scripts.clone();
    let engine_e = engine.clone();
    registry
        .register_with_metadata(
            "script.execute",
            CommandMetadata::requires("script.execute"),
            move |params: Value, ctx: CommandExecutionCtx| {
                let scripts = scripts_e.clone();
                let engine = engine_e.clone();
                async move {
                    let actor = ctx.actor.clone().unwrap_or_else(ActorSnapshot::system);
                    execute_script(&*scripts, engine.as_ref(), &params, &actor).await
                }
            },
        )
        .await;

    TestEnv { registry, _engine: engine, audit }
}

/// Seed содержит политику `platform.scripts`: namespaced действия, scope Platform,
/// приоритет 90 и отсутствие привязки к системным ролям.
#[tokio::test]
async fn seed_contains_platform_scripts_policy() {
    let db = mem_db().await;
    let store = SurrealEventStore::new(db.clone());
    store.ensure_schema().await.unwrap();
    let role_repo = SurrealRoleRepository::new(db.clone());
    role_repo.ensure_schema().await.unwrap();
    let policy_repo = SurrealPermissionPolicyRepository::new(db.clone());
    policy_repo.ensure_schema().await.unwrap();
    let audit = SurrealAuditRepository::new(db.clone());
    audit.ensure_schema().await.unwrap();

    let company_id = Uuid::new_v4();
    seed_system_roles_and_policies(&company_id, &role_repo, &policy_repo, &audit)
        .await
        .unwrap();

    let policy = policy_repo
        .get_by_code("platform.scripts")
        .await
        .unwrap();
    assert!(policy.is_system);
    assert_eq!(policy.scope_type, PermissionScopeType::Platform);
    assert_eq!(
        policy.actions,
        vec!["script.manage".to_string(), "script.execute".to_string(), "script.read".to_string()]
    );
    assert_eq!(policy.priority, 90);
    assert_eq!(policy.entity_type, None);
    assert_eq!(policy.module_code, None);
    assert!(!policy.deny);

    // Политика ни к какой роли напрямую не привязана.
    let roles = role_repo.list().await.unwrap();
    assert_eq!(roles.len(), 4);
    assert!(roles.iter().all(|r| !r.permission_policy_codes.contains(&"platform.scripts".to_string())));
    let admin = role_repo.get_by_code(&company_id, "admin").await.unwrap();
    assert_eq!(admin.permission_policy_codes, vec!["platform.full".to_string()]);
}

/// Администратор проходит полный цикл: создание, выполнение (с args+object),
/// чтение скрипта через pipeline — политики `platform.full` + `script.*`.
#[tokio::test]
async fn admin_runs_script_lifecycle_through_pipeline() {
    let db = mem_db().await;
    let company_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let env = pipeline_env(&db, company_id, user_id, &["admin"]).await;

    let created = env
        .registry
        .execute_ctx(
            "script.create",
            json!({
                "code": "double.amount",
                "name": "Удвоить сумму",
                "source": "ctx.object.amount * ctx.args.factor",
                "company_id": company_id,
                "entity_type": "invoice",
            }),
            ctx(user_id, company_id),
        )
        .await
        .unwrap();
    assert_eq!(created["code"], "double.amount");
    assert_eq!(created["entity_type"], "invoice");
    assert_eq!(created["script_type"], "formula");

    let executed = env
        .registry
        .execute_ctx(
            "script.execute",
            json!({ "code": "double.amount", "object": { "amount": 10 }, "args": { "factor": 3 } }),
            ctx(user_id, company_id),
        )
        .await
        .unwrap();
    assert_eq!(executed, json!(30));

    let got = env
        .registry
        .execute_ctx(
            "script.get",
            json!({ "code": "double.amount", "company_id": company_id }),
            ctx(user_id, company_id),
        )
        .await
        .unwrap();
    assert_eq!(got["source"], "ctx.object.amount * ctx.args.factor");
}

/// Сотрудник (staff.objects: create/read/update без префикса) изолирован от скриптов:
/// не может ни исполнять, ни управлять ими (точный матчинг действий RBAC).
#[tokio::test]
async fn staff_denied_on_script_execute_and_manage() {
    let db = mem_db().await;
    let company_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let env = pipeline_env(&db, company_id, user_id, &["staff"]).await;

    for command in ["script.execute", "script.create"] {
        let err = env
            .registry
            .execute_ctx(command, json!({ "code": "x" }), ctx(user_id, company_id))
            .await
            .unwrap_err();
        assert!(
            err_contains(&err, "недостаточно прав"),
            "staff должен получить отказ на {command}: {err}"
        );
    }
}

/// Гость не читает скрипты; отказ фиксируется в аудите `permission.denied`.
#[tokio::test]
async fn guest_denied_on_script_read() {
    let db = mem_db().await;
    let company_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let env = pipeline_env(&db, company_id, user_id, &["guest"]).await;

    let err = env
        .registry
        .execute_ctx("script.get", json!({ "code": "x" }), ctx(user_id, company_id))
        .await
        .unwrap_err();
    assert!(
        err_contains(&err, "недостаточно прав"),
        "guest не должен читать скрипты: {err}"
    );

    let denied_log = env
        .audit
        .query(AuditFilter {
            action: Some("permission.denied".to_string()),
            company_id: Some(company_id),
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(denied_log.len(), 1);
}

/// Прямые валидации `execute_script`: привязка к типу требует object, отключённый
/// скрипт не запускается, object без привязки запрещён, успешный проход с args+object.
#[tokio::test]
async fn execute_script_validations_and_success() {
    let db = mem_db().await;
    let store = SurrealEventStore::new(db.clone());
    store.ensure_schema().await.unwrap();
    let audit = SurrealAuditRepository::new(db.clone());
    audit.ensure_schema().await.unwrap();
    let scripts = SurrealScriptRepository::new(db.clone());
    scripts.ensure_schema().await.unwrap();
    let engine = build_engine(&db);

    let company = Uuid::new_v4();
    let caller = actor(Uuid::new_v4(), company);

    let bound = Script {
        id: Uuid::new_v4(),
        code: "bound".to_string(),
        name: "Привязанный".to_string(),
        script_type: ScriptType::Formula,
        source: "ctx.object.amount * ctx.args.factor".to_string(),
        company_id: Some(company),
        module_code: None,
        entity_type: Some("invoice".to_string()),
        is_active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    scripts
        .create(&bound, &[script_event(&bound, "script.created")])
        .await
        .unwrap();

    let mut disabled = bound.clone();
    disabled.id = Uuid::new_v4();
    disabled.code = "disabled".to_string();
    disabled.is_active = false;
    disabled.entity_type = None;
    scripts
        .create(&disabled, &[script_event(&disabled, "script.created")])
        .await
        .unwrap();

    let mut free = bound.clone();
    free.id = Uuid::new_v4();
    free.code = "free".to_string();
    free.entity_type = None;
    scripts
        .create(&free, &[script_event(&free, "script.created")])
        .await
        .unwrap();

    let err = execute_script(&scripts, &engine, &json!({ "code": "bound" }), &caller)
        .await
        .unwrap_err();
    assert!(
        matches!(err, DomainError::ValidationError(_)) && err_contains(&err, "требуется передать object"),
        "привязанный скрипт без object: {err}"
    );

    let err = execute_script(
        &scripts,
        &engine,
        &json!({ "code": "disabled", "object": { "amount": 1 } }),
        &caller,
    )
    .await
    .unwrap_err();
    assert!(
        err_contains(&err, "отключён"),
        "отключённый скрипт не запускается: {err}"
    );

    let err = execute_script(
        &scripts,
        &engine,
        &json!({ "code": "free", "object": { "amount": 1 } }),
        &caller,
    )
    .await
    .unwrap_err();
    assert!(
        err_contains(&err, "object передавать нельзя"),
        "object без привязки к типу: {err}"
    );

    let value = execute_script(
        &scripts,
        &engine,
        &json!({ "code": "bound", "object": { "amount": 10 }, "args": { "factor": 3 } }),
        &caller,
    )
    .await
    .unwrap();
    assert_eq!(value, json!(30));

    let err = execute_script(&scripts, &engine, &json!({ "code": "unknown" }), &caller)
        .await
        .unwrap_err();
    assert!(
        matches!(err, DomainError::NotFound(_)),
        "неизвестный код: {err}"
    );

    let err = execute_script(&scripts, &engine, &json!({ "object": { "x": 1 } }), &caller)
        .await
        .unwrap_err();
    assert!(
        matches!(err, DomainError::ValidationError(_)),
        "обязательный code: {err}"
    );
}