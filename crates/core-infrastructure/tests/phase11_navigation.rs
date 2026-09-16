//! Интеграционные приёмочные тесты доработки перед Фазой 11b:
//! системная политика `platform.modules` + команда `module.navigation`.
//!
//! Сценарии: сидинг политики и её привязка к `staff`/`guest`, срез навигации
//! для админа, сотрудника и гостя, исключение недоступных/отключённых модулей,
//! отклонение анонима и актора без компании, идемпотентная миграция ролей.
//! Хранилища — `mem://` (kv-mem в dev-dependencies), WASM-фикстура
//! `accounting.wasm` (пересобрана после добавления `navigation` в манифест).

use std::sync::Arc;

use core_application::command_registry::{CommandExecutionCtx, CommandMetadata, CommandRegistry};
use core_application::module_manager::ModuleManager;
use core_application::permission_manager::PermissionManager;
use core_application::ports::{
    PermissionPolicyRepository, RoleRepository, UserRepository,
};
use core_application::seed::seed_system_roles_and_policies;
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::permission::{PermissionPolicy, PermissionScopeType, RecordAccessLevel};
use core_domain::role::Role;
use core_domain::user::{Person, User, UserStatus};
use core_infrastructure::extism_wasm_host::ExtismWasmHost;
use core_infrastructure::surreal_audit_repository::SurrealAuditRepository;
use core_infrastructure::surreal_metadata_repository::SurrealMetadataRepository;
use core_infrastructure::surreal_module_repository::SurrealModuleRepository;
use core_infrastructure::surreal_object_repository::SurrealObjectRepository;
use core_infrastructure::surreal_permission_policy_repository::SurrealPermissionPolicyRepository;
use core_infrastructure::surreal_role_repository::SurrealRoleRepository;
use core_infrastructure::surreal_script_repository::SurrealScriptRepository;
use core_infrastructure::surreal_user_repository::SurrealUserRepository;
use core_infrastructure::SurrealEventStore;
use serde_json::{json, Value};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

const ACCT_WASM: &[u8] = include_bytes!("fixtures/accounting.wasm");

async fn mem_db() -> Surreal<Any> {
    let db = surrealdb::engine::any::connect("mem://").await.unwrap();
    db.use_ns("test")
        .use_db(Uuid::new_v4().to_string())
        .await
        .unwrap();
    db
}

fn temp_cache() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("2c-11b-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Env {
    _db: Surreal<Any>,
    manager: Arc<ModuleManager>,
    roles: Arc<SurrealRoleRepository>,
    policies: Arc<SurrealPermissionPolicyRepository>,
    users: Arc<SurrealUserRepository>,
    audit: Arc<SurrealAuditRepository>,
    permissions: Arc<PermissionManager>,
    company_id: Uuid,
}

async fn setup() -> Env {
    let db = mem_db().await;

    let store = Arc::new(SurrealEventStore::new(db.clone()));
    store.ensure_schema().await.unwrap();
    let objects = Arc::new(SurrealObjectRepository::new(db.clone()));
    objects.ensure_schema().await.unwrap();
    let metadata = Arc::new(SurrealMetadataRepository::new(db.clone()));
    metadata.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();
    let policies = Arc::new(SurrealPermissionPolicyRepository::new(db.clone()));
    policies.ensure_schema().await.unwrap();
    let modules = Arc::new(SurrealModuleRepository::new(db.clone()));
    modules.ensure_schema().await.unwrap();
    let users = Arc::new(SurrealUserRepository::new(db.clone()));
    users.ensure_schema().await.unwrap();
    let roles = Arc::new(SurrealRoleRepository::new(db.clone()));
    roles.ensure_schema().await.unwrap();

    let transactions =
        core_application::TransactionOrchestrator::new(objects.clone(), metadata.clone());
    let scripts = Arc::new(SurrealScriptRepository::new(db.clone()));
    scripts.ensure_schema().await.unwrap();
    let script_engine = Arc::new(
        core_infrastructure::RhaiScriptEngine::new(core_infrastructure::rhai_core_api::CoreApiShared {
            store: store.clone(),
            db: db.clone(),
            audit: audit.clone(),
            runtime: tokio::runtime::Handle::current(),
        })
        .unwrap(),
    );
    let host = Arc::new(
        ExtismWasmHost::new(
            db.clone(),
            objects.as_ref().clone(),
            metadata.as_ref().clone(),
            store.as_ref().clone(),
            users.as_ref().clone(),
            transactions,
            script_engine,
            temp_cache(),
        )
        .unwrap(),
    );
    host.ensure_schema().await.unwrap();

    let app = Arc::new(core_application::AppRegistry::new());
    let manager = Arc::new(ModuleManager::new(
        host,
        modules.clone(),
        app,
        policies.clone(),
        metadata,
        scripts,
        audit.clone(),
        temp_cache(),
    ));

    let company_id = Uuid::new_v4();
    seed_system_roles_and_policies(&company_id, roles.as_ref(), policies.as_ref(), audit.as_ref())
        .await
        .unwrap();

    let permissions = Arc::new(PermissionManager::new(roles.clone(), policies.clone()));

    Env {
        _db: db,
        manager,
        roles,
        policies,
        users,
        audit,
        permissions,
        company_id,
    }
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

async fn create_user(
    env: &Env,
    login: &str,
    role_codes: &[&str],
) -> (Uuid, ActorSnapshot) {
    let user_id = Uuid::new_v4();
    let role_ids: Vec<String> = env
        .roles
        .list()
        .await
        .unwrap()
        .iter()
        .filter(|r| role_codes.contains(&r.code.as_str()))
        .map(|r| r.id.to_string())
        .collect();
    let person = Person {
        id: user_id,
        user_id,
        last_name: "Тестов".to_string(),
        first_name: login.to_string(),
        middle_name: None,
        display_name: login.to_string(),
    };
    let user = User {
        id: user_id,
        login: login.to_string(),
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
    env.users
        .create(&user, &person, &[user_event(&user, env.company_id)])
        .await
        .unwrap();
    let actor = ActorSnapshot {
        user_id: Some(user_id),
        login: login.to_string(),
        full_name: login.to_string(),
        position: None,
        company_id: Some(env.company_id),
        ip_address: None,
    };
    (user_id, actor)
}

async fn role_by_code(env: &Env, code: &str) -> Role {
    env.roles
        .get_by_code(&env.company_id, code)
        .await
        .unwrap()
}

/// Предоставляет роли прямой доступ к команде модуля, добавив политику с
/// действием, равным `required_permission` (например `accounting.read`).
async fn grant_role_module_action(env: &Env, role_code: &str, module_code: &str, action: &str) {
    let policy_code = format!("test.{module_code}.{action}");
    let now = chrono::Utc::now();
    let policy = PermissionPolicy {
        id: Uuid::new_v4(),
        code: policy_code.clone(),
        name: policy_code.clone(),
        description: None,
        scope_type: PermissionScopeType::Module(module_code.to_string()),
        entity_type: None,
        actions: vec![action.to_string()],
        record_access: RecordAccessLevel::All,
        deny: false,
        priority: 0,
        module_code: Some(module_code.to_string()),
        is_system: false,
        created_at: now,
        updated_at: now,
    };
    env.policies.upsert(&policy).await.unwrap();

    let role = role_by_code(env, role_code).await;
    let mut updated = role.clone();
    updated.permission_policy_codes.push(policy_code);
    env.roles.update(&updated, &[]).await.unwrap();
}

#[tokio::test]
async fn seed_creates_platform_modules_policy_bound_to_staff_and_guest() {
    let env = setup().await;

    let policy = env
        .policies
        .get_by_code("platform.modules")
        .await
        .unwrap();
    assert_eq!(policy.scope_type, PermissionScopeType::Platform);
    assert_eq!(policy.actions, vec!["module.read".to_string()]);
    assert_eq!(policy.record_access, RecordAccessLevel::ByCompany);
    assert_eq!(policy.priority, 40);
    assert!(policy.is_system);

    for code in ["staff", "guest"] {
        let role = role_by_code(&env, code).await;
        assert!(
            role.permission_policy_codes
                .iter()
                .any(|c| c == "platform.modules"),
            "роль {code} должна иметь platform.modules"
        );
    }
    let admin = role_by_code(&env, "admin").await;
    assert!(!admin.permission_policy_codes.iter().any(|c| c == "platform.modules"));
    let archived = role_by_code(&env, "archived").await;
    assert!(!archived.permission_policy_codes.iter().any(|c| c == "platform.modules"));
}

#[tokio::test]
async fn admin_sees_all_enabled_modules_with_navigation() {
    let env = setup().await;
    env.manager
        .install("accounting", ACCT_WASM, &env.company_id.to_string())
        .await
        .unwrap();
    let (_, admin_actor) = create_user(&env, "admin", &["admin"]).await;

    let out = env
        .manager
        .get_navigation(env.company_id, Some(admin_actor), &env.permissions)
        .await
        .unwrap();
    let modules = out["modules"].as_array().unwrap();
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0]["code"], "platform");
    let accounting = &modules[1];
    assert_eq!(accounting["code"], "accounting");
    assert_eq!(
        accounting["display_name"],
        json!("Управленческий учёт")
    );
    assert!(accounting["version"].is_string());
    let nav = accounting["navigation"].as_array().unwrap();
    assert_eq!(nav.len(), 3);
    let codes: Vec<&str> = nav
        .iter()
        .map(|n| n["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, vec!["accounts", "periods", "entries"]);
    assert_eq!(nav[0]["entity_type"], "account");
    assert_eq!(nav[0]["label"], "План счетов");
    // навигация не содержит внутренностей манифеста
    assert!(accounting.get("manifest").is_none());
    assert!(accounting.get("wasm_sha256").is_none());
    assert!(accounting.get("capabilities").is_none());
}

#[tokio::test]
async fn staff_sees_modules_with_accessible_commands() {
    let env = setup().await;
    env.manager
        .install("accounting", ACCT_WASM, &env.company_id.to_string())
        .await
        .unwrap();
    grant_role_module_action(&env, "staff", "accounting", "accounting.read").await;
    let (_, staff_actor) = create_user(&env, "staff", &["staff"]).await;

    let out = env
        .manager
        .get_navigation(env.company_id, Some(staff_actor), &env.permissions)
        .await
        .unwrap();
    let modules = out["modules"].as_array().unwrap();
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0]["code"], "platform");
    assert_eq!(modules[1]["code"], "accounting");
    assert_eq!(modules[1]["navigation"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn module_without_accessible_commands_is_excluded() {
    let env = setup().await;
    env.manager
        .install("accounting", ACCT_WASM, &env.company_id.to_string())
        .await
        .unwrap();
    let (_, guest_actor) = create_user(&env, "guest", &["guest"]).await;

    let out = env
        .manager
        .get_navigation(env.company_id, Some(guest_actor), &env.permissions)
        .await
        .unwrap();
    let modules = out["modules"].as_array().unwrap();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0]["code"], "platform");
}

#[tokio::test]
async fn disabled_module_is_absent() {
    let env = setup().await;
    env.manager
        .install("accounting", ACCT_WASM, &env.company_id.to_string())
        .await
        .unwrap();
    env.manager
        .disable("accounting", &env.company_id.to_string())
        .await
        .unwrap();
    let (_, admin_actor) = create_user(&env, "root", &["admin"]).await;

    let out = env
        .manager
        .get_navigation(env.company_id, Some(admin_actor), &env.permissions)
        .await
        .unwrap();
    assert_eq!(out["modules"].as_array().unwrap().len(), 1);
    assert_eq!(out["modules"][0]["code"], "platform");
}

#[tokio::test]
async fn anonymous_gets_permission_error() {
    let env = setup().await;
    let registry = CommandRegistry::new();
    registry
        .attach_pipeline(env.audit.clone(), env.permissions.clone())
        .await;
    registry
        .register_with_metadata(
            "module.navigation",
            CommandMetadata::requires("module.read"),
            |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({ "modules": [] })) },
        )
        .await;

    let err = registry
        .execute_ctx(
            "module.navigation",
            json!({}),
            CommandExecutionCtx {
                actor: Some(ActorSnapshot::anonymous()),
                module_code: None,
                entity_type: None,
            },
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, DomainError::PermissionDenied(_)),
        "ожидали PermissionDenied, получено: {err}"
    );
    assert_eq!(err.code(), "PERMISSION_ERROR");
}

#[tokio::test]
async fn actor_without_company_gets_permission_error() {
    let env = setup().await;
    let registry = CommandRegistry::new();
    registry
        .attach_pipeline(env.audit.clone(), env.permissions.clone())
        .await;
    registry
        .register_with_metadata(
            "module.navigation",
            CommandMetadata::requires("module.read"),
            |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({ "modules": [] })) },
        )
        .await;

    let actor = ActorSnapshot {
        user_id: Some(Uuid::new_v4()),
        login: "orphan".to_string(),
        full_name: "Без компании".to_string(),
        position: None,
        company_id: None,
        ip_address: None,
    };
    let err = registry
        .execute_ctx(
            "module.navigation",
            json!({}),
            CommandExecutionCtx {
                actor: Some(actor),
                module_code: None,
                entity_type: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::PermissionDenied(_)));
}

#[tokio::test]
async fn migrate_supplements_existing_roles_idempotently() {
    let env = setup().await;

    // Имитация докомиграционной роли: убираем platform.modules у staff.
    let staff = role_by_code(&env, "staff").await;
    let mut staff_without = staff.clone();
    staff_without
        .permission_policy_codes
        .retain(|c| c != "platform.modules");
    env.roles.update(&staff_without, &[]).await.unwrap();

    let first = seed_system_roles_and_policies(
        &env.company_id,
        env.roles.as_ref(),
        env.policies.as_ref(),
        env.audit.as_ref(),
    )
    .await
    .unwrap();
    assert!(
        first.policies_added >= 1,
        "должна быть добавлена минимум одна политика"
    );
    assert!(role_by_code(&env, "staff")
        .await
        .permission_policy_codes
        .iter()
        .any(|c| c == "platform.modules"));

    let second = seed_system_roles_and_policies(
        &env.company_id,
        env.roles.as_ref(),
        env.policies.as_ref(),
        env.audit.as_ref(),
    )
    .await
    .unwrap();
    assert_eq!(second.policies_added, 0, "повторный сид ничего не добавляет");
}