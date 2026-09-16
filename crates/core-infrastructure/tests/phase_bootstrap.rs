//! Интеграционные приёмочные тесты команды `system.bootstrap`.
//!
//! Сценарии: полный цикл инициализации платформы (компания, суперадмин,
//! системные роли/политики, привязка к роли `admin`, аудит), идемпотентность
//! (повторный вызов и коллизия кода), отклонение анонима пайплайном, вход
//! в систему после бутстрапа и отказ при неверном пароле.
//! Хранилища — `mem://` (kv-mem в dev-dependencies).

use std::sync::Arc;

use core_application::auth::AuthService;
use core_application::bootstrap::{bootstrap_platform, BootstrapParams};
use core_application::command_registry::{CommandExecutionCtx, CommandMetadata, CommandRegistry};
use core_application::permission_manager::PermissionManager;
use core_application::ports::{
    AuditRepository, AuthToken, CompanyRepository, MetadataRepository, PermissionPolicyRepository,
    RoleRepository, TokenManager, UserRepository,
};
use core_domain::audit::{AuditFilter, AuditResult};
use core_domain::company::Company;
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_application::metadata_seed::system_metadata_type_count;
use core_infrastructure::surreal_audit_repository::SurrealAuditRepository;
use core_infrastructure::surreal_company_repository::SurrealCompanyRepository;
use core_infrastructure::surreal_metadata_repository::SurrealMetadataRepository;
use core_infrastructure::surreal_permission_policy_repository::SurrealPermissionPolicyRepository;
use core_infrastructure::surreal_role_repository::SurrealRoleRepository;
use core_infrastructure::surreal_user_repository::SurrealUserRepository;
use serde_json::json;
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

struct Env {
    _db: Surreal<Any>,
    companies: Arc<SurrealCompanyRepository>,
    users: Arc<SurrealUserRepository>,
    roles: Arc<SurrealRoleRepository>,
    policies: Arc<SurrealPermissionPolicyRepository>,
    audit: Arc<SurrealAuditRepository>,
    metadata: Arc<SurrealMetadataRepository>,
}

async fn setup() -> Env {
    let db = mem_db().await;

    let store = Arc::new(core_infrastructure::SurrealEventStore::new(db.clone()));
    store.ensure_schema().await.unwrap();
    let companies = Arc::new(SurrealCompanyRepository::new(db.clone()));
    companies.ensure_schema().await.unwrap();
    let users = Arc::new(SurrealUserRepository::new(db.clone()));
    users.ensure_schema().await.unwrap();
    let roles = Arc::new(SurrealRoleRepository::new(db.clone()));
    roles.ensure_schema().await.unwrap();
    let policies = Arc::new(SurrealPermissionPolicyRepository::new(db.clone()));
    policies.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();
    let metadata = Arc::new(SurrealMetadataRepository::new(db.clone()));
    metadata.ensure_schema().await.unwrap();

    Env {
        _db: db,
        companies,
        users,
        roles,
        policies,
        audit,
        metadata,
    }
}

#[derive(Debug)]
struct MockTokenManager;

impl TokenManager for MockTokenManager {
    fn issue(&self, _actor: &ActorSnapshot) -> Result<AuthToken, DomainError> {
        Ok(AuthToken {
            access_token: "test-token".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(8),
            token_type: "Bearer",
        })
    }

    fn parse(&self, _token: &str) -> Result<ActorSnapshot, DomainError> {
        Err(DomainError::PermissionDenied("mock".to_string()))
    }
}

fn company_event(company: &Company) -> Event {
    Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::Company,
        stream_id: company.id.to_string(),
        event_type: "company.created".to_string(),
        version: 0,
        payload: serde_json::to_value(company).unwrap(),
        metadata: ActorSnapshot::system(),
        company_id: company.id.to_string(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    }
}

async fn params(code: &str, login: &str) -> BootstrapParams {
    BootstrapParams {
        company_code: code.to_string(),
        company_name: "Acme Corp".to_string(),
        admin_login: login.to_string(),
        admin_password: "secret123".to_string(),
        admin_last_name: Some("Алексеев".to_string()),
        admin_first_name: Some("Михаил".to_string()),
    }
}

#[tokio::test]
async fn full_bootstrap_cycle_creates_company_admin_roles_and_binds() {
    let env = setup().await;

    let res = bootstrap_platform(
        params("acme", "root").await,
        env.companies.as_ref(),
        env.users.as_ref(),
        env.roles.as_ref(),
        env.policies.as_ref(),
        env.audit.as_ref(),
        env.metadata.as_ref(),
    )
    .await
    .unwrap();

    // Ответ команды.
    assert_eq!(res.company_code, "acme");
    assert_eq!(res.admin_login, "root");
    assert_eq!(res.roles_seeded, 4);
    assert_eq!(res.policies_seeded, 6);
    assert_eq!(res.metadata_entity_types_seeded, system_metadata_type_count());

    // Компания создана.
    let company = env.companies.get(&res.company_id).await.unwrap();
    assert_eq!(company.name, "Acme Corp");

    // Пользователь создан и привязан к системной роли admin.
    let user = env.users.get_by_login("root").await.unwrap();
    assert_eq!(user.id, res.admin_user_id);
    assert!(user.must_change_password);
    let admin_role = env
        .roles
        .get_by_code(&res.company_id, "admin")
        .await
        .unwrap();
    assert!(
        user.role_ids.contains(&admin_role.id.to_string()),
        "админ должен быть привязан к роли admin"
    );

    // Роли и политики засижены.
    let roles = env.roles.list().await.unwrap();
    assert_eq!(roles.len(), 4);
    for code in ["admin", "staff", "guest", "archived"] {
        env.roles.get_by_code(&res.company_id, code).await.unwrap();
    }
    for code in [
        "platform.full",
        "staff.objects",
        "guest.objects",
        "archived.objects",
        "platform.scripts",
        "platform.modules",
    ] {
        env.policies.get_by_code(code).await.unwrap();
    }

    // Primary-профиль для JWT company_id.
    let profiles = env.users.list_profiles(&res.admin_user_id).await.unwrap();
    assert_eq!(profiles.len(), 1);
    assert!(profiles[0].is_primary);
    assert_eq!(profiles[0].company_id, res.company_id);

    // Системные типы метаданных засижены.
    let types = env.metadata.list_entity_types().await.unwrap();
    assert_eq!(types.len(), system_metadata_type_count());
    for code in [
        "company",
        "user",
        "role",
        "entity_type",
        "entity_field",
        "entity_state",
        "entity_transition",
    ] {
        let et = env.metadata.get_entity_type_by_code("", code).await.unwrap();
        assert!(et.is_system, "тип {code} должен быть системным");
    }

    // Схема системного типа собрана полностью: у `user` есть состояния и переход.
    let user_schema = env.metadata.get_schema("", "user").await.unwrap();
    assert_eq!(user_schema.states.len(), 2);
    assert_eq!(user_schema.transitions.len(), 1);
    assert_eq!(user_schema.transitions[0].from_state, "active");
    assert_eq!(user_schema.transitions[0].to_state, "blocked");

    // Аудит: запись system.bootstrap с деталями.
    let entries = env
        .audit
        .query(AuditFilter {
            action: Some("system.bootstrap".to_string()),
            actor_user_id: None,
            target_entity_type: None,
            target_entity_id: None,
            company_id: None,
            result_success: None,
            from: None,
            to: None,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert!(matches!(entry.result, AuditResult::Success));
    assert_eq!(entry.details.as_ref().unwrap()["company_code"], "acme");
    assert_eq!(entry.details.as_ref().unwrap()["admin_login"], "root");
    assert_eq!(
        entry.details.as_ref().unwrap()["metadata_entity_types_seeded"],
        7
    );
}

#[tokio::test]
async fn second_bootstrap_call_is_rejected() {
    let env = setup().await;

    bootstrap_platform(
        params("acme", "root").await,
        env.companies.as_ref(),
        env.users.as_ref(),
        env.roles.as_ref(),
        env.policies.as_ref(),
        env.audit.as_ref(),
        env.metadata.as_ref(),
    )
    .await
    .unwrap();

    let err = bootstrap_platform(
        params("other", "root2").await,
        env.companies.as_ref(),
        env.users.as_ref(),
        env.roles.as_ref(),
        env.policies.as_ref(),
        env.audit.as_ref(),
        env.metadata.as_ref(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, DomainError::ValidationError(m) if m.contains("уже инициализирована")),
        "ожидали ValidationError, получено: {err}"
    );
}

#[tokio::test]
async fn bootstrap_with_existing_company_code_is_rejected() {
    let env = setup().await;

    let now = chrono::Utc::now();
    let company = Company {
        id: Uuid::new_v4(),
        code: "acme".to_string(),
        name: "Ранняя компания".to_string(),
        is_active: true,
        created_at: now,
        updated_at: now,
    };
    env.companies.create(&company, &[company_event(&company)]).await.unwrap();

    let err = bootstrap_platform(
        params("acme", "root").await,
        env.companies.as_ref(),
        env.users.as_ref(),
        env.roles.as_ref(),
        env.policies.as_ref(),
        env.audit.as_ref(),
        env.metadata.as_ref(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, DomainError::ValidationError(m) if m.contains("Компания acme уже существует")),
        "ожидали ValidationError о коллизии кода, получено: {err}"
    );
}

#[tokio::test]
async fn anonymous_gets_permission_error() {
    let env = setup().await;
    let permissions = Arc::new(PermissionManager::new(
        env.roles.clone(),
        env.policies.clone(),
    ));
    let registry = CommandRegistry::new();
    registry
        .attach_pipeline(env.audit.clone(), permissions)
        .await;
    registry
        .register_with_metadata(
            "system.bootstrap",
            CommandMetadata::requires("system.bootstrap"),
            |_: serde_json::Value, _ctx: CommandExecutionCtx| async move {
                Ok(json!({ "ok": true }))
            },
        )
        .await;

    let err = registry
        .execute_ctx(
            "system.bootstrap",
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
async fn login_after_bootstrap_succeeds() {
    let env = setup().await;

    bootstrap_platform(
        params("acme", "root").await,
        env.companies.as_ref(),
        env.users.as_ref(),
        env.roles.as_ref(),
        env.policies.as_ref(),
        env.audit.as_ref(),
        env.metadata.as_ref(),
    )
    .await
    .unwrap();

    let users: Arc<dyn UserRepository> = env.users.clone();
    let audit: Arc<dyn AuditRepository> = env.audit.clone();
    let tokens: Arc<dyn TokenManager> = Arc::new(MockTokenManager);
    let auth = AuthService::new(users, audit, tokens);

    let token = auth.login("root", "secret123", None, None).await.unwrap();
    assert_eq!(token.access_token, "test-token");
    assert_eq!(token.token_type, "Bearer");
}

#[tokio::test]
async fn login_with_wrong_password_is_rejected() {
    let env = setup().await;

    bootstrap_platform(
        params("acme", "root").await,
        env.companies.as_ref(),
        env.users.as_ref(),
        env.roles.as_ref(),
        env.policies.as_ref(),
        env.audit.as_ref(),
        env.metadata.as_ref(),
    )
    .await
    .unwrap();

    let users: Arc<dyn UserRepository> = env.users.clone();
    let audit: Arc<dyn AuditRepository> = env.audit.clone();
    let tokens: Arc<dyn TokenManager> = Arc::new(MockTokenManager);
    let auth = AuthService::new(users, audit, tokens);

    let err = auth.login("root", "wrong-password", None, None).await.unwrap_err();
    assert!(
        matches!(&err, DomainError::ValidationError(m) if m.contains("неверный логин или пароль")),
        "ожидали ValidationError, получено: {err}"
    );
}