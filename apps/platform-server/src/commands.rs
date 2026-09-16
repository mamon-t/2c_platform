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
use core_application::script_runner::execute_script;
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
}

/// Регистрирует набор команд Фазы 4: универсальные объекты, CRUD с OCC,
/// снимки версий и нумерацию документов. Команды валидируют данные против
/// мета-модели (`Object::validate`) и добавляют события объектов в Трубу
/// вместе с записью в Доску.
async fn register_object_commands(
    registry: &CommandRegistry,
    objects: Arc<SurrealObjectRepository>,
    metadata: Arc<SurrealMetadataRepository>,
) {
    registry
        .register_with_metadata("object.create", CommandMetadata::requires("create"), {
            let objects = objects.clone();
            let metadata = metadata.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                let metadata = metadata.clone();
                async move {
                    let entity_type = require(&params, "entity_type")?;
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
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
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let object = objects.get(&id).await?;
                    encode(&object)
                }
            }
        })
        .await;

    registry
        .register_with_metadata("object.list", CommandMetadata::requires("read"), {
            let objects = objects.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                async move {
                    let entity_type = require(&params, "entity_type")?;
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let limit = optional_u32(&params, "limit")?.unwrap_or(100) as usize;
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
            move |params: Value, _ctx: CommandExecutionCtx| {
                let objects = objects.clone();
                let metadata = metadata.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
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
) {
    register_object_commands(registry, objects, metadata).await;
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
/// `script.create/update/delete` — право `script.manage`; `script.get/list/validate` —
/// `script.read`; `script.execute` — `script.execute`. Права выдаются политикой
/// `platform.scripts` (seed), deny-by-default RBAC, префиксные команды.
pub async fn register_phase13_commands(
    registry: &CommandRegistry,
    scripts: Arc<SurrealScriptRepository>,
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
            let engine = engine.clone();
            move |params: Value, _ctx: CommandExecutionCtx| {
                let engine = engine.clone();
                async move {
                    let source = require(&params, "source")?;
                    engine.validate(&source)?;
                    Ok(json!({ "valid": true }))
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
