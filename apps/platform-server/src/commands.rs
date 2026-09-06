//! Command layer for Phase 2: companies, users, roles.
//!
//! Each mutating command advances both the Pipe (events) and the Board
//! (materialized collection) atomically through the repository, writing the
//! audit snapshot with the system actor until authentication exists.

use chrono::{NaiveDate, Utc};
use core_application::ports::{
    CompanyRepository, EntitySchema, MetadataRepository, ObjectRepository, RoleRepository,
    UserRepository,
};
use core_application::CommandRegistry;
use core_domain::company::Company;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::metadata::{
    EntityAction, EntityField, EntityForm, EntityKind, EntityRelation, EntityState, EntityTransition,
    EntityType, FieldType, OnDelete, RelationKind,
};
use core_domain::object::{Object, ObjectKind};
use core_domain::role::Role;
use core_domain::user::{
    ContactChannelType, ContactPurpose, Person, User, UserCompanyProfile, UserContact, UserStatus,
};
use core_infrastructure::{
    SurrealCompanyRepository, SurrealMetadataRepository, SurrealObjectRepository,
    SurrealRoleRepository, SurrealUserRepository,
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

fn encode<T: serde::Serialize>(value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("сериализация: {e}"))
}

fn require(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("отсутствует обязательный параметр '{key}'"))
}

fn optional(value: &Value, key: &str) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| format!("параметр '{key}' должен быть строкой")),
    }
}

fn optional_bool(value: &Value, key: &str) -> Result<Option<bool>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("параметр '{key}' должен быть булевым")),
    }
}

fn parse_enum<T: serde::de::DeserializeOwned>(value: &Value, key: &str) -> Result<T, String> {
    let raw = value
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("отсутствует обязательный параметр '{key}'"))?;
    serde_json::from_value(Value::String(raw.to_string()))
        .map_err(|e| format!("некорректное значение '{key}': {e}"))
}

fn parse_uuids(value: &Value, key: &str) -> Result<Vec<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|i| {
                i.as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("параметр '{key}' должен содержать строки"))
            })
            .collect(),
        Some(_) => Err(format!("параметр '{key}' должен быть массивом")),
    }
}

fn parse_purposes(value: &Value) -> Result<Vec<ContactPurpose>, String> {
    parse_uuids(value, "purposes").and_then(|names| name_list_to_purposes(&names))
}

fn name_list_to_purposes(names: &[String]) -> Result<Vec<ContactPurpose>, String> {
    names
        .iter()
        .map(|name| {
            serde_json::from_value(Value::String(name.clone()))
                .map_err(|e| format!("некорректное назначение контакта: {e}"))
        })
        .collect()
}

async fn register_company_commands(
    registry: &CommandRegistry,
    companies: Arc<SurrealCompanyRepository>,
) {
    registry
        .register("company.create", {
            let companies = companies.clone();
            move |params: Value| {
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
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&company)
                }
            }
        })
        .await;

    registry
        .register("company.get", {
            let companies = companies.clone();
            move |params: Value| {
                let companies = companies.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let company = companies.get(&id).await.map_err(|e| e.to_string())?;
                    encode(&company)
                }
            }
        })
        .await;

    registry
        .register("company.list", {
            let companies = companies.clone();
            move |_params: Value| {
                let companies = companies.clone();
                async move {
                    let list = companies.list().await.map_err(|e| e.to_string())?;
                    let rows: Result<Vec<Value>, String> =
                        list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register("company.update", {
            let companies = companies.clone();
            move |params: Value| {
                let companies = companies.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let existing = companies.get(&id).await.map_err(|e| e.to_string())?;
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
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&company)
                }
            }
        })
        .await;
}

async fn register_user_commands(registry: &CommandRegistry, users: Arc<SurrealUserRepository>) {
    registry
        .register("user.create", {
            let users = users.clone();
            move |params: Value| {
                let users = users.clone();
                async move {
                    let login = require(&params, "login")?;
                    let password_hash = require(&params, "password_hash")?;
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
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(json!({
                        "user": encode(&user)?,
                        "person": encode(&person)?,
                    }))
                }
            }
        })
        .await;

    registry
        .register("user.get", {
            let users = users.clone();
            move |params: Value| {
                let users = users.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let user = users.get(&id).await.map_err(|e| e.to_string())?;
                    encode(&user)
                }
            }
        })
        .await;

    registry
        .register("user.list", {
            let users = users.clone();
            move |_params: Value| {
                let users = users.clone();
                async move {
                    let list = users.list().await.map_err(|e| e.to_string())?;
                    let rows: Result<Vec<Value>, String> = list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register("user.update", {
            let users = users.clone();
            move |params: Value| {
                let users = users.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let existing = users.get(&id).await.map_err(|e| e.to_string())?;
                    let user = User {
                        id,
                        login: optional(&params, "login")?.unwrap_or(existing.login),
                        password_hash: optional(&params, "password_hash")?
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
                    users.update(&user, &[event]).await.map_err(|e| e.to_string())?;
                    encode(&user)
                }
            }
        })
        .await;

    registry
        .register("user.contact.add", {
            let users = users.clone();
            move |params: Value| {
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
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&contact)
                }
            }
        })
        .await;

    registry
        .register("user.profile.add", {
            let users = users.clone();
            move |params: Value| {
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
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&profile)
                }
            }
        })
        .await;
}

async fn register_role_commands(registry: &CommandRegistry, roles: Arc<SurrealRoleRepository>) {
    registry
        .register("role.create", {
            let roles = roles.clone();
            move |params: Value| {
                let roles = roles.clone();
                async move {
                    let code = require(&params, "code")?;
                    let name = require(&params, "name")?;
                    let description = optional(&params, "description")?.unwrap_or_default();
                    let is_system = optional_bool(&params, "is_system")?.unwrap_or(false);
                    let now = Utc::now();
                    let role = Role {
                        id: Uuid::new_v4(),
                        code,
                        name,
                        description,
                        is_system,
                        created_at: now,
                        updated_at: now,
                    };
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let event = system_event(
                        StreamType::Role,
                        role.id.to_string(),
                        "role.created",
                        &company_id,
                        encode(&role)?,
                    );
                    roles.create(&role, &[event]).await.map_err(|e| e.to_string())?;
                    encode(&role)
                }
            }
        })
        .await;

    registry
        .register("role.get", {
            let roles = roles.clone();
            move |params: Value| {
                let roles = roles.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let role = roles.get(&id).await.map_err(|e| e.to_string())?;
                    encode(&role)
                }
            }
        })
        .await;

    registry
        .register("role.list", {
            let roles = roles.clone();
            move |_params: Value| {
                let roles = roles.clone();
                async move {
                    let list = roles.list().await.map_err(|e| e.to_string())?;
                    let rows: Result<Vec<Value>, String> = list.iter().map(encode).collect();
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
        .register("metadata.entity_type.create", {
            let metadata = metadata.clone();
            move |params: Value| {
                let metadata = metadata.clone();
                async move {
                    let schema = parse_schema(&params)?;
                    let event = metadata_event(&schema, "metadata.entity_type.created");
                    metadata
                        .create_entity_type(&schema, &[event])
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&schema)
                }
            }
        })
        .await;

    registry
        .register("metadata.entity_type.get", {
            let metadata = metadata.clone();
            move |params: Value| {
                let metadata = metadata.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let entity_type = metadata
                        .get_entity_type(&id)
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&entity_type)
                }
            }
        })
        .await;

    registry
        .register("metadata.entity_type.get_by_code", {
            let metadata = metadata.clone();
            move |params: Value| {
                let metadata = metadata.clone();
                async move {
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let code = require(&params, "code")?;
                    let entity_type = metadata
                        .get_entity_type_by_code(&company_id, &code)
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&entity_type)
                }
            }
        })
        .await;

    registry
        .register("metadata.entity_type.list", {
            let metadata = metadata.clone();
            move |_params: Value| {
                let metadata = metadata.clone();
                async move {
                    let list = metadata.list_entity_types().await.map_err(|e| e.to_string())?;
                    let rows: Result<Vec<Value>, String> = list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register("metadata.entity_type.update", {
            let metadata = metadata.clone();
            move |params: Value| {
                let metadata = metadata.clone();
                async move {
                    let mut schema = parse_schema(&params)?;
                    let company_id = schema.entity_type.company_id.clone();
                    let existing = metadata
                        .get_entity_type_by_code(&company_id, &schema.entity_type.code)
                        .await
                        .map_err(|e| e.to_string())?;
                    schema.entity_type.id = existing.id;
                    schema.entity_type.created_at = existing.created_at;
                    schema.entity_type.updated_at = Utc::now();
                    let event = metadata_event(&schema, "metadata.entity_type.updated");
                    metadata
                        .update_entity_type(&schema, &[event])
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&schema)
                }
            }
        })
        .await;

    registry
        .register("metadata.schema.get", {
            let metadata = metadata.clone();
            move |params: Value| {
                let metadata = metadata.clone();
                async move {
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let code = require(&params, "code")?;
                    let schema = metadata
                        .get_schema(&company_id, &code)
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&schema)
                }
            }
        })
        .await;
}

/// Registers the Phase 4 command set: universal objects, CRUD with OCC,
/// version snapshots and document numbering. Commands validate data against
/// the metatype model (`Object::validate`) and append object events to the
/// Pipe together with the Board write.
async fn register_object_commands(
    registry: &CommandRegistry,
    objects: Arc<SurrealObjectRepository>,
    metadata: Arc<SurrealMetadataRepository>,
) {
    registry
        .register("object.create", {
            let objects = objects.clone();
            let metadata = metadata.clone();
            move |params: Value| {
                let objects = objects.clone();
                let metadata = metadata.clone();
                async move {
                    let entity_type = require(&params, "entity_type")?;
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let schema = metadata
                        .get_schema(&company_id, &entity_type)
                        .await
                        .map_err(|e| e.to_string())?;
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
                        .validate(&schema.fields, &schema.states)
                        .map_err(|e| e.to_string())?;
                    let event = system_event(
                        StreamType::Object,
                        object.id.to_string(),
                        "object.created",
                        &company_id,
                        encode(&object)?,
                    );
                    let stored = objects
                        .create(&object, &[event])
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&stored)
                }
            }
        })
        .await;

    registry
        .register("object.get", {
            let objects = objects.clone();
            move |params: Value| {
                let objects = objects.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let object = objects.get(&id).await.map_err(|e| e.to_string())?;
                    encode(&object)
                }
            }
        })
        .await;

    registry
        .register("object.list", {
            let objects = objects.clone();
            move |params: Value| {
                let objects = objects.clone();
                async move {
                    let entity_type = require(&params, "entity_type")?;
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let limit = optional_u32(&params, "limit")?.unwrap_or(100) as usize;
                    let list = objects
                        .list(&entity_type, &company_id, limit)
                        .await
                        .map_err(|e| e.to_string())?;
                    let rows: Result<Vec<Value>, String> = list.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register("object.update", {
            let objects = objects.clone();
            let metadata = metadata.clone();
            move |params: Value| {
                let objects = objects.clone();
                let metadata = metadata.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let expected_version = parse_u64(&params, "expected_version")?;
                    let existing = objects.get(&id).await.map_err(|e| e.to_string())?;
                    let schema = metadata
                        .get_schema(&existing.company_id, &existing.entity_type)
                        .await
                        .map_err(|e| e.to_string())?;
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
                        .validate(&schema.fields, &schema.states)
                        .map_err(|e| e.to_string())?;
                    let event = system_event(
                        StreamType::Object,
                        id.to_string(),
                        "object.updated",
                        &existing.company_id,
                        encode(&updated)?,
                    );
                    let stored = objects
                        .update(&updated, &[event])
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&stored)
                }
            }
        })
        .await;

    registry
        .register("object.delete", {
            let objects = objects.clone();
            move |params: Value| {
                let objects = objects.clone();
                async move {
                    let id = parse_uuid(&params, "id")?;
                    let existing = objects.get(&id).await.map_err(|e| e.to_string())?;
                    let event = system_event(
                        StreamType::Object,
                        id.to_string(),
                        "object.deleted",
                        &existing.company_id,
                        json!({}),
                    );
                    objects
                        .delete(&id, &[event])
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(json!({ "deleted": true, "id": id.to_string() }))
                }
            }
        })
        .await;

    registry
        .register("object.snapshot.list", {
            let objects = objects.clone();
            move |params: Value| {
                let objects = objects.clone();
                async move {
                    let id = parse_uuid(&params, "object_id")?;
                    let snapshots = objects
                        .get_snapshots(&id)
                        .await
                        .map_err(|e| e.to_string())?;
                    let rows: Result<Vec<Value>, String> = snapshots.iter().map(encode).collect();
                    Ok(Value::Array(rows?))
                }
            }
        })
        .await;

    registry
        .register("object.snapshot.restore", {
            let objects = objects.clone();
            move |params: Value| {
                let objects = objects.clone();
                async move {
                    let id = parse_uuid(&params, "object_id")?;
                    let version = parse_u64(&params, "version")?;
                    let existing = objects.get(&id).await.map_err(|e| e.to_string())?;
                    let event = system_event(
                        StreamType::Object,
                        id.to_string(),
                        "object.restored",
                        &existing.company_id,
                        json!({ "version": version }),
                    );
                    let stored = objects
                        .restore_snapshot(&id, version, &[event])
                        .await
                        .map_err(|e| e.to_string())?;
                    encode(&stored)
                }
            }
        })
        .await;

    registry
        .register("document.number.next", {
            let objects = objects.clone();
            move |params: Value| {
                let objects = objects.clone();
                async move {
                    let entity_type = require(&params, "entity_type")?;
                    let company_id = optional(&params, "company_id")?.unwrap_or_default();
                    let number = objects
                        .next_document_number(&entity_type, &company_id)
                        .await
                        .map_err(|e| e.to_string())?;
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

fn parse_schema(value: &Value) -> Result<EntitySchema, String> {
    let entity_type_value = value
        .get("entity_type")
        .ok_or_else(|| "отсутствует обязательный параметр 'entity_type'".to_string())?;
    let id = match entity_type_value.get("id") {
        Some(Value::String(raw)) => Uuid::parse_str(raw)
            .map_err(|e| format!("некорректный UUID 'entity_type.id': {e}"))?,
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

fn parse_fields(value: &Value, entity_type_code: &str) -> Result<Vec<EntityField>, String> {
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

fn parse_states(value: &Value, entity_type_code: &str) -> Result<Vec<EntityState>, String> {
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

fn parse_transitions(value: &Value, entity_type_code: &str) -> Result<Vec<EntityTransition>, String> {
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

fn parse_forms(value: &Value, entity_type_code: &str) -> Result<Vec<EntityForm>, String> {
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

fn parse_actions(value: &Value, entity_type_code: &str) -> Result<Vec<EntityAction>, String> {
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

fn parse_relations(value: &Value, entity_type_code: &str) -> Result<Vec<EntityRelation>, String> {
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

fn items(value: &Value, key: &str) -> Result<Vec<Value>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(_) => Err(format!("параметр '{key}' должен быть массивом")),
    }
}

fn parse_u32(value: &Value, key: &str) -> Result<u32, String> {
    value
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| format!("отсутствует обязательный параметр '{key}'"))
}

fn parse_u64(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| format!("отсутствует обязательный параметр '{key}'"))
}

fn optional_u32(value: &Value, key: &str) -> Result<Option<u32>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(|v| Some(v as u32))
            .ok_or_else(|| format!("параметр '{key}' должен быть целым числом")),
    }
}

/// Registers the Phase 3 command set into the shared registry.
pub async fn register_phase3_commands(
    registry: &CommandRegistry,
    metadata: Arc<SurrealMetadataRepository>,
) {
    register_metadata_commands(registry, metadata).await;
}

/// Registers the Phase 4 command set into the shared registry.
pub async fn register_phase4_commands(
    registry: &CommandRegistry,
    objects: Arc<SurrealObjectRepository>,
    metadata: Arc<SurrealMetadataRepository>,
) {
    register_object_commands(registry, objects, metadata).await;
}

/// Registers the Phase 2 command set into the shared registry.
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

fn parse_uuid(value: &Value, key: &str) -> Result<Uuid, String> {
    let raw = require(value, key)?;
    Uuid::parse_str(&raw).map_err(|e| format!("некорректный UUID '{key}': {e}"))
}

fn parse_optional_uuid(value: &Value, key: &str) -> Result<Option<Uuid>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => parse_uuid(value, key).map(Some),
    }
}

fn parse_optional_enum<T: serde::de::DeserializeOwned>(
    value: &Value,
    key: &str,
) -> Result<Option<T>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let raw = v
                .as_str()
                .ok_or_else(|| format!("параметр '{key}' должен быть строкой"))?;
            serde_json::from_value(Value::String(raw.to_string()))
                .map(Some)
                .map_err(|e| format!("некорректное значение '{key}': {e}"))
        }
    }
}

fn parse_optional_date(value: &Value, key: &str) -> Result<Option<NaiveDate>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let raw = v
                .as_str()
                .ok_or_else(|| format!("параметр '{key}' должен быть строкой даты"))?;
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .map(Some)
                .map_err(|e| format!("некорректная дата '{key}': {e}"))
        }
    }
}