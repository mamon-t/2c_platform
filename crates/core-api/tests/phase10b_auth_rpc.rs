//! Интеграционные приёмочные тесты подфазы 10b (раздел 10 ТЗ v3.1):
//! JWT-аутентификация и RBAC поверх транспортного конверта `RpcMessage`.
//!
//! На мем-базе (`mem://`) проверяются: вход в систему по логину/паролю
//! (Argon2id), аудит `user.login`/`user.login_failed`/`user.logout`,
//! блокировка после 5 неудачных попыток, разрешение Bearer-токена
//! и отказ анонимных запросов к командам с `required_permission`.

use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use core_api::idempotency::IdempotencyStore;
use core_api::routes::ApiState;
use core_api::{JwtConfig, JwtTokenManager, RpcMessage, router};
use core_application::auth::AuthService;
use core_application::command_registry::{CommandMetadata, CommandRegistry};
use core_application::permission_manager::PermissionManager;
use core_application::ports::{AuditRepository, RoleRepository, TokenManager, UserRepository};
use core_application::seed::seed_system_roles_and_policies;
use core_domain::audit::{AuditFilter, AuditResult};
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::password::hash_password;
use core_domain::user::{Person, User, UserCompanyProfile, UserStatus};
use core_infrastructure::{
    SurrealAuditRepository, SurrealEventStore, SurrealPermissionPolicyRepository,
    SurrealRoleRepository, SurrealUserRepository,
};
use serde_json::{Value, json};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use tower::ServiceExt;
use uuid::Uuid;

const SECRET: &str = "test-secret";

async fn mem_db() -> Surreal<Any> {
    let db = surrealdb::engine::any::connect("mem://").await.unwrap();
    db.use_ns("test")
        .use_db(Uuid::new_v4().to_string())
        .await
        .unwrap();
    db
}

struct Env {
    store: Arc<SurrealEventStore>,
    registry: Arc<CommandRegistry>,
    idempotency: Arc<IdempotencyStore>,
    tokens: Arc<dyn TokenManager>,
    users: Arc<SurrealUserRepository>,
    audit: Arc<SurrealAuditRepository>,
    company_id: Uuid,
    pushes: Arc<core_api::PushHub>,
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
    users: &SurrealUserRepository,
    id: Uuid,
    company_id: Uuid,
    login: &str,
    password: &str,
    role_ids: Vec<String>,
) {
    let person = Person {
        id,
        user_id: id,
        last_name: "Тестов".to_string(),
        first_name: "Тест".to_string(),
        middle_name: None,
        display_name: format!("Тест {login}"),
    };
    let password_hash = hash_password(password).unwrap();
    let user = User {
        id,
        login: login.to_string(),
        password_hash,
        status: UserStatus::Active,
        role_ids,
        failed_login_count: 0,
        locked_until: None,
        must_change_password: false,
        locale: "ru-RU".to_string(),
        timezone: "Europe/Moscow".to_string(),
        person_id: Some(id),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    users
        .create(&user, &person, &[user_event(&user, company_id)])
        .await
        .unwrap();
    users
        .add_profile(
            &UserCompanyProfile {
                id: Uuid::new_v4(),
                user_id: id,
                company_id,
                employee_number: None,
                position: Some("Тестировщик".to_string()),
                department: None,
                is_primary: true,
                is_active: true,
                valid_from: None,
                valid_to: None,
            },
            &[],
        )
        .await
        .unwrap();
}

async fn setup() -> Env {
    let db = mem_db().await;

    let store = Arc::new(SurrealEventStore::new(db.clone()));
    store.ensure_schema().await.unwrap();
    let role_repo = SurrealRoleRepository::new(db.clone());
    role_repo.ensure_schema().await.unwrap();
    let policy_repo = SurrealPermissionPolicyRepository::new(db.clone());
    policy_repo.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();
    let users = Arc::new(SurrealUserRepository::new(db.clone()));
    users.ensure_schema().await.unwrap();

    let company_id = Uuid::new_v4();
    seed_system_roles_and_policies(&company_id, &role_repo, &policy_repo, audit.as_ref())
        .await
        .unwrap();

    let roles = role_repo.list().await.unwrap();
    let role_id = |code: &str| {
        roles
            .iter()
            .find(|r| r.code == code)
            .unwrap_or_else(|| panic!("роль {code} не найдена"))
            .id
            .to_string()
    };
    let boss_id = Uuid::new_v4();
    create_user(&users, boss_id, company_id, "boss", SECRET, vec![role_id("admin")]).await;
    let clerk_id = Uuid::new_v4();
    create_user(&users, clerk_id, company_id, "clerk", SECRET, vec![role_id("staff")]).await;

    let registry = Arc::new(CommandRegistry::new());
    registry
        .register_with_metadata(
            "core.sample.secret",
            CommandMetadata::requires("role.manage"),
            |_: Value| async move { Ok(json!({"secret": true})) },
        )
        .await;

    let permissions = Arc::new(PermissionManager::new(Arc::new(role_repo), Arc::new(policy_repo)));
    registry.attach_pipeline(audit.clone(), permissions).await;

    let tokens: Arc<dyn TokenManager> = Arc::new(JwtTokenManager::new(JwtConfig {
        secret: SECRET.to_string(),
        access_ttl: Duration::from_secs(600),
    }));
    let auth = Arc::new(AuthService::new(users.clone(), audit.clone(), tokens.clone()));
    register_auth_commands(&registry, auth).await;

    Env {
        store,
        registry,
        idempotency: IdempotencyStore::new(),
        tokens,
        users,
        audit,
        company_id,
        pushes: core_api::PushHub::new(),
    }
}

/// Реплика производственной регистрации команд фазы 10b (platform-server).
async fn register_auth_commands(registry: &CommandRegistry, auth: Arc<AuthService>) {
    registry
        .register_with_metadata("user.login", CommandMetadata::unrestricted(), {
            let auth = auth.clone();
            move |params: Value| {
                let auth = auth.clone();
                async move {
                    let login = params
                        .get("login")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .ok_or_else(|| {
                            DomainError::ValidationError("отсутствует логин".to_string())
                        })?;
                    let password = params
                        .get("password")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .ok_or_else(|| {
                            DomainError::ValidationError("отсутствует пароль".to_string())
                        })?;
                    let token = auth.login(&login, &password, None, None).await?;
                    serde_json::to_value(token)
                        .map_err(|e| DomainError::ValidationError(format!("сериализация: {e}")))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("user.logout", CommandMetadata::unrestricted(), {
            let auth = auth.clone();
            move |_: Value| {
                let auth = auth.clone();
                async move {
                    auth.logout(None, None, None).await?;
                    Ok(json!({ "logout": true }))
                }
            }
        })
        .await;
}

fn api_state(env: &Env) -> ApiState {
    ApiState {
        registry: env.registry.clone(),
        store: env.store.clone(),
        idempotency: env.idempotency.clone(),
        tokens: env.tokens.clone(),
        pushes: env.pushes.clone(),
    }
}

async fn dispatch(env: &Env, body: Value, bearer: Option<&str>) -> (StatusCode, RpcMessage) {
    let mut builder = Request::builder().method("POST").uri("/rpc");
    builder = builder.header("content-type", "application/json");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router(api_state(env))
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let message = serde_json::from_slice(&bytes).unwrap();
    (status, message)
}

async fn post_rpc(env: &Env, body: Value) -> (StatusCode, RpcMessage) {
    dispatch(env, body, None).await
}

fn login_body(id: &str, login: &str, password: &str) -> Value {
    json!({
        "type": "command",
        "id": id,
        "module": "core",
        "action": "user.login",
        "payload": {"login": login, "password": password}
    })
}

fn secret_body(id: &str) -> Value {
    json!({
        "type": "command",
        "id": id,
        "module": "core",
        "action": "core.sample.secret",
        "payload": {}
    })
}

async fn login_failed_count(env: &Env) -> usize {
    env.audit
        .query(AuditFilter {
            action: Some("user.login_failed".to_string()),
            company_id: None,
            ..AuditFilter::default()
        })
        .await
        .unwrap()
        .len()
}

#[tokio::test]
async fn login_success_returns_token_and_audits() {
    let env = setup().await;
    let (status, message) = post_rpc(&env, login_body("l1", "boss", SECRET)).await;
    assert_eq!(status, StatusCode::OK);
    let RpcMessage::Response { id, payload } = message else {
        panic!("ожидали Response, получено: {message:?}");
    };
    assert_eq!(id, "l1");
    let token = payload["access_token"].as_str().expect("access_token");
    assert!(!token.is_empty());
    assert_eq!(payload["token_type"], "Bearer");
    assert!(payload["expires_at"].as_str().is_some());

    let parsed = env.tokens.parse(token).unwrap();
    assert_eq!(parsed.login, "boss");
    assert!(parsed.company_id.is_some());
}

#[tokio::test]
async fn login_wrong_password_is_rejected_and_audited() {
    let env = setup().await;
    let (status, message) = post_rpc(&env, login_body("l2", "boss", "wrong-pass")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(matches!(message, RpcMessage::Error { code, .. } if code == "VALIDATION_ERROR"));
    assert_eq!(login_failed_count(&env).await, 1);
}

#[tokio::test]
async fn login_unknown_user_is_rejected_without_enumeration() {
    let env = setup().await;
    let (status, message) = post_rpc(&env, login_body("l3", "ghost", SECRET)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(matches!(message, RpcMessage::Error { code, .. } if code == "VALIDATION_ERROR"));
    assert_eq!(login_failed_count(&env).await, 1);
}

#[tokio::test]
async fn five_failed_attempts_lock_the_account() {
    let env = setup().await;
    for i in 0..5 {
        let (status, _) = post_rpc(&env, login_body(&format!("bad-{i}"), "boss", "wrong")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(login_failed_count(&env).await, 5);

    // Даже корректный пароль отклоняется, пока действует блокировка.
    let (status, message) = post_rpc(&env, login_body("good-after", "boss", SECRET)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(matches!(message, RpcMessage::Error { code, .. } if code == "VALIDATION_ERROR"));

    let locked = env.users.get_by_login("boss").await.unwrap();
    assert!(locked.locked_until.is_some());
}

#[tokio::test]
async fn anonymous_request_to_protected_command_is_denied() {
    let env = setup().await;
    let (status, message) = post_rpc(&env, secret_body("s1")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(matches!(
        message,
        RpcMessage::Error {
            code, ..
        } if code == "PERMISSION_ERROR"
    ));
}

#[tokio::test]
async fn system_token_bypasses_permission_check() {
    let env = setup().await;
    let token = env.tokens.issue(&ActorSnapshot::system()).unwrap();
    let (status, message) = dispatch(&env, secret_body("s2"), Some(&token.access_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        message,
        RpcMessage::Response {
            id: "s2".to_string(),
            payload: json!({"secret": true})
        }
    );
}

#[tokio::test]
async fn staff_token_is_denied_admin_token_is_allowed() {
    let env = setup().await;

    let staff = env.users.get_by_login("clerk").await.unwrap();
    let staff_actor = ActorSnapshot {
        user_id: Some(staff.id),
        login: staff.login,
        full_name: "Тест clerk".to_string(),
        position: None,
        company_id: Some(env.company_id),
        ip_address: None,
    };
    let staff_token = env.tokens.issue(&staff_actor).unwrap();
    let (status, message) = dispatch(&env, secret_body("s3"), Some(&staff_token.access_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(matches!(message, RpcMessage::Error { code, .. } if code == "PERMISSION_ERROR"));

    let boss = env.users.get_by_login("boss").await.unwrap();
    let boss_actor = ActorSnapshot {
        user_id: Some(boss.id),
        login: boss.login,
        full_name: "Тест boss".to_string(),
        position: None,
        company_id: Some(env.company_id),
        ip_address: None,
    };
    let boss_token = env.tokens.issue(&boss_actor).unwrap();
    let (status, message) = dispatch(&env, secret_body("s4"), Some(&boss_token.access_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(matches!(message, RpcMessage::Response { .. }));
}

#[tokio::test]
async fn logout_command_returns_ok_and_audits() {
    let env = setup().await;
    let body = json!({
        "type": "command",
        "id": "lo-1",
        "module": "core",
        "action": "user.logout",
        "payload": {}
    });
    let (status, message) = post_rpc(&env, body).await;
    assert_eq!(status, StatusCode::OK);
    assert!(matches!(message, RpcMessage::Response { .. }));

    let logouts = env
        .audit
        .query(AuditFilter {
            action: Some("user.logout".to_string()),
            company_id: None,
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(logouts.len(), 1);
    assert!(matches!(logouts[0].result, AuditResult::Success));
}