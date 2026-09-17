//! Командный слой для Фазы 2: компании, пользователи, роли.
//!
//! Каждая изменяющая команда атомарно продвигает и Трубу (события), и Доску
//! (материализованную коллекцию) через репозиторий, записывая снимок аудита
//! от системного исполнителя, пока не появится аутентификация.

use chrono::{DateTime, NaiveDate, Utc};
use core_application::auth::AuthService;
use core_application::bootstrap::BootstrapParams;
use core_application::command_registry::{CommandExecutionCtx, CommandMetadata};
use core_application::permission_manager::PermissionManager;
use core_application::ports::{
    AuditRepository, CompanyRepository, EntitySchema, MetadataRepository, ObjectRepository,
    RoleRepository, ScriptEngine, ScriptRepository, UserRepository,
};
use core_application::script_runner::{execute_script, test_script, validate_script};
use core_application::seed::seed_system_roles_and_policies;
use core_application::CommandRegistry;
use core_application::ModuleManager;
use core_domain::audit::{AuditEntry, AuditFilter, AuditResult, AuditTarget};
use core_domain::company::Company;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::error::DomainError;
use core_domain::metadata::{
    EntityAction, EntityField, EntityForm, EntityKind, EntityRelation, EntityState, EntityTransition,
    EntityType, FieldType, OnDelete, RelationKind,
};
use core_domain::object::{Object, ObjectKind};
use core_domain::password::hash_password;
use core_domain::role::Role;
use core_domain::script::{Script, ScriptType};
use core_domain::user::{
    ContactChannelType, ContactPurpose, Person, User, UserCompanyProfile, UserContact, UserStatus,
};
use core_infrastructure::{
    SurrealAuditRepository, SurrealCompanyRepository, SurrealMetadataRepository,
    SurrealObjectRepository, SurrealPermissionPolicyRepository, SurrealRoleRepository,
    SurrealScriptRepository, SurrealUserRepository,
};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

fn system_event(
    stream_type: StreamType,
    stream_id: String,
    event_type: &str,
    company_id: &str,
    payload: Value,
) -> Event {
    Event {
        id: Uuid::new_v4(),
        stream_type,
        stream_id,
        event_type: event_type.to_string(),
        version: 0,
        payload,
        metadata: ActorSnapshot::system(),
        company_id: company_id.to_string(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: Utc::now(),
    }
}

fn encode<T: serde::Serialize>(value: &T) -> Result<Value, DomainError> {
    serde_json::to_value(value).map_err(|e| DomainError::ValidationError(format!("сериализация: {e}")))
}

fn require(value: &Value, key: &str) -> Result<String, DomainError> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| DomainError::ValidationError(format!("отсутствует обязательный параметр '{key}'")))
}

fn optional(value: &Value, key: &str) -> Result<Option<String>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| DomainError::ValidationError(format!("параметр '{key}' должен быть строкой"))),
    }
}

fn optional_bool(value: &Value, key: &str) -> Result<Option<bool>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_bool()
            .map(Some)
            .ok_or_else(|| DomainError::ValidationError(format!("параметр '{key}' должен быть булевым"))),
    }
}

fn parse_enum<T: serde::de::DeserializeOwned>(value: &Value, key: &str) -> Result<T, DomainError> {
    let raw = value
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| DomainError::ValidationError(format!("отсутствует обязательный параметр '{key}'")))?;
    serde_json::from_value(Value::String(raw.to_string()))
        .map_err(|e| DomainError::ValidationError(format!("некорректное значение '{key}': {e}")))
}

fn parse_uuids(value: &Value, key: &str) -> Result<Vec<String>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|i| {
                i.as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| DomainError::ValidationError(format!("параметр '{key}' должен содержать строки")))
            })
            .collect(),
        Some(_) => Err(DomainError::ValidationError(format!("параметр '{key}' должен быть массивом"))),
    }
}

fn parse_purposes(value: &Value) -> Result<Vec<ContactPurpose>, DomainError> {
    parse_uuids(value, "purposes").and_then(|names| name_list_to_purposes(&names))
}

fn name_list_to_purposes(names: &[String]) -> Result<Vec<ContactPurpose>, DomainError> {
    names
        .iter()
        .map(|name| {
            serde_json::from_value(Value::String(name.clone()))
                .map_err(|e| DomainError::ValidationError(format!("некорректное назначение контакта: {e}")))
        })
        .collect()
}

async fn register_company_commands(
    registry: &CommandRegistry,
    companies: Arc<SurrealCompanyRepository>,
) {
    registry
        .register_with_metadata("company.create", CommandMetadata::requires("create"), {
            let companies = companies.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let companies = companies.clone();
                async move {
                    let code = require(&params, "code")?;
                    let name = require(&params, "name")?;
                    let is_active = optional_bool(&params, "is_active")?.unwrap_or(true);
                    let now = Utc::now();
                    let company = Company {
                        id: Uuid::new_v4(),
                        code,
                        name,
                        is_active,
                        created_at: now,
                        updated_at: now,
                    };
                    let payload = encode(&company)?;
                    let event = system_event(
                        StreamType::Company,
                        company.id.to_string(),
                        "company.created",
                        &company.id.to_string(),
                        payload,
                    );
                    companies
                        .create(&company, &[event])
                        .await?;
                    encode(&company)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("company.get", CommandMetadata::requires("read"), {
            let companies = companies.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let companies = companies.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let company = companies.get(&id).await?;
                    encode(&company)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("company.list", CommandMetadata::requires("read"), {
            let companies = companies.clone();
            move |_params: Value, _ctx: CommandExecutionCtx| {
                let companies = companies.clone();
                async move {
                    let list = companies.list().await?;
                    let rows: Result<Vec<Value>, DomainError> =
                        list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("company.update", CommandMetadata::requires("update"), {
            let companies = companies.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let companies = companies.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let existing = companies.get(&id).await?;
                    let company = Company {
                        id,
                        code: optional(&params, "code")?.unwrap_or(existing.code),
                        name: optional(&params, "name")?.unwrap_or(existing.name),
                        is_active: optional_bool(&params, "is_active")?.unwrap_or(existing.is_active),
                        created_at: existing.created_at,
                        updated_at: Utc::now(),
                    };
                    let payload = encode(&company)?;
                    let event = system_event(
                        StreamType::Company,
                        company.id.to_string(),
                        "company.updated",
                        &company.id.to_string(),
                        payload,
                    );
                    companies
                        .update(&company, &[event])
                        .await?;
                    encode(&company)
                }
            }
        })
        .await;
}

async fn register_user_commands(registry: &CommandRegistry, users: Arc<SurrealUserRepository>) {
    registry
        .register_with_metadata("user.create", CommandMetadata::requires("create"), {
            let users = users.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let users = users.clone();
                async move {
                    let login = require(&params, "login")?;
                    let password = require(&params, "password")?;
                    let password_hash =
                        hash_password(&password).map_err(DomainError::ValidationError)?;
                    let last_name = require(&params, "last_name")?;
                    let first_name = require(&params, "first_name")?;
                    let middle_name = optional(&params, "middle_name")?;
                    let status = parse_optional_enum::<UserStatus>(&params, "status")?
                        .unwrap_or(UserStatus::Active);
                    let locale = optional(&params, "locale")?.unwrap_or_else(|| "ru-RU".to_string());
                    let timezone =
                        optional(&params, "timezone")?.unwrap_or_else(|| "Europe/Moscow".to_string());
                    let role_ids = parse_uuids(&params, "role_ids")?;
                    let display_name = optional(&params, "display_name")?.unwrap_or_else(|| {
                        format!(
                            "{last_name} {first_name}{}",
                            middle_name
                                .as_ref()
                                .map(|m| format!(" {m}"))
                                .unwrap_or_default()
                        )
                    });

                    let now = Utc::now();
                    let user_id = Uuid::new_v4();
                    let user = User {
                        id: user_id,
                        login,
                        password_hash,
                        status,
                        role_ids,
                        failed_login_count: 0,
                        locked_until: None,
                        must_change_password: true,
                        locale,
                        timezone,
                        person_id: Some(user_id),
                        created_at: now,
                        updated_at: now,
                    };
                    let person = Person {
                        id: user_id,
                        user_id,
                        last_name,
                        first_name,
                        middle_name,
                        display_name,
                    };
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let events = vec![
                        system_event(
                            StreamType::User,
                            user.id.to_string(),
                            "user.created",
                            &company_id,
                            encode(&user)?,
                        ),
                        system_event(
                            StreamType::Person,
                            person.id.to_string(),
                            "person.created",
                            &company_id,
                            encode(&person)?,
                        ),
                    ];
                    users
                        .create(&user, &person, &events)
                        .await?;
                    Ok(json!({
                        "user": encode(&user)?,
                        "person": encode(&person)?,
                    }))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("user.get", CommandMetadata::requires("read"), {
            let users = users.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let users = users.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let user = users.get(&id).await?;
                    encode(&user)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("user.list", CommandMetadata::requires("read"), {
            let users = users.clone();
            move |_params: Value, _ctx: CommandExecutionCtx| {
                let users = users.clone();
                async move {
                    let list = users.list().await?;
                    let rows: Result<Vec<Value>, DomainError> = list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("user.update", CommandMetadata::requires("update"), {
            let users = users.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let users = users.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let existing = users.get(&id).await?;
                    let user = User {
                        id,
                        login: optional(&params, "login")?.unwrap_or(existing.login),
                        password_hash: optional(&params, "password")?
                            .map(|password| hash_password(&password).map_err(DomainError::ValidationError))
                            .transpose()?
                            .unwrap_or(existing.password_hash),
                        status: parse_optional_enum::<UserStatus>(&params, "status")?
                            .unwrap_or(existing.status),
                        role_ids: {
                            let raw = params.get("role_ids");
                            match raw {
                                None => existing.role_ids,
                                Some(_) => parse_uuids(&params, "role_ids")?,
                            }
                        },
                        failed_login_count: existing.failed_login_count,
                        locked_until: existing.locked_until,
                        must_change_password: optional_bool(&params, "must_change_password")?
                            .unwrap_or(existing.must_change_password),
                        locale: optional(&params, "locale")?.unwrap_or(existing.locale),
                        timezone: optional(&params, "timezone")?.unwrap_or(existing.timezone),
                        person_id: existing.person_id,
                        created_at: existing.created_at,
                        updated_at: Utc::now(),
                    };
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let event = system_event(
                        StreamType::User,
                        user.id.to_string(),
                        "user.updated",
                        &company_id,
                        encode(&user)?,
                    );
                    users.update(&user, &[event]).await?;
                    encode(&user)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("user.contact.add", CommandMetadata::requires("create"), {
            let users = users.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let users = users.clone();
                async move {
                    let user_id = parse_uuid(&params, "user_id")?;
                    let channel_type = parse_enum::<ContactChannelType>(&params, "channel_type")?;
                    let value = require(&params, "value")?;
                    let is_primary = optional_bool(&params, "is_primary")?.unwrap_or(false);
                    let is_verified = optional_bool(&params, "is_verified")?.unwrap_or(false);
                    let purposes = parse_purposes(&params)?;
                    let contact = UserContact {
                        id: Uuid::new_v4(),
                        user_id,
                        channel_type,
                        value,
                        is_primary,
                        is_verified,
                        purposes,
                    };
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let event = system_event(
                        StreamType::UserContact,
                        contact.id.to_string(),
                        "user_contact.created",
                        &company_id,
                        encode(&contact)?,
                    );
                    users
                        .add_contact(&contact, &[event])
                        .await?;
                    encode(&contact)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("user.profile.add", CommandMetadata::requires("create"), {
            let users = users.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let users = users.clone();
                async move {
                    let user_id = parse_uuid(&params, "user_id")?;
                    let company_id = parse_uuid(&params, "company_id")?;
                    let profile = UserCompanyProfile {
                        id: Uuid::new_v4(),
                        user_id,
                        company_id,
                        employee_number: optional(&params, "employee_number")?,
                        position: optional(&params, "position")?,
                        department: optional(&params, "department")?,
                        is_primary: optional_bool(&params, "is_primary")?.unwrap_or(true),
                        is_active: optional_bool(&params, "is_active")?.unwrap_or(true),
                        valid_from: parse_optional_date(&params, "valid_from")?,
                        valid_to: parse_optional_date(&params, "valid_to")?,
                    };
                    let event = system_event(
                        StreamType::UserProfile,
                        profile.id.to_string(),
                        "user_profile.created",
                        &company_id.to_string(),
                        encode(&profile)?,
                    );
                    users
                        .add_profile(&profile, &[event])
                        .await?;
                    encode(&profile)
                }
            }
        })
        .await;
}

async fn register_role_commands(registry: &CommandRegistry, roles: Arc<SurrealRoleRepository>) {
    registry
        .register_with_metadata("role.create", CommandMetadata::requires("role.manage"), {
            let roles = roles.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let roles = roles.clone();
                async move {
                    let code = require(&params, "code")?;
                    let name = require(&params, "name")?;
                    let description = optional(&params, "description")?.unwrap_or_default();
                    let is_system = optional_bool(&params, "is_system")?.unwrap_or(false);
                    let company_id = parse_uuid(&params, "company_id")?;
                    let permission_policy_codes = parse_uuids(&params, "permission_policy_codes")?;
                    let now = Utc::now();
                    let role = Role {
                        id: Uuid::new_v4(),
                        company_id,
                        code,
                        name,
                        description,
                        permission_policy_codes,
                        is_system,
                        created_at: now,
                        updated_at: now,
                    };
                    let event = system_event(
                        StreamType::Role,
                        role.id.to_string(),
                        "role.created",
                        &role.company_id.to_string(),
                        encode(&role)?,
                    );
                    roles.create(&role, &[event]).await?;
                    encode(&role)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("role.get", CommandMetadata::requires("role.manage"), {
            let roles = roles.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let roles = roles.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let role = roles.get(&id).await?;
                    encode(&role)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("role.list", CommandMetadata::requires("role.manage"), {
            let roles = roles.clone();
            move |_params: Value, _ctx: CommandExecutionCtx| {
                let roles = roles.clone();
                async move {
                    let list = roles.list().await?;
                    let rows: Result<Vec<Value>, DomainError> = list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;
}

async fn register_metadata_commands(
    registry: &CommandRegistry,
    metadata: Arc<SurrealMetadataRepository>,
) {
    registry
        .register_with_metadata("metadata.entity_type.create", CommandMetadata::requires("metadata.manage"), {
            let metadata = metadata.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let metadata = metadata.clone();
                async move {
                    let schema = parse_schema(&params)?;
                    let event = metadata_event(&schema, "metadata.entity_type.created");
                    metadata
                        .create_entity_type(&schema, &[event])
                        .await?;
                    encode(&schema)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("metadata.entity_type.get", CommandMetadata::requires("metadata.read"), {
            let metadata = metadata.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let metadata = metadata.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let entity_type = metadata
                        .get_entity_type(&id)
                        .await?;
                    encode(&entity_type)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("metadata.entity_type.get_by_code", CommandMetadata::requires("metadata.read"), {
            let metadata = metadata.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let metadata = metadata.clone();
                async move {
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let code = require(&params, "code")?;
                    let entity_type = metadata
                        .get_entity_type_by_code(&company_id, &code)
                        .await?;
                    encode(&entity_type)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("metadata.entity_type.list", CommandMetadata::requires("metadata.read"), {
            let metadata = metadata.clone();
            move |_params: Value, _ctx: CommandExecutionCtx| {
                let metadata = metadata.clone();
                async move {
                    let list = metadata.list_entity_types().await?;
                    let rows: Result<Vec<Value>, DomainError> = list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("metadata.entity_type.update", CommandMetadata::requires("metadata.manage"), {
            let metadata = metadata.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let metadata = metadata.clone();
                async move {
                    let mut schema = parse_schema(&params)?;
                    let company_id = schema.entity_type.company_id.clone();
                    let existing = metadata
                        .get_entity_type_by_code(&company_id, &schema.entity_type.code)
                        .await?;
                    schema.entity_type.id = existing.id;
                    schema.entity_type.created_at = existing.created_at;
                    schema.entity_type.updated_at = Utc::now();
                    let event = metadata_event(&schema, "metadata.entity_type.updated");
                    metadata
                        .update_entity_type(&schema, &[event])
                        .await?;
                    encode(&schema)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("metadata.schema.get", CommandMetadata::requires("metadata.read"), {
            let metadata = metadata.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let metadata = metadata.clone();
                async move {
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let code = require(&params, "code")?;
                    let schema = metadata
                        .get_schema(&company_id, &code)
                        .await?;
                    encode(&schema)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("metadata.export", CommandMetadata::requires("metadata.read"), {
            let metadata = metadata.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let metadata = metadata.clone();
                async move {
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let code = require(&params, "code")?;
                    metadata.export_entity_type(&company_id, &code).await
                }
            }
        })
        .await;

    registry
        .register_with_metadata("metadata.import", CommandMetadata::requires("metadata.manage"), {
            let metadata = metadata.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let metadata = metadata.clone();
                async move {
                    let schema = parse_schema(&params)?;
                    let event = metadata_event(&schema, "metadata.entity_type.imported");
                    metadata.import_entity_type(&params, &[event]).await
                }
            }
        })
        .await;
}

/// Коды ядровых сущностей, доступных через универсальные команды `object.*`
/// (подфаза 15pre-2): компании, пользователи и роли рендерятся клиентом SDUI
/// из системных метаданных.
const CORE_ENTITY_TYPES: &[&str] = &["company", "user", "role"];

fn is_core_entity_type(entity_type: &str) -> bool {
    CORE_ENTITY_TYPES.contains(&entity_type)
}

/// Ядровая сущность, найденная по идентификатору для делегирования `object.*`.
enum CoreEntity {
    Company(Company),
    User(User),
    Role(Role),
}

/// Собирает универсальную JSON-форму объекта Доски для ядровой сущности:
/// клиент SDUI рендерит компании/пользователей/роли теми же виджетами, что
/// и обычные объекты.
fn core_object_json(
    id: &str,
    entity_type: &str,
    company_id: &str,
    state: &str,
    data: Value,
) -> Value {
    json!({
        "id": id,
        "entity_type": entity_type,
        "kind": "Catalog",
        "company_id": company_id,
        "state": state,
        "data": data,
        "computed": {},
        "version": 1,
        "number": null,
        "date": null,
        "parent_id": null,
    })
}

fn company_to_json(company: &Company) -> Value {
    core_object_json(
        &company.id.to_string(),
        "company",
        "",
        "active",
        json!({
            "code": company.code,
            "name": company.name,
            "is_active": company.is_active,
        }),
    )
}

fn user_to_json(user: &User) -> Value {
    let state = match user.status {
        UserStatus::Active | UserStatus::Invited => "active",
        UserStatus::Disabled | UserStatus::Locked => "blocked",
        UserStatus::Archived => "archived",
    };
    core_object_json(
        &user.id.to_string(),
        "user",
        "",
        state,
        json!({
            "login": user.login,
            "status": user.status,
            "role_ids": user.role_ids,
            "locale": user.locale,
            "timezone": user.timezone,
        }),
    )
}

fn role_to_json(role: &Role) -> Value {
    core_object_json(
        &role.id.to_string(),
        "role",
        &role.company_id.to_string(),
        "active",
        json!({
            "code": role.code,
            "name": role.name,
            "permission_policy_codes": role.permission_policy_codes,
            "is_system": role.is_system,
        }),
    )
}

fn encode_core_entity(entity: &CoreEntity) -> Value {
    match entity {
        CoreEntity::Company(company) => company_to_json(company),
        CoreEntity::User(user) => user_to_json(user),
        CoreEntity::Role(role) => role_to_json(role),
    }
}

/// Ищет ядровую сущность по id в репозиториях компаний/пользователей/ролей.
async fn fetch_core_entity(
    companies: &SurrealCompanyRepository,
    users: &SurrealUserRepository,
    roles: &SurrealRoleRepository,
    id: &Uuid,
) -> Result<CoreEntity, DomainError> {
    match companies.get(id).await {
        Ok(company) => return Ok(CoreEntity::Company(company)),
        Err(DomainError::NotFound(_)) => {}
        Err(e) => return Err(e),
    }
    match users.get(id).await {
        Ok(user) => return Ok(CoreEntity::User(user)),
        Err(DomainError::NotFound(_)) => {}
        Err(e) => return Err(e),
    }
    match roles.get(id).await {
        Ok(role) => return Ok(CoreEntity::Role(role)),
        Err(DomainError::NotFound(_)) => {}
        Err(e) => return Err(e),
    }
    Err(DomainError::NotFound(format!(
        "Объект {id} не найден в ядровых типах"
    )))
}

/// Список ядровых сущностей в универсальной форме `object.list`. Компании —
/// глобальные, роли и пользователи фильтруются по `company_id` (пользователь
/// принадлежит компании, если ему назначена роль этой компании).
async fn list_core_objects(
    companies: &SurrealCompanyRepository,
    users: &SurrealUserRepository,
    roles: &SurrealRoleRepository,
    entity_type: &str,
    company_id: &str,
    limit: usize,
) -> Result<Value, DomainError> {
    match entity_type {
        "company" => {
            let all = companies.list().await?;
            let rows: Vec<Value> = all.iter().take(limit).map(company_to_json).collect();
            Ok(Value::Array(rows))
        }
        "user" => {
            let all = users.list().await?;
            let rows: Vec<Value> = match Uuid::parse_str(company_id).ok() {
                Some(cid) => {
                    let role_ids: HashSet<String> = roles
                        .list()
                        .await?
                        .into_iter()
                        .filter(|role| role.company_id == cid)
                        .map(|role| role.id.to_string())
                        .collect();
                    all.iter()
                        .filter(|user| user.role_ids.iter().any(|rid| role_ids.contains(rid)))
                        .take(limit)
                        .map(user_to_json)
                        .collect()
                }
                None => all.iter().take(limit).map(user_to_json).collect(),
            };
            Ok(Value::Array(rows))
        }
        "role" => {
            let all = roles.list().await?;
            let cid = Uuid::parse_str(company_id).ok();
            let rows: Vec<Value> = all
                .iter()
                .filter(|role| cid.is_none_or(|c| role.company_id == c))
                .take(limit)
                .map(role_to_json)
                .collect();
            Ok(Value::Array(rows))
        }
        _ => Err(DomainError::ValidationError(format!(
            "неизвестный ядровой тип сущности: {entity_type}"
        ))),
    }
}

/// Создаёт ядровую сущность из универсальных параметров `object.create`.
/// Для пользователя в `data` дополнительно обязателен пароль.
async fn create_core_object(
    companies: &SurrealCompanyRepository,
    users: &SurrealUserRepository,
    roles: &SurrealRoleRepository,
    entity_type: &str,
    company_id: &str,
    params: &Value,
) -> Result<Value, DomainError> {
    let data = params.get("data").cloned().unwrap_or(json!({}));
    let now = Utc::now();
    match entity_type {
        "company" => {
            let company = Company {
                id: Uuid::new_v4(),
                code: require(&data, "code")?,
                name: require(&data, "name")?,
                is_active: optional_bool(&data, "is_active")?.unwrap_or(true),
                created_at: now,
                updated_at: now,
            };
            let payload = encode(&company)?;
            let event = system_event(
                StreamType::Company,
                company.id.to_string(),
                "company.created",
                &company.id.to_string(),
                payload,
            );
            companies.create(&company, &[event]).await?;
            Ok(company_to_json(&company))
        }
        "user" => {
            let login = require(&data, "login")?;
            let password = require(&data, "password")?;
            let password_hash = hash_password(&password).map_err(DomainError::ValidationError)?;
            let last_name = optional(&data, "last_name")?.unwrap_or_default();
            let first_name = optional(&data, "first_name")?.unwrap_or_else(|| login.clone());
            let middle_name = optional(&data, "middle_name")?;
            let status = parse_user_status(&data, "status")?.unwrap_or(UserStatus::Active);
            let locale = optional(&data, "locale")?.unwrap_or_else(|| "ru-RU".to_string());
            let timezone =
                optional(&data, "timezone")?.unwrap_or_else(|| "Europe/Moscow".to_string());
            let role_ids = parse_uuids(&data, "role_ids")?;
            let display_name = format!(
                "{last_name} {first_name}{}",
                middle_name
                    .as_ref()
                    .map(|m| format!(" {m}"))
                    .unwrap_or_default()
            );
            let user_id = Uuid::new_v4();
            let user = User {
                id: user_id,
                login,
                password_hash,
                status,
                role_ids,
                failed_login_count: 0,
                locked_until: None,
                must_change_password: true,
                locale,
                timezone,
                person_id: Some(user_id),
                created_at: now,
                updated_at: now,
            };
            let person = Person {
                id: user_id,
                user_id,
                last_name,
                first_name,
                middle_name,
                display_name,
            };
            let events = vec![
                system_event(
                    StreamType::User,
                    user.id.to_string(),
                    "user.created",
                    company_id,
                    encode(&user)?,
                ),
                system_event(
                    StreamType::Person,
                    person.id.to_string(),
                    "person.created",
                    company_id,
                    encode(&person)?,
                ),
            ];
            users.create(&user, &person, &events).await?;
            Ok(user_to_json(&user))
        }
        "role" => {
            let role = Role {
                id: Uuid::new_v4(),
                company_id: Uuid::parse_str(company_id).map_err(|_| {
                    DomainError::ValidationError(
                        "role: company_id обязателен и должен быть UUID".to_string(),
                    )
                })?,
                code: require(&data, "code")?,
                name: require(&data, "name")?,
                description: optional(&data, "description")?.unwrap_or_default(),
                permission_policy_codes: parse_uuids(&data, "permission_policy_codes")?,
                is_system: optional_bool(&data, "is_system")?.unwrap_or(false),
                created_at: now,
                updated_at: now,
            };
            let payload = encode(&role)?;
            let event = system_event(
                StreamType::Role,
                role.id.to_string(),
                "role.created",
                &role.company_id.to_string(),
                payload,
            );
            roles.create(&role, &[event]).await?;
            Ok(role_to_json(&role))
        }
        _ => Err(DomainError::ValidationError(format!(
            "неизвестный ядровой тип сущности: {entity_type}"
        ))),
    }
}

/// Обновляет ядровую сущность из универсальных параметров `object.update`.
async fn update_core_object(
    companies: &SurrealCompanyRepository,
    users: &SurrealUserRepository,
    roles: &SurrealRoleRepository,
    params: &Value,
) -> Result<Value, DomainError> {
    let id = parse_uuid(params, "id")?;
    let data = params.get("data").cloned().unwrap_or(json!({}));
    match fetch_core_entity(companies, users, roles, &id).await? {
        CoreEntity::Company(existing) => {
            let company = Company {
                id,
                code: optional(&data, "code")?.unwrap_or(existing.code),
                name: optional(&data, "name")?.unwrap_or(existing.name),
                is_active: optional_bool(&data, "is_active")?.unwrap_or(existing.is_active),
                created_at: existing.created_at,
                updated_at: Utc::now(),
            };
            let payload = encode(&company)?;
            let event = system_event(
                StreamType::Company,
                company.id.to_string(),
                "company.updated",
                &company.id.to_string(),
                payload,
            );
            companies.update(&company, &[event]).await?;
            Ok(company_to_json(&company))
        }
        CoreEntity::User(existing) => {
            let user = User {
                id,
                login: optional(&data, "login")?.unwrap_or(existing.login),
                password_hash: existing.password_hash,
                status: parse_user_status(&data, "status")?.unwrap_or(existing.status),
                role_ids: match data.get("role_ids") {
                    None => existing.role_ids,
                    Some(_) => parse_uuids(&data, "role_ids")?,
                },
                failed_login_count: existing.failed_login_count,
                locked_until: existing.locked_until,
                must_change_password: existing.must_change_password,
                locale: optional(&data, "locale")?.unwrap_or(existing.locale),
                timezone: optional(&data, "timezone")?.unwrap_or(existing.timezone),
                person_id: existing.person_id,
                created_at: existing.created_at,
                updated_at: Utc::now(),
            };
            let payload = encode(&user)?;
            let event = system_event(
                StreamType::User,
                user.id.to_string(),
                "user.updated",
                "",
                payload,
            );
            users.update(&user, &[event]).await?;
            Ok(user_to_json(&user))
        }
        CoreEntity::Role(existing) => {
            let role = Role {
                id,
                company_id: existing.company_id,
                code: optional(&data, "code")?.unwrap_or(existing.code),
                name: optional(&data, "name")?.unwrap_or(existing.name),
                description: optional(&data, "description")?.unwrap_or(existing.description),
                permission_policy_codes: match data.get("permission_policy_codes") {
                    None => existing.permission_policy_codes,
                    Some(_) => parse_uuids(&data, "permission_policy_codes")?,
                },
                is_system: optional_bool(&data, "is_system")?.unwrap_or(existing.is_system),
                created_at: existing.created_at,
                updated_at: Utc::now(),
            };
            let payload = encode(&role)?;
            let event = system_event(
                StreamType::Role,
                role.id.to_string(),
                "role.updated",
                &role.company_id.to_string(),
                payload,
            );
            roles.update(&role, &[event]).await?;
            Ok(role_to_json(&role))
        }
    }
}

/// Разбирает статус пользователя из SDUI-формы: принимает как значения enum
/// `UserStatus` (invited/active/disabled/locked/archived), так и псевдоним
/// `blocked` из системной схемы типа `user`.
fn parse_user_status(value: &Value, key: &str) -> Result<Option<UserStatus>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let raw = v.as_str().ok_or_else(|| {
                DomainError::ValidationError(format!("параметр '{key}' должен быть строкой"))
            })?;
            match raw {
                "active" => Ok(Some(UserStatus::Active)),
                "invited" => Ok(Some(UserStatus::Invited)),
                "blocked" | "disabled" => Ok(Some(UserStatus::Disabled)),
                "locked" => Ok(Some(UserStatus::Locked)),
                "archived" => Ok(Some(UserStatus::Archived)),
                _ => Err(DomainError::ValidationError(format!(
                    "некорректный статус пользователя: {raw}"
                ))),
            }
        }
    }
}

/// Регистрирует набор команд Фазы 4: универсальные объекты, CRUD с OCC,
/// снимки версий и нумерацию документов. Команды валидируют данные против
/// мета-модели (`Object::validate`) и добавляют события объектов в Трубу
/// вместе с записью в Доску.
async fn register_object_commands(
    registry: &CommandRegistry,
    objects: Arc<SurrealObjectRepository>,
    metadata: Arc<SurrealMetadataRepository>,
    companies: Arc<SurrealCompanyRepository>,
    users: Arc<SurrealUserRepository>,
    roles: Arc<SurrealRoleRepository>,
) {
    registry
        .register_with_metadata("object.create", CommandMetadata::requires("create"), {
            let objects = objects.clone();
            let metadata = metadata.clone();
            let companies = companies.clone();
            let users = users.clone();
            let roles = roles.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                let metadata = metadata.clone();
                let companies = companies.clone();
                let users = users.clone();
                let roles = roles.clone();
                async move {
                    let entity_type = require(&params, "entity_type")?;
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    if is_core_entity_type(&entity_type) {
                        return create_core_object(
                            &companies,
                            &users,
                            &roles,
                            &entity_type,
                            &company_id,
                            &params,
                        )
                        .await;
                    }
                    let schema = metadata
                        .get_schema(&company_id, &entity_type)
                        .await?;
                    let kind = parse_enum::<ObjectKind>(&params, "kind")?;
                    let state = match optional(&params, "state")? {
                        Some(state) => state,
                        None => schema
                            .states
                            .iter()
                            .find(|s| s.is_initial)
                            .map(|s| s.code.clone())
                            .unwrap_or_else(|| "draft".to_string()),
                    };
                    let now = Utc::now();
                    let object = Object {
                        id: Uuid::new_v4(),
                        entity_type: entity_type.clone(),
                        kind,
                        company_id: company_id.clone(),
                        state,
                        data: params.get("data").cloned().unwrap_or(json!({})),
                        computed: json!({}),
                        number: None,
                        date: parse_optional_date(&params, "date")?,
                        parent_id: parse_optional_uuid(&params, "parent_id")?,
                        version: 1,
                        created_by: "system".to_string(),
                        updated_by: "system".to_string(),
                        created_at: now,
                        updated_at: now,
                    };
                    object
                        .validate(&schema.fields, &schema.states)?;
                    let event = system_event(
                        StreamType::Object,
                        object.id.to_string(),
                        "object.created",
                        &company_id,
                        encode(&object)?,
                    );
                    let stored = objects
                        .create(&object, &[event])
                        .await?;
                    encode(&stored)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("object.get", CommandMetadata::requires("read"), {
            let objects = objects.clone();
            let companies = companies.clone();
            let users = users.clone();
            let roles = roles.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                let companies = companies.clone();
                let users = users.clone();
                let roles = roles.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    match fetch_core_entity(&companies, &users, &roles, &id).await {
                        Ok(entity) => Ok(encode_core_entity(&entity)),
                        Err(DomainError::NotFound(_)) => {
                            let object = objects.get(&id).await?;
                            encode(&object)
                        }
                        Err(e) => Err(e),
                    }
                }
            }
        })
        .await;

    registry
        .register_with_metadata("object.list", CommandMetadata::requires("read"), {
            let objects = objects.clone();
            let companies = companies.clone();
            let users = users.clone();
            let roles = roles.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                let companies = companies.clone();
                let users = users.clone();
                let roles = roles.clone();
                async move {
                    let entity_type = require(&params, "entity_type")?;
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let limit = optional_u32(&params, "limit")?.unwrap_or(100) as usize;
                    if is_core_entity_type(&entity_type) {
                        return list_core_objects(
                            &companies,
                            &users,
                            &roles,
                            &entity_type,
                            &company_id,
                            limit,
                        )
                        .await;
                    }
                    let list = objects
                        .list(&entity_type, &company_id, limit)
                        .await?;
                    let rows: Result<Vec<Value>, DomainError> = list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("object.update", CommandMetadata::requires("update"), {
            let objects = objects.clone();
            let metadata = metadata.clone();
            let companies = companies.clone();
            let users = users.clone();
            let roles = roles.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                let metadata = metadata.clone();
                let companies = companies.clone();
                let users = users.clone();
                let roles = roles.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    match fetch_core_entity(&companies, &users, &roles, &id).await {
                        Ok(_) => return update_core_object(&companies, &users, &roles, &params).await,
                        Err(DomainError::NotFound(_)) => {}
                        Err(e) => return Err(e),
                    }
                    let expected_version = parse_u64(&params, "expected_version")?;
                    let existing = objects.get(&id).await?;
                    let schema = metadata
                        .get_schema(&existing.company_id, &existing.entity_type)
                        .await?;
                    let updated = Object {
                        id,
                        entity_type: existing.entity_type.clone(),
                        kind: existing.kind,
                        company_id: existing.company_id.clone(),
                        state: optional(&params, "state")?.unwrap_or(existing.state.clone()),
                        data: match params.get("data") {
                            None => existing.data.clone(),
                            Some(data) => data.clone(),
                        },
                        computed: existing.computed.clone(),
                        number: existing.number.clone(),
                        date: match parse_optional_date(&params, "date")? {
                            Some(date) => Some(date),
                            None if params.get("date").is_some() => existing.date,
                            None => existing.date,
                        },
                        parent_id: match parse_optional_uuid(&params, "parent_id")? {
                            Some(parent_id) => Some(parent_id),
                            None if params.get("parent_id").is_some() => existing.parent_id,
                            None => existing.parent_id,
                        },
                        version: expected_version,
                        created_by: existing.created_by.clone(),
                        updated_by: "system".to_string(),
                        created_at: existing.created_at,
                        updated_at: Utc::now(),
                    };
                    updated
                        .validate(&schema.fields, &schema.states)?;
                    let event = system_event(
                        StreamType::Object,
                        id.to_string(),
                        "object.updated",
                        &existing.company_id,
                        encode(&updated)?,
                    );
                    let stored = objects
                        .update(&updated, &[event])
                        .await?;
                    encode(&stored)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("object.delete", CommandMetadata::requires("delete"), {
            let objects = objects.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let existing = objects.get(&id).await?;
                    let event = system_event(
                        StreamType::Object,
                        id.to_string(),
                        "object.deleted",
                        &existing.company_id,
                        json!({}),
                    );
                    objects
                        .delete(&id, &[event])
                        .await?;
                    Ok(json!({ "deleted": true, "id": id.to_string() }))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("object.snapshot.list", CommandMetadata::requires("read"), {
            let objects = objects.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                async move {
                    let id = parse_uuid(&params, "object_id")?;
                    let snapshots = objects
                        .get_snapshots(&id)
                        .await?;
                    let rows: Result<Vec<Value>, DomainError> = snapshots.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("object.snapshot.restore", CommandMetadata::requires("update"), {
            let objects = objects.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                async move {
                    let id = parse_uuid(&params, "object_id")?;
                    let version = parse_u64(&params, "version")?;
                    let existing = objects.get(&id).await?;
                    let event = system_event(
                        StreamType::Object,
                        id.to_string(),
                        "object.restored",
                        &existing.company_id,
                        json!({ "version": version }),
                    );
                    let stored = objects
                        .restore_snapshot(&id, version, &[event])
                        .await?;
                    encode(&stored)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("document.number.next", CommandMetadata::requires("create"), {
            let objects = objects.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                async move {
                    let entity_type = require(&params, "entity_type")?;
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let number = objects
                        .next_document_number(&entity_type, &company_id)
                        .await?;
                    Ok(json!({ "number": number }))
                }
            }
        })
        .await;
}

fn metadata_event(schema: &EntitySchema, event_type: &str) -> Event {
    Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::Metadata,
        stream_id: schema.entity_type.id.to_string(),
        event_type: event_type.to_string(),
        version: 0,
        payload: encode(schema).unwrap_or_else(|_| json!({})),
        metadata: ActorSnapshot::system(),
        company_id: schema.entity_type.company_id.clone(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: Utc::now(),
    }
}

fn parse_schema(value: &Value) -> Result<EntitySchema, DomainError> {
    let entity_type_value = value
        .get("entity_type")
        .ok_or_else(|| DomainError::ValidationError("отсутствует обязательный параметр 'entity_type'".to_string()))?;
    let id = match entity_type_value.get("id") {
        Some(Value::String(raw)) => Uuid::parse_str(raw)
            .map_err(|e| DomainError::ValidationError(format!("некорректный UUID 'entity_type.id': {e}")))?,
        _ => Uuid::new_v4(),
    };
    let code = require(entity_type_value, "code")?;
    let name = require(entity_type_value, "name")?;
    let kind = parse_enum::<EntityKind>(entity_type_value, "kind")?;
    let company_id = optional(entity_type_value, "company_id")?.unwrap_or_default();
    let metadata_version = parse_u32(entity_type_value, "metadata_version")?;
    let is_system = optional_bool(entity_type_value, "is_system")?.unwrap_or(false);
    let now = Utc::now();
    let entity_type = EntityType {
        id,
        code: code.clone(),
        name,
        kind,
        company_id,
        metadata_version,
        is_system,
        created_at: now,
        updated_at: now,
    };
    Ok(EntitySchema {
        fields: parse_fields(value, &code)?,
        states: parse_states(value, &code)?,
        transitions: parse_transitions(value, &code)?,
        forms: parse_forms(value, &code)?,
        actions: parse_actions(value, &code)?,
        relations: parse_relations(value, &code)?,
        entity_type,
    })
}

fn parse_fields(value: &Value, entity_type_code: &str) -> Result<Vec<EntityField>, DomainError> {
    let items = items(value, "fields")?;
    items
        .iter()
        .map(|item| {
            Ok(EntityField {
                id: Uuid::new_v4(),
                entity_type: entity_type_code.to_string(),
                code: require(item, "code")?,
                label: require(item, "label")?,
                data_type: parse_enum::<FieldType>(item, "data_type")?,
                required: optional_bool(item, "required")?.unwrap_or(false),
                is_unique: optional_bool(item, "is_unique")?.unwrap_or(false),
                is_indexed: optional_bool(item, "is_indexed")?.unwrap_or(false),
                options: item.get("options").cloned().unwrap_or(json!({})),
                is_system: optional_bool(item, "is_system")?.unwrap_or(false),
                order: optional_u32(item, "order")?.unwrap_or(0),
            })
        })
        .collect()
}

fn parse_states(value: &Value, entity_type_code: &str) -> Result<Vec<EntityState>, DomainError> {
    let items = items(value, "states")?;
    items
        .iter()
        .map(|item| {
            Ok(EntityState {
                id: Uuid::new_v4(),
                entity_type: entity_type_code.to_string(),
                code: require(item, "code")?,
                label: require(item, "label")?,
                color: optional(item, "color")?,
                is_initial: optional_bool(item, "is_initial")?.unwrap_or(false),
                is_final: optional_bool(item, "is_final")?.unwrap_or(false),
            })
        })
        .collect()
}

fn parse_transitions(value: &Value, entity_type_code: &str) -> Result<Vec<EntityTransition>, DomainError> {
    let items = items(value, "transitions")?;
    items
        .iter()
        .map(|item| {
            Ok(EntityTransition {
                id: Uuid::new_v4(),
                entity_type: entity_type_code.to_string(),
                code: require(item, "code")?,
                label: require(item, "label")?,
                from_state: require(item, "from_state")?,
                to_state: require(item, "to_state")?,
            })
        })
        .collect()
}

fn parse_forms(value: &Value, entity_type_code: &str) -> Result<Vec<EntityForm>, DomainError> {
    let items = items(value, "forms")?;
    items
        .iter()
        .map(|item| {
            Ok(EntityForm {
                id: Uuid::new_v4(),
                entity_type: entity_type_code.to_string(),
                code: require(item, "code")?,
                label: require(item, "label")?,
                layout: item.get("layout").cloned().unwrap_or(json!({})),
            })
        })
        .collect()
}

fn parse_actions(value: &Value, entity_type_code: &str) -> Result<Vec<EntityAction>, DomainError> {
    let items = items(value, "actions")?;
    items
        .iter()
        .map(|item| {
            Ok(EntityAction {
                id: Uuid::new_v4(),
                entity_type: entity_type_code.to_string(),
                code: require(item, "code")?,
                label: require(item, "label")?,
                handler: require(item, "handler")?,
            })
        })
        .collect()
}

fn parse_relations(value: &Value, entity_type_code: &str) -> Result<Vec<EntityRelation>, DomainError> {
    let items = items(value, "relations")?;
    items
        .iter()
        .map(|item| {
            Ok(EntityRelation {
                id: Uuid::new_v4(),
                entity_type: entity_type_code.to_string(),
                code: require(item, "code")?,
                target_type: require(item, "target_type")?,
                kind: parse_enum::<RelationKind>(item, "kind")?,
                on_delete: parse_enum::<OnDelete>(item, "on_delete")?,
            })
        })
        .collect()
}

fn items(value: &Value, key: &str) -> Result<Vec<Value>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(_) => Err(DomainError::ValidationError(format!("параметр '{key}' должен быть массивом"))),
    }
}

fn parse_u32(value: &Value, key: &str) -> Result<u32, DomainError> {
    value
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| DomainError::ValidationError(format!("отсутствует обязательный параметр '{key}'")))
}

fn parse_u64(value: &Value, key: &str) -> Result<u64, DomainError> {
    value
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| DomainError::ValidationError(format!("отсутствует обязательный параметр '{key}'")))
}

fn optional_u32(value: &Value, key: &str) -> Result<Option<u32>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(|v| Some(v as u32))
            .ok_or_else(|| DomainError::ValidationError(format!("параметр '{key}' должен быть числом"))),
    }
}

/// Регистрирует набор команд Фазы 3 в общем реестре.
pub async fn register_phase3_commands(
    registry: &CommandRegistry,
    metadata: Arc<SurrealMetadataRepository>,
) {
    register_metadata_commands(registry, metadata).await;
}

/// Регистрирует набор команд Фазы 4 в общем реестре.
pub async fn register_phase4_commands(
    registry: &CommandRegistry,
    objects: Arc<SurrealObjectRepository>,
    metadata: Arc<SurrealMetadataRepository>,
    companies: Arc<SurrealCompanyRepository>,
    users: Arc<SurrealUserRepository>,
    roles: Arc<SurrealRoleRepository>,
) {
    register_object_commands(registry, objects, metadata, companies, users, roles).await;
}

/// Регистрирует набор команд Фазы 5 по ТЗ v3.1: сидинг системных ролей и
/// политик (`role.seed`) и лечение компаний, созданных до Фазы 5
/// (`system.migrate_permissions`). Обе команды требуют права `role.manage`,
/// что соответствует Рекомендации 2: администратор (политика `platform.full`)
/// и системный исполнитель могут инициализировать RBAC-набор.
pub async fn register_phase5_commands(
    registry: &CommandRegistry,
    roles: Arc<SurrealRoleRepository>,
    policies: Arc<SurrealPermissionPolicyRepository>,
    audit: Arc<SurrealAuditRepository>,
    companies: Arc<SurrealCompanyRepository>,
) {
    registry
        .register_with_metadata("role.seed", CommandMetadata::requires("role.manage"), {
            let roles = roles.clone();
            let policies = policies.clone();
            let audit = audit.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let roles = roles.clone();
                let policies = policies.clone();
                let audit = audit.clone();
                async move {
                    let company_id = parse_uuid(&params, "company_id")?;
                    seed_system_roles_and_policies(
                        &company_id,
                        roles.as_ref(),
                        policies.as_ref(),
                        audit.as_ref(),
                    )
                    .await?;
                    Ok(json!({ "seeded": true, "company_id": company_id.to_string() }))
                }
            }
        })
        .await;

    registry
        .register_with_metadata(
            "system.migrate_permissions",
            CommandMetadata::requires("role.manage"),
            {
                let roles = roles.clone();
                let policies = policies.clone();
                let audit = audit.clone();
                let companies = companies.clone();
                move |_params: Value, _ctx: CommandExecutionCtx| {
                    let roles = roles.clone();
                    let policies = policies.clone();
                    let audit = audit.clone();
                    let companies = companies.clone();
                    async move {
                        let companies_list = companies
                            .list()
                            .await?;
                        let mut seeded = 0usize;
                        let mut policies_added = 0usize;
                        for company in &companies_list {
                            if roles.get_by_code(&company.id, "admin").await.is_err() {
                                seeded += 1;
                            }
                            let summary = seed_system_roles_and_policies(
                                &company.id,
                                roles.as_ref(),
                                policies.as_ref(),
                                audit.as_ref(),
                            )
                            .await?;
                            policies_added += summary.policies_added;
                        }
                        let entry = AuditEntry {
                            id: Uuid::new_v4(),
                            action: "system.migrate_permissions".to_string(),
                            actor: ActorSnapshot::system(),
                            target: None,
                            result: AuditResult::Success,
                            details: Some(json!({
                                "companies_total": companies_list.len(),
                                "seeded": seeded,
                                "policies_added": policies_added,
                            })),
                            ip_address: None,
                            user_agent: None,
                            company_id: None,
                            timestamp: Utc::now(),
                        };
                        audit.log(entry).await?;
                        Ok(json!({
                            "companies_total": companies_list.len(),
                            "seeded": seeded,
                            "policies_added": policies_added,
                        }))
                    }
                }
            },
        )
        .await;
}

/// Регистрирует команду `system.bootstrap`: однократная инициализация
/// платформы (первая компания, суперадмин, системные роли/политики).
/// Требует `system.bootstrap` — политики с таким действием нет, поэтому команда
/// доступна только системному исполнителю через `POST /debug/command` (или
/// прямому вызову из `main`); аноним и аутентифицированные пользователи
/// получают `PERMISSION_ERROR`.
pub async fn register_bootstrap_command(
    registry: &CommandRegistry,
    companies: Arc<SurrealCompanyRepository>,
    users: Arc<SurrealUserRepository>,
    roles: Arc<SurrealRoleRepository>,
    policies: Arc<SurrealPermissionPolicyRepository>,
    audit: Arc<SurrealAuditRepository>,
    metadata: Arc<SurrealMetadataRepository>,
) {
    registry
        .register_with_metadata(
            "system.bootstrap",
            CommandMetadata::requires("system.bootstrap"),
            {
                let companies = companies.clone();
                let users = users.clone();
                let roles = roles.clone();
                let policies = policies.clone();
                let audit = audit.clone();
                let metadata = metadata.clone();
                move |params: Value, _ctx: CommandExecutionCtx| {
                    let companies = companies.clone();
                    let users = users.clone();
                    let roles = roles.clone();
                    let policies = policies.clone();
                    let audit = audit.clone();
                    let metadata = metadata.clone();
                    async move {
                        let params: BootstrapParams = serde_json::from_value(params).map_err(|e| {
                            DomainError::ValidationError(format!(
                                "system.bootstrap параметры: {e}"
                            ))
                        })?;
                        let result = core_application::bootstrap_platform(
                            params,
                            companies.as_ref(),
                            users.as_ref(),
                            roles.as_ref(),
                            policies.as_ref(),
                            audit.as_ref(),
                            metadata.as_ref(),
                        )
                        .await?;
                        encode(&result)
                    }
                }
            },
        )
        .await;
}

/// Регистрирует набор команд Фазы 2 в общем реестре.
pub async fn register_phase2_commands(
    registry: &CommandRegistry,
    companies: Arc<SurrealCompanyRepository>,
    users: Arc<SurrealUserRepository>,
    roles: Arc<SurrealRoleRepository>,
) {
    register_company_commands(registry, companies).await;
    register_user_commands(registry, users).await;
    register_role_commands(registry, roles).await;
}

/// Регистрирует набор команд Фазы 4 по ТЗ v3.1: операционный аудит
/// (`audit_log`) — запись и чтение записей действий пользователей и системы.
/// Команды пока используют системного исполнителя (`ActorSnapshot::system()`),
/// аутентификация появится позже.
pub async fn register_phase4_audit_commands(
    registry: &CommandRegistry,
    audit: Arc<SurrealAuditRepository>,
) {
    registry
        .register_with_metadata("audit.log", CommandMetadata::requires("role.manage"), {
            let audit = audit.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let audit = audit.clone();
                async move {
                    let args: LogAuditArgs = serde_json::from_value(params)
                        .map_err(|e| DomainError::ValidationError(format!("audit.log параметры: {e}")))?;
                    let entry = AuditEntry {
                        id: Uuid::new_v4(),
                        action: args.action,
                        actor: args.actor.unwrap_or_else(ActorSnapshot::system),
                        target: args.target,
                        result: args.result.unwrap_or(AuditResult::Success),
                        details: args.details,
                        ip_address: args.ip_address,
                        user_agent: args.user_agent,
                        company_id: args.company_id,
                        timestamp: Utc::now(),
                    };
                    audit.log(entry.clone()).await?;
                    encode(&entry)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("audit.query", CommandMetadata::requires("audit.read"), {
            let audit = audit.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let audit = audit.clone();
                async move {
                    let args: QueryAuditArgs = serde_json::from_value(params)
                        .map_err(|e| DomainError::ValidationError(format!("audit.query параметры: {e}")))?;
                    let filter = AuditFilter {
                        action: args.action,
                        actor_user_id: args.actor_user_id,
                        target_entity_type: args.target_entity_type,
                        target_entity_id: args.target_entity_id,
                        company_id: args.company_id,
                        result_success: args.result_success,
                        from: args.from,
                        to: args.to,
                        limit: args.limit,
                    };
                    let entries = audit.query(filter).await?;
                    let rows: Result<Vec<Value>, DomainError> = entries.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;
}

/// Параметры команды `audit.log`.
#[derive(serde::Deserialize)]
struct LogAuditArgs {
    action: String,
    #[serde(default)]
    actor: Option<ActorSnapshot>,
    #[serde(default)]
    target: Option<AuditTarget>,
    #[serde(default)]
    result: Option<AuditResult>,
    #[serde(default)]
    details: Option<Value>,
    #[serde(default)]
    ip_address: Option<String>,
    #[serde(default)]
    user_agent: Option<String>,
    #[serde(default)]
    company_id: Option<Uuid>,
}

/// Параметры команды `audit.query`.
#[derive(serde::Deserialize)]
struct QueryAuditArgs {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    actor_user_id: Option<Uuid>,
    #[serde(default)]
    target_entity_type: Option<String>,
    #[serde(default)]
    target_entity_id: Option<Uuid>,
    #[serde(default)]
    company_id: Option<Uuid>,
    #[serde(default)]
    result_success: Option<bool>,
    #[serde(default)]
    from: Option<DateTime<Utc>>,
    #[serde(default)]
    to: Option<DateTime<Utc>>,
    #[serde(default)]
    limit: Option<usize>,
}

fn parse_uuid(value: &Value, key: &str) -> Result<Uuid, DomainError> {
    let raw = require(value, key)?;
    Uuid::parse_str(&raw).map_err(|e| DomainError::ValidationError(format!("некорректный UUID '{key}': {e}")))
}

fn parse_optional_uuid(value: &Value, key: &str) -> Result<Option<Uuid>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => parse_uuid(value, key).map(Some),
    }
}

fn parse_optional_enum<T: serde::de::DeserializeOwned>(
    value: &Value,
    key: &str,
) -> Result<Option<T>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let raw = v
                .as_str()
                .ok_or_else(|| DomainError::ValidationError(format!("параметр '{key}' должен быть строкой")))?;
            serde_json::from_value(Value::String(raw.to_string()))
                .map(Some)
                .map_err(|e| DomainError::ValidationError(format!("некорректное значение '{key}': {e}")))
        }
    }
}

fn parse_optional_date(value: &Value, key: &str) -> Result<Option<NaiveDate>, DomainError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let raw = v
                .as_str()
                .ok_or_else(|| DomainError::ValidationError(format!("параметр '{key}' должен быть строкой даты")))?;
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .map(Some)
                .map_err(|e| DomainError::ValidationError(format!("некорректная дата '{key}': {e}")))
        }
    }
}
/// Регистрирует команды управления WASM-модулями (подфаза 9b):
/// `module.install/uninstall/enable/disable/list/info` — требуют глобального
/// права `module.manage`; `module.navigation` (подфаза 11b-prep) — требует
/// `module.read` (политика `platform.modules`, доступна `staff`/`guest`).
pub async fn register_phase9_module_commands(
    registry: &CommandRegistry,
    manager: Arc<ModuleManager>,
    companies: Arc<SurrealCompanyRepository>,
    permissions: Arc<PermissionManager>,
) {
    registry
        .register_with_metadata("module.install", CommandMetadata::requires("module.manage"), {
            let manager = manager.clone();
            let companies = companies.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let manager = manager.clone();
                let companies = companies.clone();
                async move {
                    let code = require(&params, "code")?;
                    let wasm_base64 = require(&params, "wasm_base64")?;
                    let company_id = require(&params, "company_id")?;
                    let company_uuid = Uuid::parse_str(&company_id)
                        .map_err(|e| DomainError::ValidationError(format!("некорректный 'company_id': {e}")))?;
                    companies
                        .get(&company_uuid)
                        .await
                        .map_err(|e| DomainError::NotFound(format!("компания не найдена: {e}")))?;
                    let wasm_bytes =
                        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &wasm_base64)
                            .map_err(|e| DomainError::ValidationError(format!("некорректный wasm_base64: {e}")))?;
                    let record = manager
                        .install(&code, &wasm_bytes, &company_id)
                        .await?;
                    encode(&record)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("module.uninstall", CommandMetadata::requires("module.manage"), {
            let manager = manager.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let manager = manager.clone();
                async move {
                    let code = require(&params, "code")?;
                    let record = manager.uninstall(&code).await?;
                    encode(&record)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("module.enable", CommandMetadata::requires("module.manage"), {
            let manager = manager.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let manager = manager.clone();
                async move {
                    let code = require(&params, "code")?;
                    let company_id = require(&params, "company_id")?;
                    Uuid::parse_str(&company_id)
                        .map_err(|e| DomainError::ValidationError(format!("некорректный 'company_id': {e}")))?;
                    let record = manager
                        .enable(&code, &company_id)
                        .await?;
                    encode(&record)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("module.disable", CommandMetadata::requires("module.manage"), {
            let manager = manager.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let manager = manager.clone();
                async move {
                    let code = require(&params, "code")?;
                    let company_id = require(&params, "company_id")?;
                    manager
                        .disable(&code, &company_id)
                        .await?;
                    Ok(json!({ "disabled": code, "company_id": company_id }))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("module.list", CommandMetadata::requires("module.manage"), {
            let manager = manager.clone();
            move |_params: Value, _ctx: CommandExecutionCtx| {
                let manager = manager.clone();
                async move {
                    let modules = manager.modules();
                    let records = modules.list().await?;
                    let rows: Result<Vec<Value>, DomainError> = records.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("module.info", CommandMetadata::requires("module.manage"), {
            let manager = manager.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let modules = manager.modules();
                async move {
                    let code = require(&params, "code")?;
                    let record = modules.get(&code).await?;
                    encode(&record)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("module.navigation", CommandMetadata::requires("module.read"), {
            let manager = manager.clone();
            let permissions = permissions.clone();
            move |params: Value, ctx: CommandExecutionCtx| {
                let manager = manager.clone();
                let permissions = permissions.clone();
                async move {
                    let company_id = ctx
                        .actor
                        .as_ref()
                        .and_then(|a| a.company_id)
                        .or_else(|| {
                            params
                                .get("company_id")
                                .and_then(|v| v.as_str())
                                .and_then(|c| Uuid::parse_str(c).ok())
                        })
                        .ok_or_else(|| {
                            DomainError::ValidationError(
                                "команда доступна только пользователю с компанией".to_string(),
                            )
                        })?;
                    manager
                        .get_navigation(company_id, ctx.actor, &permissions)
                        .await
                }
            }
        })
        .await;
}

/// Команды аутентификации Фазы 10b: логин/выход через JWT на базе `AuthService`.
pub async fn register_phase10_commands(
    registry: &CommandRegistry,
    auth: std::sync::Arc<AuthService>,
) {
    registry
        .register_with_metadata("user.login", CommandMetadata::unrestricted(), {
            let auth = auth.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let auth = auth.clone();
                async move {
                    let login = require(&params, "login")?;
                    let password = require(&params, "password")?;
                    let token = auth.login(&login, &password, None, None).await?;
                    encode(&token)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("user.logout", CommandMetadata::unrestricted(), {
            let auth = auth.clone();
            move |_: Value, _ctx: CommandExecutionCtx| {
                let auth = auth.clone();
                async move {
                    auth.logout(None, None, None).await?;
                    Ok(json!({ "logout": true }))
                }
            }
        })
        .await;
}

/// Команды управления скриптами Rhai Фазы 13d.
///
/// `script.create/update/delete/test` — право `script.manage`; `script.get/list/validate` —
/// `script.read`; `script.execute` — `script.execute`. Права выдаются политикой
/// `platform.scripts` (seed), deny-by-default RBAC, префиксные команды.
///
/// `script.validate` дополнительно проверяет существование привязанного типа
/// сущности (`metadata`); `script.test` выполняет скрипт в тестовом режиме
/// (`test_run = true`) без влияния на данные.
pub async fn register_phase13_commands(
    registry: &CommandRegistry,
    scripts: Arc<SurrealScriptRepository>,
    metadata: Arc<dyn MetadataRepository>,
    engine: Arc<dyn ScriptEngine>,
) {
    registry
        .register_with_metadata("script.create", CommandMetadata::requires("script.manage"), {
            let scripts = scripts.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let scripts = scripts.clone();
                async move {
                    let code = require(&params, "code")?;
                    let name = require(&params, "name")?;
                    let source = require(&params, "source")?;
                    if code.is_empty() || source.is_empty() {
                        return Err(DomainError::ValidationError(
                            "script.create: code и source не могут быть пустыми".to_string(),
                        ));
                    }
                    let script_type = parse_script_type(&params)?;
                    let company_id = params
                        .get("company_id")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            Uuid::parse_str(s).map_err(|e| {
                                DomainError::ValidationError(format!("некорректный 'company_id': {e}"))
                            })
                        })
                        .transpose()?;
                    if scripts
                        .get_by_code(&code, company_id.as_ref())
                        .await?
                        .is_some()
                    {
                        return Err(DomainError::ValidationError(format!(
                            "скрипт с кодом '{code}' уже существует"
                        )));
                    }
                    let entity_type = params
                        .get("entity_type")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    let is_active = params
                        .get("is_active")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let now = Utc::now();
                    let record = Script {
                        id: Uuid::new_v4(),
                        code,
                        name,
                        script_type,
                        source,
                        company_id,
                        module_code: None,
                        entity_type,
                        is_active,
                        created_at: now,
                        updated_at: now,
                    };
                    let event = script_event(&record, "script.created");
                    let created = scripts.create(&record, &[event]).await?;
                    encode(&created)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("script.update", CommandMetadata::requires("script.manage"), {
            let scripts = scripts.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let scripts = scripts.clone();
                async move {
                    let mut record = resolve_script(&scripts, &params).await?;
                    if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                        record.name = name.to_string();
                    }
                    if let Some(source) = params.get("source").and_then(|v| v.as_str()) {
                        if source.is_empty() {
                            return Err(DomainError::ValidationError(
                                "script.update: source не может быть пустым".to_string(),
                            ));
                        }
                        record.source = source.to_string();
                    }
                    if let Some(raw) = params.get("script_type").and_then(|v| v.as_str()) {
                        record.script_type =
                            ScriptType::try_from(raw).map_err(DomainError::ValidationError)?;
                    }
                    if let Some(v) = params.get("entity_type") {
                        record.entity_type = v.as_str().map(str::to_string);
                    }
                    if let Some(v) = params.get("is_active").and_then(|v| v.as_bool()) {
                        record.is_active = v;
                    }
                    record.updated_at = Utc::now();
                    let event = script_event(&record, "script.updated");
                    let updated = scripts.update(&record, &[event]).await?;
                    encode(&updated)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("script.list", CommandMetadata::requires("script.read"), {
            let scripts = scripts.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let scripts = scripts.clone();
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
                    let records = scripts.list(company_id.as_ref()).await?;
                    let rows: Result<Vec<Value>, DomainError> = records.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("script.delete", CommandMetadata::requires("script.manage"), {
            let scripts = scripts.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let scripts = scripts.clone();
                async move {
                    let record = resolve_script(&scripts, &params).await?;
                    let event = script_event(&record, "script.deleted");
                    scripts.delete(&record.id, &[event]).await?;
                    Ok(json!({ "deleted": record.id }))
                }
            }
        })
        .await;

    registry
        .register_with_metadata("script.get", CommandMetadata::requires("script.read"), {
            let scripts = scripts.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let scripts = scripts.clone();
                async move {
                    let record = resolve_script(&scripts, &params).await?;
                    encode(&record)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("script.validate", CommandMetadata::requires("script.read"), {
            let scripts = scripts.clone();
            let metadata = metadata.clone();
            let engine = engine.clone();
            move |params: Value, ctx: CommandExecutionCtx| {
                let scripts = scripts.clone();
                let metadata = metadata.clone();
                let engine = engine.clone();
                async move {
                    let actor = ctx.actor.clone().unwrap_or_else(ActorSnapshot::system);
                    validate_script(&*scripts, engine.as_ref(), metadata.as_ref(), &params, &actor).await
                }
            }
        })
        .await;

    registry
        .register_with_metadata("script.test", CommandMetadata::requires("script.manage"), {
            let scripts = scripts.clone();
            let engine = engine.clone();
            move |params: Value, ctx: CommandExecutionCtx| {
                let scripts = scripts.clone();
                let engine = engine.clone();
                async move {
                    let actor = ctx
                        .actor
                        .clone()
                        .unwrap_or_else(ActorSnapshot::system);
                    test_script(&*scripts, engine.as_ref(), &params, &actor).await
                }
            }
        })
        .await;

    registry
        .register_with_metadata("script.execute", CommandMetadata::requires("script.execute"), {
            let scripts = scripts.clone();
            let engine = engine.clone();
            move |params: Value, ctx: CommandExecutionCtx| {
                let scripts = scripts.clone();
                let engine = engine.clone();
                async move {
                    let actor = ctx
                        .actor
                        .clone()
                        .unwrap_or_else(ActorSnapshot::system);
                    execute_script(&*scripts, engine.as_ref(), &params, &actor).await
                }
            }
        })
        .await;
}

/// Разрешает скрипт по `id` либо по `code` (+ необязательный `company_id`).
async fn resolve_script(
    scripts: &SurrealScriptRepository,
    params: &Value,
) -> Result<Script, DomainError> {
    let company_id = params
        .get("company_id")
        .and_then(|v| v.as_str())
        .map(|s| {
            Uuid::parse_str(s).map_err(|e| {
                DomainError::ValidationError(format!("некорректный 'company_id': {e}"))
            })
        })
        .transpose()?;
    if let Some(id_str) = params.get("id").and_then(|v| v.as_str()) {
        let id = Uuid::parse_str(id_str)
            .map_err(|e| DomainError::ValidationError(format!("некорректный 'id': {e}")))?;
        return scripts
            .get(&id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("скрипт с id '{id_str}' не найден")));
    }
    let code = require(params, "code")?;
    scripts
        .get_by_code(&code, company_id.as_ref())
        .await?
        .ok_or_else(|| DomainError::NotFound(format!("скрипт с кодом '{code}' не найден")))
}

/// Парсит тип скрипта из параметров команды; по умолчанию — `formula`.
fn parse_script_type(params: &Value) -> Result<ScriptType, DomainError> {
    match params.get("script_type").and_then(|v| v.as_str()) {
        Some(raw) => ScriptType::try_from(raw).map_err(DomainError::ValidationError),
        None => Ok(ScriptType::Formula),
    }
}

/// Событие `script.*` для Трубы от системного исполнителя.
fn script_event(script: &Script, event_type: &str) -> Event {
    let company_id = script.company_id.map(|u| u.to_string()).unwrap_or_default();
    system_event(
        StreamType::Script,
        script.id.to_string(),
        event_type,
        &company_id,
        serde_json::to_value(script).unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_application::metadata_seed::seed_system_metadata;
    use core_application::ports::{EventStore, MetadataRepository};
    use core_infrastructure::surreal_object_repository::SurrealObjectRepository;
    use std::sync::Arc;
    use surrealdb::engine::any::Any;
    use surrealdb::Surreal;

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
        objects: Arc<SurrealObjectRepository>,
        companies: Arc<SurrealCompanyRepository>,
        users: Arc<SurrealUserRepository>,
        roles: Arc<SurrealRoleRepository>,
        metadata: Arc<SurrealMetadataRepository>,
    }

    async fn setup() -> Env {
        let db = mem_db().await;

        let store = Arc::new(core_infrastructure::SurrealEventStore::new(db.clone()));
        store.ensure_schema().await.unwrap();
        let objects = Arc::new(SurrealObjectRepository::new(db.clone()));
        objects.ensure_schema().await.unwrap();
        let companies = Arc::new(SurrealCompanyRepository::new(db.clone()));
        companies.ensure_schema().await.unwrap();
        let users = Arc::new(SurrealUserRepository::new(db.clone()));
        users.ensure_schema().await.unwrap();
        let roles = Arc::new(SurrealRoleRepository::new(db.clone()));
        roles.ensure_schema().await.unwrap();
        let metadata = Arc::new(SurrealMetadataRepository::new(db.clone()));
        metadata.ensure_schema().await.unwrap();

        seed_system_metadata(metadata.as_ref()).await.unwrap();

        Env {
            _db: db,
            objects,
            companies,
            users,
            roles,
            metadata,
        }
    }

    async fn registry(env: &Env) -> core_application::CommandRegistry {
        let registry = core_application::CommandRegistry::new();
        register_metadata_commands(&registry, env.metadata.clone()).await;
        register_object_commands(
            &registry,
            env.objects.clone(),
            env.metadata.clone(),
            env.companies.clone(),
            env.users.clone(),
            env.roles.clone(),
        )
        .await;
        registry
    }

    fn note_schema() -> EntitySchema {
        let now = Utc::now();
        EntitySchema {
            entity_type: EntityType {
                id: Uuid::new_v4(),
                code: "note".to_string(),
                name: "Заметка".to_string(),
                kind: ObjectKind::Document,
                company_id: String::new(),
                metadata_version: 1,
                is_system: false,
                created_at: now,
                updated_at: now,
            },
            fields: vec![EntityField {
                id: Uuid::new_v4(),
                entity_type: "note".to_string(),
                code: "title".to_string(),
                label: "Заголовок".to_string(),
                data_type: FieldType::String,
                required: true,
                is_unique: false,
                is_indexed: false,
                options: Value::Null,
                is_system: false,
                order: 1,
            }],
            states: vec![EntityState {
                id: Uuid::new_v4(),
                entity_type: "note".to_string(),
                code: "draft".to_string(),
                label: "Черновик".to_string(),
                color: None,
                is_initial: true,
                is_final: false,
            }],
            transitions: vec![],
            forms: vec![],
            actions: vec![],
            relations: vec![],
        }
    }

    async fn create_company(registry: &core_application::CommandRegistry, code: &str, name: &str) -> Value {
        registry
            .execute(
                "object.create",
                json!({
                    "entity_type": "company",
                    "company_id": "",
                    "data": { "code": code, "name": name },
                }),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn object_crud_company_via_sdui_mapping() {
        let env = setup().await;
        let registry = registry(&env).await;

        let acme = create_company(&registry, "acme", "Acme Corp").await;
        let acme_id = acme["id"].as_str().unwrap().to_string();
        let globex = create_company(&registry, "globex", "Globex").await;

        let list = registry
            .execute(
                "object.list",
                json!({ "entity_type": "company", "company_id": "" }),
            )
            .await
            .unwrap();
        let arr = list.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let codes: Vec<&str> = arr
            .iter()
            .map(|o| o["data"]["code"].as_str().unwrap())
            .collect();
        assert_eq!(codes, vec!["acme", "globex"]);

        let got = registry
            .execute("object.get", json!({ "id": acme_id }))
            .await
            .unwrap();
        assert_eq!(got["entity_type"], "company");
        assert_eq!(got["data"]["name"], "Acme Corp");
        assert_eq!(got["data"]["is_active"], true);

        let renamed = registry
            .execute(
                "object.update",
                json!({ "id": acme_id, "data": { "name": "Acme 2.0" } }),
            )
            .await
            .unwrap();
        assert_eq!(renamed["data"]["name"], "Acme 2.0");
        assert_eq!(renamed["data"]["code"], "acme");

        assert_ne!(acme_id, globex["id"].as_str().unwrap());
    }

    #[tokio::test]
    async fn object_list_filters_users_and_roles_by_company() {
        let env = setup().await;
        let registry = registry(&env).await;

        let c1_id = create_company(&registry, "one", "Company One").await["id"]
            .as_str()
            .unwrap()
            .to_string();
        let c2_id = create_company(&registry, "two", "Company Two").await["id"]
            .as_str()
            .unwrap()
            .to_string();

        let r1 = registry
            .execute(
                "object.create",
                json!({
                    "entity_type": "role",
                    "company_id": c1_id,
                    "data": { "code": "admin", "name": "Администратор" },
                }),
            )
            .await
            .unwrap();
        let r1_id = r1["id"].as_str().unwrap().to_string();
        let r2 = registry
            .execute(
                "object.create",
                json!({
                    "entity_type": "role",
                    "company_id": c2_id,
                    "data": { "code": "guest", "name": "Гость" },
                }),
            )
            .await
            .unwrap();
        let r2_id = r2["id"].as_str().unwrap().to_string();

        registry
            .execute(
                "object.create",
                json!({
                    "entity_type": "user",
                    "company_id": "",
                    "data": { "login": "ivan", "password": "secret123", "role_ids": [r1_id] },
                }),
            )
            .await
            .unwrap();
        registry
            .execute(
                "object.create",
                json!({
                    "entity_type": "user",
                    "company_id": "",
                    "data": { "login": "petr", "password": "secret123", "role_ids": [r2_id] },
                }),
            )
            .await
            .unwrap();

        let in_c1 = registry
            .execute(
                "object.list",
                json!({ "entity_type": "user", "company_id": c1_id }),
            )
            .await
            .unwrap();
        let logins: Vec<&str> = in_c1
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["data"]["login"].as_str().unwrap())
            .collect();
        assert_eq!(logins, vec!["ivan"]);

        let roles_in_c1 = registry
            .execute(
                "object.list",
                json!({ "entity_type": "role", "company_id": c1_id }),
            )
            .await
            .unwrap();
        let role_codes: Vec<&str> = roles_in_c1
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["data"]["code"].as_str().unwrap())
            .collect();
        assert_eq!(role_codes, vec!["admin"]);
    }

    #[tokio::test]
    async fn object_get_falls_back_to_generic_objects() {
        let env = setup().await;
        let registry = registry(&env).await;

        let note_event = system_event(
            StreamType::Metadata,
            Uuid::new_v4().to_string(),
            "metadata.registered",
            "",
            json!({}),
        );
        let schema = note_schema();
        env.metadata
            .create_entity_type(&schema, &[note_event])
            .await
            .unwrap();

        let created = registry
            .execute(
                "object.create",
                json!({
                    "entity_type": "note",
                    "company_id": "",
                    "kind": "document",
                    "data": { "title": "Первая заметка" },
                }),
            )
            .await
            .unwrap();
        let note_id = created["id"].as_str().unwrap().to_string();
        assert_eq!(created["entity_type"], "note");
        assert_eq!(created["state"], "draft");

        let got = registry
            .execute("object.get", json!({ "id": note_id }))
            .await
            .unwrap();
        assert_eq!(got["data"]["title"], "Первая заметка");

        let list = registry
            .execute(
                "object.list",
                json!({ "entity_type": "note", "company_id": "" }),
            )
            .await
            .unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn object_crud_user_with_status_and_roles() {
        let env = setup().await;
        let registry = registry(&env).await;

        let created = registry
            .execute(
                "object.create",
                json!({
                    "entity_type": "user",
                    "company_id": "",
                    "data": { "login": "anna", "password": "secret123", "status": "invited" },
                }),
            )
            .await
            .unwrap();
        let uid = created["id"].as_str().unwrap().to_string();
        assert_eq!(created["data"]["login"], "anna");
        assert_eq!(created["data"]["status"], "invited");
        assert_eq!(created["data"]["locale"], "ru-RU");

        let updated = registry
            .execute(
                "object.update",
                json!({ "id": uid, "data": { "status": "active", "timezone": "Asia/Yekaterinburg" } }),
            )
            .await
            .unwrap();
        assert_eq!(updated["data"]["status"], "active");
        assert_eq!(updated["data"]["timezone"], "Asia/Yekaterinburg");
    }

    #[tokio::test]
    async fn object_create_rejects_role_without_company() {
        let env = setup().await;
        let registry = registry(&env).await;

        let err = registry
            .execute(
                "object.create",
                json!({
                    "entity_type": "role",
                    "company_id": "",
                    "data": { "code": "admin", "name": "Администратор" },
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[tokio::test]
    async fn metadata_export_returns_system_schema_json() {
        let env = setup().await;
        let registry = registry(&env).await;

        let exported = registry
            .execute("metadata.export", json!({ "code": "company", "company_id": "" }))
            .await
            .unwrap();
        assert_eq!(exported["entity_type"]["code"], "company");
        assert_eq!(exported["entity_type"]["is_system"], true);
        let codes: Vec<&str> = exported["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["code"].as_str().unwrap())
            .collect();
        assert!(codes.contains(&"code"));
        assert!(codes.contains(&"name"));
        assert!(codes.contains(&"is_active"));
    }

    #[tokio::test]
    async fn metadata_import_registers_type_and_writes_event() {
        let env = setup().await;
        let registry = registry(&env).await;

        let note = note_schema();
        let doc = serde_json::to_value(&note).unwrap();

        let imported = registry
            .execute("metadata.import", doc)
            .await
            .unwrap();
        assert_eq!(imported["entity_type"]["code"], "note");
        assert_eq!(imported["entity_type"]["kind"], "document");

        let schema = env.metadata.get_schema("", "note").await.unwrap();
        assert_eq!(schema.fields.len(), 1);
        assert_eq!(schema.states.len(), 1);

        let store = core_infrastructure::SurrealEventStore::new(env._db.clone());
        let stream = store
            .read_stream(StreamType::Metadata, &note.entity_type.id.to_string())
            .await
            .unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].event_type, "metadata.entity_type.imported");
    }
}
