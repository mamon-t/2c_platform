//! Идемпотентный сидинг системных метаданных при инициализации платформы.
//!
//! При `system.bootstrap` создаются 7 общеплатформенных типов сущностей
//! (`company_id == ""`, `is_system == true`), которые ложатся в основу
//! SDUI-рендеринга ядровых сущностей (Фаза 15pre-2). Сидинг использует
//! ensure-семантику `MetadataRepository::create_entity_type`: повторный вызов
//! с той же `metadata_version` — no-op, поэтому функция безопасна к повтору.

use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::metadata::{EntityField, EntityState, EntityTransition, EntityType, FieldType};
use core_domain::object::ObjectKind;
use serde_json::json;
use uuid::Uuid;

use crate::ports::{EntitySchema, MetadataRepository};

/// Коды общеплатформенных системных типов сущностей.
pub const SYSTEM_METADATA_TYPES: &[&str] = &[
    "company",
    "user",
    "role",
    "entity_type",
    "entity_field",
    "entity_state",
    "entity_transition",
];

/// Количество системных типов сущностей, сидируемых при бутстрапе.
pub fn system_metadata_type_count() -> usize {
    SYSTEM_METADATA_TYPES.len()
}

fn system_event(entity_type_id: Uuid, event_type: &str) -> Event {
    Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::Metadata,
        stream_id: entity_type_id.to_string(),
        event_type: event_type.to_string(),
        version: 0,
        payload: json!({}),
        metadata: ActorSnapshot::system(),
        company_id: String::new(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    }
}

fn entity_type(code: &str, name: &str, kind: ObjectKind) -> EntityType {
    let now = chrono::Utc::now();
    EntityType {
        id: Uuid::new_v4(),
        code: code.to_string(),
        name: name.to_string(),
        kind,
        company_id: String::new(),
        metadata_version: 1,
        is_system: true,
        created_at: now,
        updated_at: now,
    }
}

fn field(
    entity_type: &str,
    code: &str,
    label: &str,
    data_type: FieldType,
    required: bool,
    order: u32,
) -> EntityField {
    EntityField {
        id: Uuid::new_v4(),
        entity_type: entity_type.to_string(),
        code: code.to_string(),
        label: label.to_string(),
        data_type,
        required,
        is_unique: false,
        is_indexed: false,
        options: json!({}),
        is_system: true,
        order,
    }
}

fn state(entity_type: &str, code: &str, label: &str, is_initial: bool, is_final: bool) -> EntityState {
    EntityState {
        id: Uuid::new_v4(),
        entity_type: entity_type.to_string(),
        code: code.to_string(),
        label: label.to_string(),
        color: None,
        is_initial,
        is_final,
    }
}

fn transition(entity_type: &str, code: &str, label: &str, from_state: &str, to_state: &str) -> EntityTransition {
    EntityTransition {
        id: Uuid::new_v4(),
        entity_type: entity_type.to_string(),
        code: code.to_string(),
        label: label.to_string(),
        from_state: from_state.to_string(),
        to_state: to_state.to_string(),
    }
}

fn company_schema() -> EntitySchema {
    let entity_type = entity_type("company", "Компания", ObjectKind::Catalog);
    EntitySchema {
        entity_type,
        fields: vec![
            field("company", "code", "Код", FieldType::String, true, 1),
            field("company", "name", "Наименование", FieldType::String, true, 2),
            field("company", "is_active", "Активна", FieldType::Boolean, false, 3),
        ],
        states: vec![],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    }
}

fn user_schema() -> EntitySchema {
    let entity_type = entity_type("user", "Пользователь", ObjectKind::Catalog);
    EntitySchema {
        entity_type,
        fields: vec![
            field("user", "login", "Логин", FieldType::String, true, 1),
            field("user", "status", "Статус", FieldType::String, false, 2),
            field("user", "role_ids", "Роли", FieldType::Array, false, 3),
            field("user", "locale", "Локаль", FieldType::String, false, 4),
            field("user", "timezone", "Часовой пояс", FieldType::String, false, 5),
        ],
        states: vec![
            state("user", "active", "Активен", true, false),
            state("user", "blocked", "Заблокирован", false, true),
        ],
        transitions: vec![transition(
            "user",
            "block",
            "Заблокировать",
            "active",
            "blocked",
        )],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    }
}

fn role_schema() -> EntitySchema {
    let entity_type = entity_type("role", "Роль", ObjectKind::Catalog);
    EntitySchema {
        entity_type,
        fields: vec![
            field("role", "code", "Код", FieldType::String, true, 1),
            field("role", "name", "Наименование", FieldType::String, true, 2),
            field("role", "permission_policy_codes", "Политики", FieldType::Array, false, 3),
            field("role", "is_system", "Системная", FieldType::Boolean, false, 4),
        ],
        states: vec![],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    }
}

fn entity_type_schema() -> EntitySchema {
    let entity_type = entity_type("entity_type", "Тип сущности", ObjectKind::Catalog);
    EntitySchema {
        entity_type,
        fields: vec![
            field("entity_type", "code", "Код", FieldType::String, true, 1),
            field("entity_type", "name", "Наименование", FieldType::String, true, 2),
            field("entity_type", "kind", "Вид", FieldType::String, true, 3),
            field("entity_type", "company_id", "Компания", FieldType::String, false, 4),
            field("entity_type", "is_system", "Системный", FieldType::Boolean, false, 5),
        ],
        states: vec![],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    }
}

fn entity_field_schema() -> EntitySchema {
    let entity_type = entity_type("entity_field", "Поле типа", ObjectKind::Catalog);
    EntitySchema {
        entity_type,
        fields: vec![
            field("entity_field", "entity_type", "Тип сущности", FieldType::String, true, 1),
            field("entity_field", "code", "Код", FieldType::String, true, 2),
            field("entity_field", "label", "Метка", FieldType::String, true, 3),
            field("entity_field", "data_type", "Тип данных", FieldType::String, true, 4),
            field("entity_field", "required", "Обязательное", FieldType::Boolean, false, 5),
            field("entity_field", "order", "Порядок", FieldType::Integer, false, 6),
        ],
        states: vec![],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    }
}

fn entity_state_schema() -> EntitySchema {
    let entity_type = entity_type("entity_state", "Состояние типа", ObjectKind::Catalog);
    EntitySchema {
        entity_type,
        fields: vec![
            field("entity_state", "entity_type", "Тип сущности", FieldType::String, true, 1),
            field("entity_state", "code", "Код", FieldType::String, true, 2),
            field("entity_state", "label", "Метка", FieldType::String, true, 3),
            field("entity_state", "is_initial", "Начальное", FieldType::Boolean, false, 4),
            field("entity_state", "is_final", "Конечное", FieldType::Boolean, false, 5),
        ],
        states: vec![],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    }
}

fn entity_transition_schema() -> EntitySchema {
    let entity_type = entity_type("entity_transition", "Переход типа", ObjectKind::Catalog);
    EntitySchema {
        entity_type,
        fields: vec![
            field("entity_transition", "entity_type", "Тип сущности", FieldType::String, true, 1),
            field("entity_transition", "code", "Код", FieldType::String, true, 2),
            field("entity_transition", "from_state", "Из состояния", FieldType::String, false, 3),
            field("entity_transition", "to_state", "В состояние", FieldType::String, false, 4),
        ],
        states: vec![],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    }
}

/// Создаёт декларативные схемы семи системных типов сущностей.
pub fn system_metadata_schemas() -> Vec<EntitySchema> {
    vec![
        company_schema(),
        user_schema(),
        role_schema(),
        entity_type_schema(),
        entity_field_schema(),
        entity_state_schema(),
        entity_transition_schema(),
    ]
}

/// Сидит системные типы метаданных с ensure-семантикой.
///
/// Схемы регистрируются через `MetadataRepository::create_entity_type`
/// с событиями `metadata.seeded`; повторный вызов при той же версии — no-op.
///
/// # Errors
///
/// Возвращает `DomainError::Storage` при сбое сохранения схемы.
pub async fn seed_system_metadata<M>(metadata: &M) -> Result<usize, DomainError>
where
    M: MetadataRepository + ?Sized,
{
    for schema in system_metadata_schemas() {
        let event = system_event(schema.entity_type.id, "metadata.seeded");
        metadata.create_entity_type(&schema, &[event]).await?;
    }
    Ok(system_metadata_type_count())
}