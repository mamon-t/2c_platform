//! Команда `system.bootstrap`: однократная инициализация платформы.
//!
//! Создаёт первую компанию, суперадминистратора и системные роли/политики.
//! Бизнес-логика вынесена сюда, чтобы интеграционные тесты могли вызывать её
//! без бинарника platform-server. Атомарность не гарантируется (каждый шаг —
//! отдельная операция репозитория): повторный вызов безопасен из-за
//! идемпотентности `seed_system_roles_and_policies` и guard'а на существование
//! компании.

use core_domain::audit::{AuditEntry, AuditResult, AuditTarget};
use core_domain::company::Company;
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::password::hash_password;
use core_domain::user::{Person, User, UserCompanyProfile, UserStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::metadata_seed::seed_system_metadata;
use crate::ports::{
    AuditRepository, CompanyRepository, MetadataRepository, PermissionPolicyRepository,
    RoleRepository, UserRepository,
};
use crate::seed::{seed_system_roles_and_policies, system_policies_count, system_roles_count};

/// Параметры команды `system.bootstrap`.
#[derive(Debug, Clone, Deserialize)]
pub struct BootstrapParams {
    pub company_code: String,
    pub company_name: String,
    pub admin_login: String,
    pub admin_password: String,
    pub admin_last_name: Option<String>,
    pub admin_first_name: Option<String>,
}

/// Итог успешного бутстрапа платформы (ответ команды).
#[derive(Debug, Clone, Serialize)]
pub struct BootstrapResult {
    pub company_id: Uuid,
    pub company_code: String,
    pub admin_user_id: Uuid,
    pub admin_login: String,
    pub roles_seeded: usize,
    pub policies_seeded: usize,
    pub metadata_entity_types_seeded: usize,
}

fn system_event(
    stream_type: StreamType,
    stream_id: &str,
    event_type: &str,
    company_id: &str,
    payload: Value,
) -> Event {
    Event {
        id: Uuid::new_v4(),
        stream_type,
        stream_id: stream_id.to_string(),
        event_type: event_type.to_string(),
        version: 0,
        payload,
        metadata: ActorSnapshot::system(),
        company_id: company_id.to_string(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    }
}

/// Выполняет полный цикл инициализации платформы: компания → суперадмин →
/// системные роли/политики → привязка админа к роли `admin` → аудит.
///
/// Идемпотентность: если компания с нужным `company_code` уже существует —
/// `ValidationError` «Компания {code} уже существует»; если платформа содержит
/// любые другие компании — `ValidationError` «Платформа уже инициализирована».
///
/// # Errors
///
/// Возвращает `DomainError::ValidationError` при нарушениях идемпотентности и
/// неверном пароле, `DomainError::Storage` при сбоях хранилища.
pub async fn bootstrap_platform<C, U, R, P, A, M>(
    params: BootstrapParams,
    companies: &C,
    users: &U,
    roles: &R,
    policies: &P,
    audit: &A,
    metadata: &M,
) -> Result<BootstrapResult, DomainError>
where
    C: CompanyRepository + ?Sized,
    U: UserRepository + ?Sized,
    R: RoleRepository + ?Sized,
    P: PermissionPolicyRepository + ?Sized,
    A: AuditRepository + ?Sized,
    M: MetadataRepository + ?Sized,
{
    let existing = companies.list().await?;
    if existing.iter().any(|c| c.code == params.company_code) {
        return Err(DomainError::ValidationError(format!(
            "Компания {} уже существует",
            params.company_code
        )));
    }
    if !existing.is_empty() {
        return Err(DomainError::ValidationError(
            "Платформа уже инициализирована. Для добавления компаний используйте company.create"
                .to_string(),
        ));
    }

    let now = chrono::Utc::now();
    let company_id = Uuid::new_v4();
    let company = Company {
        id: company_id,
        code: params.company_code.clone(),
        name: params.company_name.clone(),
        is_active: true,
        created_at: now,
        updated_at: now,
    };
    let company_payload = serde_json::to_value(&company)
        .map_err(|e| DomainError::Storage(format!("компания encode: {e}")))?;
    companies
        .create(
            &company,
            &[system_event(
                StreamType::Company,
                &company_id.to_string(),
                "company.created",
                &company_id.to_string(),
                company_payload,
            )],
        )
        .await?;

    let password_hash = hash_password(&params.admin_password)
        .map_err(|e| DomainError::ValidationError(format!("ошибка хеширования пароля: {e}")))?;
    let user_id = Uuid::new_v4();
    let last_name = params
        .admin_last_name
        .clone()
        .unwrap_or_else(|| "Администратор".to_string());
    let first_name = params
        .admin_first_name
        .clone()
        .unwrap_or_else(|| "Системы".to_string());
    let person = Person {
        id: user_id,
        user_id,
        last_name: last_name.clone(),
        first_name: first_name.clone(),
        middle_name: None,
        display_name: format!("{last_name} {first_name}"),
    };
    let user = User {
        id: user_id,
        login: params.admin_login.clone(),
        password_hash,
        status: UserStatus::Active,
        role_ids: vec![],
        failed_login_count: 0,
        locked_until: None,
        must_change_password: true,
        locale: "ru-RU".to_string(),
        timezone: "Europe/Moscow".to_string(),
        person_id: Some(user_id),
        created_at: now,
        updated_at: now,
    };
    let user_payload = serde_json::to_value(&user)
        .map_err(|e| DomainError::Storage(format!("пользователь encode: {e}")))?;
    let person_payload = serde_json::to_value(&person)
        .map_err(|e| DomainError::Storage(format!("персона encode: {e}")))?;
    users
        .create(
            &user,
            &person,
            &[
                system_event(
                    StreamType::User,
                    &user_id.to_string(),
                    "user.created",
                    &company_id.to_string(),
                    user_payload,
                ),
                system_event(
                    StreamType::Person,
                    &user_id.to_string(),
                    "person.created",
                    &company_id.to_string(),
                    person_payload,
                ),
            ],
        )
        .await?;

    let summary = seed_system_roles_and_policies(&company_id, roles, policies, audit).await?;
    let metadata_seeded = seed_system_metadata(metadata).await?;

    let admin_role = roles.get_by_code(&company_id, "admin").await?;
    let mut bound = user;
    bound.role_ids = vec![admin_role.id.to_string()];
    let bound_payload = serde_json::to_value(&bound)
        .map_err(|e| DomainError::Storage(format!("привязка encode: {e}")))?;
    users
        .update(
            &bound,
            &[system_event(
                StreamType::User,
                &user_id.to_string(),
                "user.updated",
                &company_id.to_string(),
                bound_payload,
            )],
        )
        .await?;

    let profile = UserCompanyProfile {
        id: Uuid::new_v4(),
        user_id,
        company_id,
        employee_number: None,
        position: Some("Администратор".to_string()),
        department: None,
        is_primary: true,
        is_active: true,
        valid_from: None,
        valid_to: None,
    };
    let profile_payload = serde_json::to_value(&profile)
        .map_err(|e| DomainError::Storage(format!("профиль encode: {e}")))?;
    users
        .add_profile(
            &profile,
            &[system_event(
                StreamType::UserProfile,
                &profile.id.to_string(),
                "user_profile.created",
                &company_id.to_string(),
                profile_payload,
            )],
        )
        .await?;

    let entry = AuditEntry {
        id: Uuid::new_v4(),
        action: "system.bootstrap".to_string(),
        actor: ActorSnapshot::system(),
        target: Some(AuditTarget {
            entity_type: Some("company".to_string()),
            entity_id: Some(company_id),
            entity_code: Some(params.company_code.clone()),
            company_id: Some(company_id),
        }),
        result: AuditResult::Success,
        details: Some(json!({
            "company_code": params.company_code,
            "admin_login": params.admin_login,
            "roles_created": summary.roles_created,
            "policies_seeded": system_policies_count(),
            "metadata_entity_types_seeded": metadata_seeded,
        })),
        ip_address: None,
        user_agent: None,
        company_id: Some(company_id),
        timestamp: chrono::Utc::now(),
    };
    audit.log(entry).await?;

    Ok(BootstrapResult {
        company_id,
        company_code: params.company_code,
        admin_user_id: user_id,
        admin_login: params.admin_login,
        roles_seeded: system_roles_count(),
        policies_seeded: system_policies_count(),
        metadata_entity_types_seeded: metadata_seeded,
    })
}