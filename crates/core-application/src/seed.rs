//! Идемпотентный сидинг системных ролей и политик доступа при инициализации компании.

use crate::ports::{AuditRepository, PermissionPolicyRepository, RoleRepository};
use core_domain::audit::{AuditEntry, AuditResult, AuditTarget};
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::permission::{PermissionPolicy, PermissionScopeType, RecordAccessLevel};
use core_domain::role::Role;
use uuid::Uuid;

/// Описание системной политики: код, имя, действия, уровень доступа к записям, приоритет.
const SYSTEM_POLICIES: &[(&str, &str, &[&str], RecordAccessLevel, i32)] = &[
    ("platform.full", "Полный доступ к платформе", &["*"], RecordAccessLevel::All, 100),
    (
        "staff.objects",
        "Работа с объектами (сотрудник)",
        &["create", "read", "update"],
        RecordAccessLevel::ByCompany,
        50,
    ),
    (
        "guest.objects",
        "Чтение объектов (гость)",
        &["read"],
        RecordAccessLevel::ByCompany,
        40,
    ),
    (
        "archived.objects",
        "Чтение своих объектов (архив)",
        &["read"],
        RecordAccessLevel::Owned,
        40,
    ),
    (
        "platform.scripts",
        "Управление скриптами",
        &["script.manage", "script.execute", "script.read"],
        RecordAccessLevel::All,
        90,
    ),
];

/// Описание системной роли: код, имя, ссылки на политики.
const SYSTEM_ROLES: &[(&str, &str, &[&str])] = &[
    ("admin", "Администратор", &["platform.full"]),
    ("staff", "Сотрудник", &["staff.objects"]),
    ("guest", "Гость", &["guest.objects"]),
    ("archived", "Архивированный сотрудник", &["archived.objects"]),
];

fn system_policy(code: &str, name: &str, actions: &[&str], record_access: RecordAccessLevel, priority: i32) -> PermissionPolicy {
    let now = chrono::Utc::now();
    PermissionPolicy {
        id: Uuid::new_v4(),
        code: code.to_string(),
        name: name.to_string(),
        description: Some("Системная политика доступа".to_string()),
        scope_type: PermissionScopeType::Platform,
        entity_type: None,
        actions: actions.iter().map(|a| a.to_string()).collect(),
        record_access,
        deny: false,
        priority,
        module_code: None,
        is_system: true,
        created_at: now,
        updated_at: now,
    }
}

fn role_created_event(role: &Role) -> Event {
    Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::Role,
        stream_id: role.id.to_string(),
        event_type: "role.created".to_string(),
        version: 0,
        payload: serde_json::to_value(role).unwrap_or_default(),
        metadata: ActorSnapshot::system(),
        company_id: role.company_id.to_string(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    }
}

/// Создаёт системные роли (`admin`, `staff`, `guest`, `archived`) и базовые
/// политики доступа для компании с ensure-семантикой.
///
/// Политики глобальны (идентифицируются по коду) и создаются идемпотентно;
/// роли принадлежат конкретной `company_id` и создаются только если ещё
/// отсутствуют. По завершении выполняется служебная запись в аудит.
pub async fn seed_system_roles_and_policies<R, P, A>(
    company_id: &Uuid,
    role_repo: &R,
    policy_repo: &P,
    audit_repo: &A,
) -> Result<(), DomainError>
where
    R: RoleRepository + ?Sized,
    P: PermissionPolicyRepository + ?Sized,
    A: AuditRepository + ?Sized,
{
    for (code, name, actions, access, priority) in SYSTEM_POLICIES {
        policy_repo
            .upsert(&system_policy(code, name, actions, access.clone(), *priority))
            .await?;
    }

    for (code, name, policy_codes) in SYSTEM_ROLES {
        if role_repo.get_by_code(company_id, code).await.is_ok() {
            continue;
        }
        let now = chrono::Utc::now();
        let role = Role {
            id: Uuid::new_v4(),
            company_id: *company_id,
            code: code.to_string(),
            name: (*name).to_string(),
            description: "Системная роль".to_string(),
            permission_policy_codes: policy_codes.iter().map(|c| c.to_string()).collect(),
            is_system: true,
            created_at: now,
            updated_at: now,
        };
        let event = role_created_event(&role);
        role_repo.create(&role, &[event]).await?;
    }

    let entry = AuditEntry {
        id: Uuid::new_v4(),
        action: "role.seed.company".to_string(),
        actor: ActorSnapshot::system(),
        target: Some(AuditTarget {
            entity_type: Some("company".to_string()),
            entity_id: Some(*company_id),
            entity_code: None,
            company_id: Some(*company_id),
        }),
        result: AuditResult::Success,
        details: Some(serde_json::json!({
            "roles": SYSTEM_ROLES.iter().map(|(code, _, _)| *code).collect::<Vec<_>>(),
        })),
        ip_address: None,
        user_agent: None,
        company_id: Some(*company_id),
        timestamp: chrono::Utc::now(),
    };
    audit_repo.log(entry).await?;
    Ok(())
}