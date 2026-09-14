//! Интеграционные приёмочные тесты RBAC (Фаза 5 по ТЗ v3.1, Приложения №7 и §8.5):
//! сидинг системных ролей, `PermissionManager` и `CommandExecutionPipeline` на полном
//! стеке хранилищ `mem://` (EventStore + роли + политики + аудит + пользователи).
//!
//! Сценарии приёмки:
//! 1. Сидинг ролей/политик идемпотентен и наполняет системные наборы.
//! 2. Администратор имеет полный доступ (политика `platform.full` закрывает любые действия).
//! 3. Сотрудник работает с объектами, но лишён административных прав.
//! 4. Гость имеет только чтение; запись запрещена с аудитом `permission.denied`.

use std::collections::HashSet;

use core_application::command_registry::{CommandExecutionCtx, CommandMetadata, CommandRegistry};
use core_application::permission_manager::PermissionManager;
use core_application::ports::{
    AuditRepository, CompanyRepository, PermissionPolicyRepository, RoleRepository, UserRepository,
};
use core_application::seed::seed_system_roles_and_policies;
use core_domain::audit::AuditFilter;
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::user::{Person, User, UserStatus};
use core_infrastructure::surreal_audit_repository::SurrealAuditRepository;
use core_infrastructure::surreal_company_repository::SurrealCompanyRepository;
use core_infrastructure::surreal_event_store::SurrealEventStore;
use core_infrastructure::surreal_permission_policy_repository::SurrealPermissionPolicyRepository;
use core_infrastructure::surreal_role_repository::SurrealRoleRepository;
use core_infrastructure::surreal_user_repository::SurrealUserRepository;
use serde_json::{json, Value};
use std::sync::Arc;
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

/// Создаёт среду с seeded ролями/политиками и пользователем с заданной ролью.
async fn seed_env(
    db: &Surreal<Any>,
    company_id: Uuid,
    user_id: Uuid,
    role_codes: &[&str],
) -> (SurrealRoleRepository, Uuid) {
    let store = SurrealEventStore::new(db.clone());
    store.ensure_schema().await.unwrap();
    let role_repo = SurrealRoleRepository::new(db.clone());
    role_repo.ensure_schema().await.unwrap();
    let policy_repo = SurrealPermissionPolicyRepository::new(db.clone());
    policy_repo.ensure_schema().await.unwrap();
    let audit = SurrealAuditRepository::new(db.clone());
    audit.ensure_schema().await.unwrap();

    seed_system_roles_and_policies(&company_id, &role_repo, &policy_repo, &audit)
        .await
        .unwrap();

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

    (role_repo, user_id)
}

async fn pipeline_env(
    db: &Surreal<Any>,
    company_id: Uuid,
    user_id: Uuid,
    role_codes: &[&str],
) -> (CommandRegistry, Arc<SurrealAuditRepository>) {
    let (_, _) = seed_env(db, company_id, user_id, role_codes).await;

    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    let role_repo = Arc::new(SurrealRoleRepository::new(db.clone()));
    let policy_repo = Arc::new(SurrealPermissionPolicyRepository::new(db.clone()));
    let permissions = Arc::new(PermissionManager::new(role_repo.clone(), policy_repo));

    let registry = CommandRegistry::new();
    registry
        .attach_pipeline(audit.clone(), permissions.clone())
        .await;
    registry
        .register_with_metadata(
            "invoice.issue",
            CommandMetadata::requires("create"),
            |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({"issued": true})) },
        )
        .await;
    registry
        .register_with_metadata(
            "invoice.read",
            CommandMetadata::requires("read"),
            |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({"invoice_id": "i-1"})) },
        )
        .await;
    registry
        .register_with_metadata(
            "company.remove",
            CommandMetadata::requires("admin.purge"),
            |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({"removed": true})) },
        )
        .await;
    registry
        .register_with_metadata(
            "role.list",
            CommandMetadata::requires("role.manage"),
            |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({"roles": []})) },
        )
        .await;
    registry
        .register_with_metadata(
            "audit.query",
            CommandMetadata::requires("audit.read"),
            |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({"entries": []})) },
        )
        .await;

    (registry, audit)
}

fn err_contains(err: &DomainError, needle: &str) -> bool {
    err.to_string().contains(needle)
}

#[tokio::test]
async fn seed_is_idempotent_and_creates_system_roles_and_policies() {
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
    seed_system_roles_and_policies(&company_id, &role_repo, &policy_repo, &audit)
        .await
        .unwrap();

    let roles = role_repo.list().await.unwrap();
    assert_eq!(roles.len(), 4, "seed не создаёт дублей");
    let codes: HashSet<String> = roles.iter().map(|r| r.code.clone()).collect();
    let expected: HashSet<String> = ["admin", "staff", "guest", "archived"]
        .iter()
        .map(|c| c.to_string())
        .collect();
    assert_eq!(codes, expected);

    let admin = role_repo.get_by_code(&company_id, "admin").await.unwrap();
    assert!(admin.is_system);
    assert_eq!(
        admin.permission_policy_codes,
        vec!["platform.full".to_string()]
    );
    let guest = role_repo.get_by_code(&company_id, "guest").await.unwrap();
    assert_eq!(
        guest.permission_policy_codes,
        vec!["guest.objects".to_string(), "platform.modules".to_string()]
    );

    let admin_policy = policy_repo.get_by_code("platform.full").await.unwrap();
    assert!(admin_policy.is_system);
    assert_eq!(admin_policy.actions, vec!["*".to_string()]);
    assert_eq!(admin_policy.priority, 100);
}

#[tokio::test]
async fn admin_has_full_access_through_pipeline() {
    let db = mem_db().await;
    let company_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let (registry, audit) = pipeline_env(&db, company_id, user_id, &["admin"]).await;

    let ctx = CommandExecutionCtx {
        actor: Some(actor(user_id, company_id)),
        module_code: Some("invoice".to_string()),
        entity_type: Some("invoice".to_string()),
    };

    let issued = registry
        .execute_ctx("invoice.issue", json!({}), ctx.clone())
        .await
        .unwrap();
    assert_eq!(issued, json!({"issued": true}));

    let removed = registry
        .execute_ctx("company.remove", json!({}), ctx.clone())
        .await
        .unwrap();
    assert_eq!(removed, json!({"removed": true}));

    let log = audit
        .query(AuditFilter {
            action: Some("command.executed".to_string()),
            company_id: Some(company_id),
            result_success: Some(true),
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(log.len(), 4, "2 команды × (started + finished)");
    assert!(log.iter().all(|e| e.action == "command.executed"));
}

#[tokio::test]
async fn staff_can_work_with_objects_but_not_admin_actions() {
    let db = mem_db().await;
    let company_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let (registry, audit) = pipeline_env(&db, company_id, user_id, &["staff"]).await;

    let ctx = CommandExecutionCtx {
        actor: Some(actor(user_id, company_id)),
        module_code: Some("invoice".to_string()),
        entity_type: Some("invoice".to_string()),
    };

    let issued = registry
        .execute_ctx("invoice.issue", json!({}), ctx.clone())
        .await
        .unwrap();
    assert_eq!(issued, json!({"issued": true}));

    let denied = registry
        .execute_ctx("company.remove", json!({}), ctx.clone())
        .await;
    let err = denied.unwrap_err();
    assert!(
        err_contains(&err, "недостаточно прав"),
        "ошибка PermissionDenied: {err}"
    );

    let denied_log = audit
        .query(AuditFilter {
            action: Some("permission.denied".to_string()),
            company_id: Some(company_id),
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(denied_log.len(), 1);
    assert!(matches!(
        denied_log[0].result,
        core_domain::audit::AuditResult::Failure { .. }
    ));

    let finished_log = audit
        .query(AuditFilter {
            action: Some("command.executed".to_string()),
            company_id: Some(company_id),
            result_success: Some(true),
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert!(
        finished_log
        .iter()
        .all(|e| {
            e.details.as_ref().is_some_and(|d| {
                d.get("stage").and_then(|s| s.as_str()) != Some("finished")
                    || d.get("command").and_then(|c| c.as_str()) != Some("company.remove")
            })
        }),
        "для company.remove нет финишного аудита (хендлер не вызывался)"
    );
}

#[tokio::test]
async fn guest_reads_only_and_writes_are_denied() {
    let db = mem_db().await;
    let company_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let (registry, audit) = pipeline_env(&db, company_id, user_id, &["guest"]).await;

    let ctx = CommandExecutionCtx {
        actor: Some(actor(user_id, company_id)),
        module_code: Some("invoice".to_string()),
        entity_type: Some("invoice".to_string()),
    };

    let read = registry
        .execute_ctx("invoice.read", json!({}), ctx.clone())
        .await
        .unwrap();
    assert_eq!(read, json!({"invoice_id": "i-1"}));

    let denied = registry
        .execute_ctx("invoice.issue", json!({}), ctx.clone())
        .await;
    let err = denied.unwrap_err();
    assert!(
        err_contains(&err, "недостаточно прав"),
        "ошибка PermissionDenied: {err}"
    );

    let denied_log = audit
        .query(AuditFilter {
            action: Some("permission.denied".to_string()),
            company_id: Some(company_id),
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(denied_log.len(), 1);
    let ec = denied_log[0]
        .target
        .as_ref()
        .unwrap()
        .entity_code
        .clone()
        .unwrap();
    assert_eq!(ec, "invoice.issue");
}

#[tokio::test]
async fn system_actor_bypasses_permission_check() {
    let db = mem_db().await;
    let store = SurrealEventStore::new(db.clone());
    store.ensure_schema().await.unwrap();
    let role_repo = SurrealRoleRepository::new(db.clone());
    role_repo.ensure_schema().await.unwrap();
    let policy_repo = SurrealPermissionPolicyRepository::new(db.clone());
    policy_repo.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();
    seed_system_roles_and_policies(
        &Uuid::new_v4(),
        &role_repo,
        &policy_repo,
        audit.as_ref(),
    )
    .await
    .unwrap();

    let permissions = Arc::new(PermissionManager::new(
        Arc::new(role_repo),
        Arc::new(policy_repo),
    ));
    let registry = CommandRegistry::new();
    registry.attach_pipeline(audit.clone(), permissions).await;
    registry
        .register_with_metadata(
            "system.only",
            CommandMetadata::requires("admin.purge"),
            |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({"system": true})) },
        )
        .await;

    let value = registry
        .execute_ctx("system.only", json!({}), CommandExecutionCtx::default())
        .await
        .unwrap();
    assert_eq!(value, json!({"system": true}));
}

#[test]
fn permission_denied_error_code_is_serializable() {
    let err = DomainError::PermissionDenied("x".to_string());
    assert_eq!(err.code(), "PERMISSION_ERROR");
}

#[tokio::test]
async fn staff_cannot_manage_system_entities() {
    let db = mem_db().await;
    let company_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let (registry, _) = pipeline_env(&db, company_id, user_id, &["staff"]).await;

    let ctx = CommandExecutionCtx {
        actor: Some(actor(user_id, company_id)),
        module_code: Some("roles".to_string()),
        entity_type: Some("roles".to_string()),
    };

    for command in ["role.list", "audit.query"] {
        let err = registry
            .execute_ctx(command, json!({}), ctx.clone())
            .await
            .unwrap_err();
        assert!(
            err_contains(&err, "недостаточно прав"),
            "staff должен получить отказ на {command}: {err}"
        );
    }
}

#[tokio::test]
async fn guest_has_no_access_to_audit() {
    let db = mem_db().await;
    let company_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let (registry, _) = pipeline_env(&db, company_id, user_id, &["guest"]).await;

    let ctx = CommandExecutionCtx {
        actor: Some(actor(user_id, company_id)),
        module_code: None,
        entity_type: None,
    };

    let err = registry
        .execute_ctx("audit.query", json!({}), ctx)
        .await
        .unwrap_err();
    assert!(
        err_contains(&err, "недостаточно прав"),
        "guest не должен читать аудит: {err}"
    );
}

/// role.seed: система и администратор проходят, сотрудник получает отказ.
#[tokio::test]
async fn role_seed_requires_role_manage() {
    let db = mem_db().await;
    let store = SurrealEventStore::new(db.clone());
    store.ensure_schema().await.unwrap();
    let role_repo = Arc::new(SurrealRoleRepository::new(db.clone()));
    role_repo.ensure_schema().await.unwrap();
    let policy_repo = Arc::new(SurrealPermissionPolicyRepository::new(db.clone()));
    policy_repo.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();
    let permissions = Arc::new(PermissionManager::new(role_repo.clone(), policy_repo.clone()));
    let registry = CommandRegistry::new();
    registry.attach_pipeline(audit.clone(), permissions).await;

    let seed_handler = {
        let role_repo = role_repo.clone();
        let policy_repo = policy_repo.clone();
        let audit = audit.clone();
        move |params: Value, _ctx: CommandExecutionCtx| {
            let role_repo = role_repo.clone();
            let policy_repo = policy_repo.clone();
            let audit = audit.clone();
            async move {
                let company_id: Uuid = serde_json::from_value(
                    params.get("company_id").cloned().unwrap_or_default(),
                )
                .map_err(|e: serde_json::Error| DomainError::ValidationError(e.to_string()))?;
                seed_system_roles_and_policies(
                    &company_id,
                    role_repo.as_ref(),
                    policy_repo.as_ref(),
                    audit.as_ref(),
                )
                .await?;
                Ok(json!({"seeded": true}))
            }
        }
    };
    registry
        .register_with_metadata("role.seed", CommandMetadata::requires("role.manage"), seed_handler)
        .await;

    let company_id = Uuid::new_v4();
    // Система: первый сид (у компании ещё нет ролей).
    registry
        .execute_ctx("role.seed", json!({"company_id": company_id}), CommandExecutionCtx::default())
        .await
        .unwrap();
    assert!(role_repo.get_by_code(&company_id, "admin").await.is_ok());
    // Повторный вызов идемпотентен: дублей нет.
    registry
        .execute_ctx("role.seed", json!({"company_id": company_id}), CommandExecutionCtx::default())
        .await
        .unwrap();
    assert_eq!(role_repo.list().await.unwrap().len(), 4);

    // Администратор (platform.full закрывает role.manage) проходит.
    let admin_id = Uuid::new_v4();
    let (_, _) = seed_env(&db, company_id, admin_id, &["admin"]).await;
    let admin_ctx = CommandExecutionCtx {
        actor: Some(actor(admin_id, company_id)),
        module_code: Some("roles".to_string()),
        entity_type: Some("role".to_string()),
    };
    registry
        .execute_ctx(
            "role.seed",
            json!({"company_id": company_id}),
            admin_ctx.clone(),
        )
        .await
        .unwrap();

    // Сотрудник получает отказ.
    let staff_id = Uuid::new_v4();
    let (_, _) = seed_env(&db, company_id, staff_id, &["staff"]).await;
    let staff_ctx = CommandExecutionCtx {
        actor: Some(actor(staff_id, company_id)),
        module_code: Some("roles".to_string()),
        entity_type: Some("role".to_string()),
    };
    let err = registry
        .execute_ctx("role.seed", json!({"company_id": company_id}), staff_ctx)
        .await
        .unwrap_err();
    assert!(
        err_contains(&err, "недостаточно прав"),
        "staff не должен сидить роли: {err}"
    );
}

/// system.migrate_permissions лечит компании без системных ролей и идемпотентен.
#[tokio::test]
async fn migrate_permissions_seeds_companies_without_roles() {
    let db = mem_db().await;
    let store = SurrealEventStore::new(db.clone());
    store.ensure_schema().await.unwrap();
    let companies = Arc::new(SurrealCompanyRepository::new(db.clone()));
    companies.ensure_schema().await.unwrap();
    let role_repo = Arc::new(SurrealRoleRepository::new(db.clone()));
    role_repo.ensure_schema().await.unwrap();
    let policy_repo = Arc::new(SurrealPermissionPolicyRepository::new(db.clone()));
    policy_repo.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();

    let old_company = Uuid::new_v4();
    companies
        .create(
            &core_domain::company::Company {
                id: old_company,
                code: "old".to_string(),
                name: "Старая компания".to_string(),
                is_active: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            &[Event {
                id: Uuid::new_v4(),
                stream_type: StreamType::Company,
                stream_id: old_company.to_string(),
                event_type: "company.created".to_string(),
                version: 0,
                payload: json!({}),
                metadata: ActorSnapshot::system(),
                company_id: old_company.to_string(),
                correlation_id: Uuid::new_v4().to_string(),
                causation_id: None,
                occurred_at: chrono::Utc::now(),
            }],
        )
        .await
        .unwrap();
    // Компания без ролей — именно её должен «вылечить» мигратор.
    assert!(role_repo.get_by_code(&old_company, "admin").await.is_err());

    let permissions = Arc::new(PermissionManager::new(role_repo.clone(), policy_repo.clone()));
    let registry = CommandRegistry::new();
    registry.attach_pipeline(audit.clone(), permissions).await;

    let migrate_handler = {
        let companies = companies.clone();
        let role_repo = role_repo.clone();
        let policy_repo = policy_repo.clone();
        let audit = audit.clone();
        move |_params: Value, _ctx: CommandExecutionCtx| {
            let companies = companies.clone();
            let role_repo = role_repo.clone();
            let policy_repo = policy_repo.clone();
            let audit = audit.clone();
            async move {
                let list = companies.list().await?;
                let mut seeded = 0usize;
                for company in &list {
                    if role_repo.get_by_code(&company.id, "admin").await.is_err() {
                        seed_system_roles_and_policies(
                            &company.id,
                            role_repo.as_ref(),
                            policy_repo.as_ref(),
                            audit.as_ref(),
                        )
                        .await?;
                        seeded += 1;
                    }
                }
                Ok(json!({"companies_total": list.len(), "seeded": seeded}))
            }
        }
    };
    registry
        .register_with_metadata(
            "system.migrate_permissions",
            CommandMetadata::requires("role.manage"),
            migrate_handler,
        )
        .await;

    // Системный исполнитель запускает миграцию.
    let result = registry
        .execute_ctx(
            "system.migrate_permissions",
            json!({}),
            CommandExecutionCtx::default(),
        )
        .await
        .unwrap();
    assert_eq!(result, json!({"companies_total": 1, "seeded": 1}));
    assert!(role_repo.get_by_code(&old_company, "admin").await.is_ok());

    // Повторный прогон идемпотентен.
    let second = registry
        .execute_ctx(
            "system.migrate_permissions",
            json!({}),
            CommandExecutionCtx::default(),
        )
        .await
        .unwrap();
    assert_eq!(second, json!({"companies_total": 1, "seeded": 0}));
    assert_eq!(role_repo.list().await.unwrap().len(), 4);
}