//! Хранилище метаданных сущностей на базе SurrealDB (проекция «Доска» для
//! метатиповой модели: entity_types, entity_fields, entity_states,
//! entity_transitions, entity_forms, entity_actions, entity_relations).

use core_application::ports::{BoxFuture, EntitySchema, MetadataRepository};
use core_domain::error::DomainError;
use core_domain::event::Event;
use core_domain::metadata::EntityType;
use serde_json::{json, Value};
use std::collections::HashSet;
use surrealdb::engine::any::Any;
use surrealdb::method::Transaction;
use surrealdb::Surreal;
use uuid::Uuid;

use crate::events::{assign_versions, with_transaction, write_events};

const ENTITY_TYPE_TABLE: &str = "entity_types";
const ENTITY_FIELD_TABLE: &str = "entity_fields";
const ENTITY_STATE_TABLE: &str = "entity_states";
const ENTITY_TRANSITION_TABLE: &str = "entity_transitions";
const ENTITY_FORM_TABLE: &str = "entity_forms";
const ENTITY_ACTION_TABLE: &str = "entity_actions";
const ENTITY_RELATION_TABLE: &str = "entity_relations";

/// Фиксирует проекцию «Доска» метатиповой модели. Типы сущностей
/// уникальны по `(code, company_id)`; все дочерние ресурсы ключуются по
/// `(entity_type, code)` для идемпотентной ensure-семантики.
#[derive(Clone)]
pub struct SurrealMetadataRepository {
    db: Surreal<Any>,
}

impl SurrealMetadataRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт семь таблиц метаданных и индексы идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS entity_types SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_entity_types_code_company \
                ON entity_types FIELDS code, company_id UNIQUE",
            "DEFINE TABLE IF NOT EXISTS entity_fields SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_entity_fields_type_code \
                ON entity_fields FIELDS entity_type, code UNIQUE",
            "DEFINE TABLE IF NOT EXISTS entity_states SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_entity_states_type_code \
                ON entity_states FIELDS entity_type, code UNIQUE",
            "DEFINE TABLE IF NOT EXISTS entity_transitions SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_entity_transitions_type \
                ON entity_transitions FIELDS entity_type",
            "DEFINE TABLE IF NOT EXISTS entity_forms SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_entity_forms_type_code \
                ON entity_forms FIELDS entity_type, code UNIQUE",
            "DEFINE TABLE IF NOT EXISTS entity_actions SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_entity_actions_type_code \
                ON entity_actions FIELDS entity_type, code UNIQUE",
            "DEFINE TABLE IF NOT EXISTS entity_relations SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_entity_relations_type_code \
                ON entity_relations FIELDS entity_type, code UNIQUE",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("metadata ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("metadata ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

const ENTITY_TYPE_FIELDS: &str = "record::id(id) AS id, code, name, kind, company_id, \
    metadata_version, is_system, created_at, updated_at";
const ENTITY_FIELD_FIELDS: &str = "record::id(id) AS id, entity_type, code, label, data_type, \
    required, is_unique, is_indexed, options, is_system, order";
const ENTITY_STATE_FIELDS: &str = "record::id(id) AS id, entity_type, code, label, color, \
    is_initial, is_final";
const ENTITY_TRANSITION_FIELDS: &str = "record::id(id) AS id, entity_type, code, label, \
    from_state, to_state";
const ENTITY_FORM_FIELDS: &str = "record::id(id) AS id, entity_type, code, label, layout";
const ENTITY_ACTION_FIELDS: &str = "record::id(id) AS id, entity_type, code, label, handler";
const ENTITY_RELATION_FIELDS: &str = "record::id(id) AS id, entity_type, code, target_type, \
    kind, on_delete";

fn decode_row<T: serde::de::DeserializeOwned>(
    kind: &str,
    row: Option<Value>,
) -> Result<T, DomainError> {
    row.map(serde_json::from_value)
        .transpose()
        .map_err(|e| DomainError::Storage(format!("{kind} decode: {e}")))?
        .ok_or_else(|| DomainError::NotFound(kind.to_string()))
}

fn decode_rows<T: serde::de::DeserializeOwned>(
    kind: &str,
    rows: Vec<Value>,
) -> Result<Vec<T>, DomainError> {
    serde_json::from_value(Value::Array(rows))
        .map_err(|e| DomainError::Storage(format!("{kind} list decode: {e}")))
}

/// Делает upsert дочернего ресурса, переиспользуя существующий
/// идентификатор записи, чтобы повторная регистрация по коду была
/// идемпотентной и никогда не создавала дублей.
async fn upsert_child(
    txn: &Transaction<Any>,
    table: &str,
    entity_type: &str,
    code: &str,
    record: &Value,
) -> Result<(), DomainError> {
    let mut response = txn
        .query(format!(
            "SELECT record::id(id) AS id FROM {table} \
             WHERE entity_type = $et AND code = $code LIMIT 1"
        ))
        .bind(("et", entity_type.to_string()))
        .bind(("code", code.to_string()))
        .await
        .map_err(|e| DomainError::Storage(format!("{table} lookup id: {e}")))?;
    let existing: Option<Value> = response
        .take(0)
        .map_err(|e| DomainError::Storage(format!("{table} lookup id take: {e}")))?;

    match existing {
        Some(row) => {
            let existing_id = row
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| DomainError::Storage(format!("{table} bad id: {row}")))?;
            let mut merged = record.clone();
            if let Some(obj) = merged.as_object_mut() {
                obj.insert("id".to_string(), json!(existing_id));
            }
            let _: Option<Value> = txn
                .upsert((table, existing_id))
                .content(merged)
                .await
                .map_err(|e| DomainError::Storage(format!("{table} write: {e}")))?;
        }
        None => {
            let new_id = record
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| DomainError::Storage(format!("{table} missing id")))?;
            let _: Option<Value> = txn
                .upsert((table, new_id))
                .content(record.clone())
                .await
                .map_err(|e| DomainError::Storage(format!("{table} write: {e}")))?;
        }
    }
    Ok(())
}

/// Проверяет, что каждая конечная точка перехода существует среди
/// переданных или уже сохранённых состояний типа сущности.
async fn validate_transitions(
    txn: &Transaction<Any>,
    schema: &EntitySchema,
) -> Result<(), DomainError> {
    let mut state_codes: HashSet<String> = schema.states.iter().map(|s| s.code.clone()).collect();
    let mut response = txn
        .query(format!(
            "SELECT code FROM {ENTITY_STATE_TABLE} WHERE entity_type = $et"
        ))
        .bind(("et", schema.entity_type.code.clone()))
        .await
        .map_err(|e| DomainError::Storage(format!("entity_states read: {e}")))?;
    let rows: Vec<Value> = response
        .take(0)
        .map_err(|e| DomainError::Storage(format!("entity_states read take: {e}")))?;
    for row in rows {
        if let Some(code) = row.get("code").and_then(|v| v.as_str()) {
            state_codes.insert(code.to_string());
        }
    }

    for transition in &schema.transitions {
        if !state_codes.contains(&transition.from_state) || !state_codes.contains(&transition.to_state)
        {
            return Err(DomainError::ValidationError(format!(
                "Переход {}: состояние '{}' или '{}' не существует у типа '{}'",
                transition.code, transition.from_state, transition.to_state, schema.entity_type.code
            )));
        }
    }
    Ok(())
}

/// Реализует контракт ensure-семантики: возвращает `Ok(false)`, когда
/// сохранённая `metadata_version` не старше переданной (no-op), и `Ok(true)`
/// после применения схемы.
///   `require_existing` — если равно true, тип сущности уже должен существовать.
async fn apply_schema(
    txn: &Transaction<Any>,
    schema: &EntitySchema,
    require_existing: bool,
) -> Result<bool, DomainError> {
    let entity_type_value = serde_json::to_value(&schema.entity_type)
        .map_err(|e| DomainError::Storage(format!("entity_type encode: {e}")))?;

    let owned_txn = txn;
    let mut response = owned_txn
        .query(format!(
            "SELECT record::id(id) AS id, metadata_version FROM {ENTITY_TYPE_TABLE} \
             WHERE code = $code AND company_id = $company_id LIMIT 1"
        ))
        .bind(("code", schema.entity_type.code.clone()))
        .bind(("company_id", schema.entity_type.company_id.clone()))
        .await
        .map_err(|e| DomainError::Storage(format!("entity_types lookup: {e}")))?;
    let existing: Option<Value> = response
        .take(0)
        .map_err(|e| DomainError::Storage(format!("entity_types lookup take: {e}")))?;

    match existing {
        Some(row) => {
            let stored_version = row
                .get("metadata_version")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            if stored_version >= schema.entity_type.metadata_version {
                return Ok(false);
            }
            let existing_id = row
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| DomainError::Storage(format!("entity_types bad id: {row}")))?;
            let mut merged = entity_type_value;
            if let Some(obj) = merged.as_object_mut() {
                obj.insert("id".to_string(), json!(existing_id));
            }
            let _: Option<Value> = owned_txn
                .upsert((ENTITY_TYPE_TABLE, existing_id))
                .content(merged)
                .await
                .map_err(|e| DomainError::Storage(format!("entity_types write: {e}")))?;
        }
        None => {
            if require_existing {
                return Err(DomainError::NotFound(format!(
                    "Тип сущности {} не найден",
                    schema.entity_type.code
                )));
            }
            let _: Option<Value> = owned_txn
                .upsert((
                    ENTITY_TYPE_TABLE,
                    schema.entity_type.id.to_string(),
                ))
                .content(entity_type_value)
                .await
                .map_err(|e| DomainError::Storage(format!("entity_types write: {e}")))?;
        }
    }

    validate_transitions(owned_txn, schema).await?;

    for field in &schema.fields {
        let value = serde_json::to_value(field)
            .map_err(|e| DomainError::Storage(format!("entity_field encode: {e}")))?;
        upsert_child(
            owned_txn,
            ENTITY_FIELD_TABLE,
            &schema.entity_type.code,
            &field.code,
            &value,
        )
        .await?;
    }
    for state in &schema.states {
        let value = serde_json::to_value(state)
            .map_err(|e| DomainError::Storage(format!("entity_state encode: {e}")))?;
        upsert_child(
            owned_txn,
            ENTITY_STATE_TABLE,
            &schema.entity_type.code,
            &state.code,
            &value,
        )
        .await?;
    }
    for transition in &schema.transitions {
        let value = serde_json::to_value(transition)
            .map_err(|e| DomainError::Storage(format!("entity_transition encode: {e}")))?;
        upsert_child(
            owned_txn,
            ENTITY_TRANSITION_TABLE,
            &schema.entity_type.code,
            &transition.code,
            &value,
        )
        .await?;
    }
    for form in &schema.forms {
        let value = serde_json::to_value(form)
            .map_err(|e| DomainError::Storage(format!("entity_form encode: {e}")))?;
        upsert_child(
            owned_txn,
            ENTITY_FORM_TABLE,
            &schema.entity_type.code,
            &form.code,
            &value,
        )
        .await?;
    }
    for action in &schema.actions {
        let value = serde_json::to_value(action)
            .map_err(|e| DomainError::Storage(format!("entity_action encode: {e}")))?;
        upsert_child(
            owned_txn,
            ENTITY_ACTION_TABLE,
            &schema.entity_type.code,
            &action.code,
            &value,
        )
        .await?;
    }
    for relation in &schema.relations {
        let value = serde_json::to_value(relation)
            .map_err(|e| DomainError::Storage(format!("entity_relation encode: {e}")))?;
        upsert_child(
            owned_txn,
            ENTITY_RELATION_TABLE,
            &schema.entity_type.code,
            &relation.code,
            &value,
        )
        .await?;
    }
    Ok(true)
}

impl MetadataRepository for SurrealMetadataRepository {
    fn create_entity_type(
        &self,
        schema: &EntitySchema,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>> {
        let db = self.db.clone();
        let schema = schema.clone();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let schema = schema.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        if apply_schema(&txn, &schema, false).await? {
                            let mut events = events.clone();
                            assign_versions(&txn, &mut events).await?;
                            write_events(&txn, &events).await?;
                        }
                        Ok(())
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn get_entity_type(&self, id: &Uuid) -> BoxFuture<'_, Result<EntityType, DomainError>> {
        let db = self.db.clone();
        let id = *id;
        Box::pin(async move {
            let mut response = db
                .query(format!(
                    "SELECT {ENTITY_TYPE_FIELDS} FROM {ENTITY_TYPE_TABLE} \
                     WHERE record::id(id) = $id LIMIT 1"
                ))
                .bind(("id", id.to_string()))
                .await
                .map_err(|e| DomainError::Storage(format!("entity_types get: {e}")))?;
            let row: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("entity_types get take: {e}")))?;
            decode_row("Тип сущности", row)
        })
    }

    fn get_entity_type_by_code(
        &self,
        company_id: &str,
        code: &str,
    ) -> BoxFuture<'_, Result<EntityType, DomainError>> {
        let db = self.db.clone();
        let company_id = company_id.to_string();
        let code = code.to_string();
        Box::pin(async move {
            let find = |target: &str| {
                let db = db.clone();
                let code = code.clone();
                let target = target.to_string();
                Box::pin(async move {
                    let mut response = db
                        .query(format!(
                            "SELECT {ENTITY_TYPE_FIELDS} FROM {ENTITY_TYPE_TABLE} \
                             WHERE code = $code AND company_id = $company_id LIMIT 1"
                        ))
                        .bind(("code", code))
                        .bind(("company_id", target))
                        .await
                        .map_err(|e| DomainError::Storage(format!("entity_types get by code: {e}")))?;
                    let row: Option<Value> = response
                        .take(0)
                        .map_err(|e| DomainError::Storage(format!("entity_types get by code take: {e}")))?;
                    decode_row::<EntityType>("Тип сущности", row)
                })
            };
            match find(&company_id).await {
                found @ Ok(_) => found,
                // Глобальные системные типы (компания-владелец ""). Один
                // и тот же код в разных компаниях не конфликтует, поэтому
                // фолбэк, только когда точного совпадения нет, и только на
                // is_system-типы: кастомный тип с company_id="" не должен
                // «протекать» во все компании.
                Err(DomainError::NotFound(_)) if !company_id.is_empty() => match find("").await {
                    Ok(et) if et.is_system => Ok(et),
                    Ok(_) => Err(DomainError::NotFound(format!(
                        "Тип сущности {code} не найден в компании {company_id}"
                    ))),
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            }
        })
    }

    fn list_entity_types(&self) -> BoxFuture<'_, Result<Vec<EntityType>, DomainError>> {
        let db = self.db.clone();
        Box::pin(async move {
            let mut response = db
                .query(format!(
                    "SELECT {ENTITY_TYPE_FIELDS} FROM {ENTITY_TYPE_TABLE} ORDER BY code"
                ))
                .await
                .map_err(|e| DomainError::Storage(format!("entity_types list: {e}")))?;
            let rows: Vec<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("entity_types list take: {e}")))?;
            decode_rows("entity_types", rows)
        })
    }

    fn update_entity_type(
        &self,
        schema: &EntitySchema,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>> {
        let db = self.db.clone();
        let schema = schema.clone();
        let events = events.to_vec();
        Box::pin(async move {
            with_transaction(&db, |txn| {
                let schema = schema.clone();
                let events = events.clone();
                async move {
                    let outcome: Result<(), DomainError> = async {
                        if apply_schema(&txn, &schema, true).await? {
                            let mut events = events.clone();
                            assign_versions(&txn, &mut events).await?;
                            write_events(&txn, &events).await?;
                        }
                        Ok(())
                    }
                    .await;
                    (txn, outcome)
                }
            })
            .await
        })
    }

    fn get_schema(
        &self,
        company_id: &str,
        entity_type_code: &str,
    ) -> BoxFuture<'_, Result<EntitySchema, DomainError>> {
        let db = self.db.clone();
        let company_id = company_id.to_string();
        let entity_type_code = entity_type_code.to_string();
        Box::pin(async move {
            let repo = SurrealMetadataRepository::new(db.clone());
            let entity_type = repo
                .get_entity_type_by_code(&company_id, &entity_type_code)
                .await?;

            let mut fields_response = db
                .query(format!(
                    "SELECT {ENTITY_FIELD_FIELDS} FROM {ENTITY_FIELD_TABLE} \
                     WHERE entity_type = $et ORDER BY order"
                ))
                .bind(("et", entity_type_code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("entity_fields read: {e}")))?;
            let fields: Vec<Value> = fields_response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("entity_fields read take: {e}")))?;

            let mut states_response = db
                .query(format!(
                    "SELECT {ENTITY_STATE_FIELDS} FROM {ENTITY_STATE_TABLE} \
                     WHERE entity_type = $et ORDER BY code"
                ))
                .bind(("et", entity_type_code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("entity_states read: {e}")))?;
            let states: Vec<Value> = states_response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("entity_states read take: {e}")))?;

            let mut transitions_response = db
                .query(format!(
                    "SELECT {ENTITY_TRANSITION_FIELDS} FROM {ENTITY_TRANSITION_TABLE} \
                     WHERE entity_type = $et ORDER BY code"
                ))
                .bind(("et", entity_type_code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("entity_transitions read: {e}")))?;
            let transitions: Vec<Value> = transitions_response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("entity_transitions read take: {e}")))?;

            let mut forms_response = db
                .query(format!(
                    "SELECT {ENTITY_FORM_FIELDS} FROM {ENTITY_FORM_TABLE} \
                     WHERE entity_type = $et ORDER BY code"
                ))
                .bind(("et", entity_type_code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("entity_forms read: {e}")))?;
            let forms: Vec<Value> = forms_response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("entity_forms read take: {e}")))?;

            let mut actions_response = db
                .query(format!(
                    "SELECT {ENTITY_ACTION_FIELDS} FROM {ENTITY_ACTION_TABLE} \
                     WHERE entity_type = $et ORDER BY code"
                ))
                .bind(("et", entity_type_code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("entity_actions read: {e}")))?;
            let actions: Vec<Value> = actions_response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("entity_actions read take: {e}")))?;

            let mut relations_response = db
                .query(format!(
                    "SELECT {ENTITY_RELATION_FIELDS} FROM {ENTITY_RELATION_TABLE} \
                     WHERE entity_type = $et ORDER BY code"
                ))
                .bind(("et", entity_type_code.clone()))
                .await
                .map_err(|e| DomainError::Storage(format!("entity_relations read: {e}")))?;
            let relations: Vec<Value> = relations_response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("entity_relations read take: {e}")))?;

            Ok(EntitySchema {
                entity_type,
                fields: decode_rows("entity_fields", fields)?,
                states: decode_rows("entity_states", states)?,
                transitions: decode_rows("entity_transitions", transitions)?,
                forms: decode_rows("entity_forms", forms)?,
                actions: decode_rows("entity_actions", actions)?,
                relations: decode_rows("entity_relations", relations)?,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_application::ports::EventStore;
    use core_domain::event::{ActorSnapshot, StreamType};
    use core_domain::metadata::{
        EntityAction, EntityField, EntityForm, EntityRelation, EntityState, EntityTransition,
        FieldType, OnDelete, RelationKind,
    };
    use chrono::Utc;

    async fn mem_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        db
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
            correlation_id: "corr".to_string(),
            causation_id: None,
            occurred_at: Utc::now(),
        }
    }

    fn sample_schema(metadata_version: u32) -> EntitySchema {
        let id = Uuid::new_v4();
        let now = Utc::now();
        EntitySchema {
            entity_type: EntityType {
                id,
                code: "invoice".to_string(),
                name: "Счёт".to_string(),
                kind: core_domain::metadata::EntityKind::Document,
                company_id: String::new(),
                metadata_version,
                is_system: false,
                created_at: now,
                updated_at: now,
            },
            fields: vec![EntityField {
                id: Uuid::new_v4(),
                entity_type: "invoice".to_string(),
                code: "sum".to_string(),
                label: "Сумма".to_string(),
                data_type: FieldType::Money,
                required: true,
                is_unique: false,
                is_indexed: true,
                options: json!({}),
                is_system: false,
                order: 1,
            }],
            states: vec![
                EntityState {
                    id: Uuid::new_v4(),
                    entity_type: "invoice".to_string(),
                    code: "draft".to_string(),
                    label: "Черновик".to_string(),
                    color: None,
                    is_initial: true,
                    is_final: false,
                },
                EntityState {
                    id: Uuid::new_v4(),
                    entity_type: "invoice".to_string(),
                    code: "posted".to_string(),
                    label: "Проведён".to_string(),
                    color: None,
                    is_initial: false,
                    is_final: true,
                },
            ],
            transitions: vec![EntityTransition {
                id: Uuid::new_v4(),
                entity_type: "invoice".to_string(),
                code: "post".to_string(),
                label: "Провести".to_string(),
                from_state: "draft".to_string(),
                to_state: "posted".to_string(),
            }],
            forms: vec![EntityForm {
                id: Uuid::new_v4(),
                entity_type: "invoice".to_string(),
                code: "main".to_string(),
                label: "Основная форма".to_string(),
                layout: json!({"sections": []}),
            }],
            actions: vec![EntityAction {
                id: Uuid::new_v4(),
                entity_type: "invoice".to_string(),
                code: "print".to_string(),
                label: "Печать".to_string(),
                handler: "print".to_string(),
            }],
            relations: vec![EntityRelation {
                id: Uuid::new_v4(),
                entity_type: "invoice".to_string(),
                code: "customer".to_string(),
                target_type: "counterparty".to_string(),
                kind: RelationKind::OneToOne,
                on_delete: OnDelete::Restrict,
            }],
        }
    }

    #[tokio::test]
    async fn create_round_trip_and_schema_assembly() {
        let db = mem_db().await;
        let repo = SurrealMetadataRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();

        let schema = sample_schema(1);
        repo.create_entity_type(&schema, &[system_event(schema.entity_type.id, "metadata.entity_type.created")])
            .await
            .unwrap();

        let got = repo.get_entity_type(&schema.entity_type.id).await.unwrap();
        assert_eq!(got.code, "invoice");
        assert_eq!(got.metadata_version, 1);

        let fetched = repo
            .get_entity_type_by_code("", "invoice")
            .await
            .unwrap();
        assert_eq!(fetched, got);

        assert_eq!(repo.list_entity_types().await.unwrap().len(), 1);

        let assembled = repo.get_schema("", "invoice").await.unwrap();
        assert_eq!(assembled.fields.len(), 1);
        assert_eq!(assembled.states.len(), 2);
        assert_eq!(assembled.transitions.len(), 1);
        assert_eq!(assembled.forms.len(), 1);
        assert_eq!(assembled.actions.len(), 1);
        assert_eq!(assembled.relations.len(), 1);
        assert_eq!(assembled.transitions[0].from_state, "draft");

        let stream = store
            .read_stream(StreamType::Metadata, &schema.entity_type.id.to_string())
            .await
            .unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].event_type, "metadata.entity_type.created");
    }

    #[tokio::test]
    async fn re_registration_with_same_version_is_noop() {
        let db = mem_db().await;
        let repo = SurrealMetadataRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();

        let schema = sample_schema(2);
        repo.create_entity_type(&schema, &[system_event(schema.entity_type.id, "metadata.seeded")])
            .await
            .unwrap();
        repo.create_entity_type(&schema, &[system_event(schema.entity_type.id, "metadata.seeded")])
            .await
            .unwrap();

        assert_eq!(repo.list_entity_types().await.unwrap().len(), 1);
        let assembled = repo.get_schema("", "invoice").await.unwrap();
        assert_eq!(assembled.fields.len(), 1);
        assert_eq!(assembled.states.len(), 2);

        let stream = store
            .read_stream(StreamType::Metadata, &schema.entity_type.id.to_string())
            .await
            .unwrap();
        assert_eq!(stream.len(), 1);
    }

    #[tokio::test]
    async fn newer_version_updates_resources_and_preserves_count() {
        let db = mem_db().await;
        let repo = SurrealMetadataRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();

        let v1 = sample_schema(1);
        repo.create_entity_type(&v1, &[system_event(v1.entity_type.id, "metadata.seeded")])
            .await
            .unwrap();

        let mut v2 = sample_schema(2);
        v2.fields[0].label = "Итоговая сумма".to_string();
        v2.states.push(EntityState {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            code: "cancelled".to_string(),
            label: "Аннулирован".to_string(),
            color: None,
            is_initial: false,
            is_final: true,
        });
        repo.update_entity_type(&v2, &[system_event(v1.entity_type.id, "metadata.entity_type.updated")])
            .await
            .unwrap();

        let assembled = repo.get_schema("", "invoice").await.unwrap();
        assert_eq!(assembled.entity_type.metadata_version, 2);
        assert_eq!(assembled.fields[0].label, "Итоговая сумма");
        assert_eq!(assembled.states.len(), 3);
    }

    #[tokio::test]
    async fn transition_with_missing_state_rejected() {
        let db = mem_db().await;
        let repo = SurrealMetadataRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();

        let mut schema = sample_schema(1);
        schema.transitions[0].to_state = "reviewed".to_string();
        let err = repo
            .create_entity_type(&schema, &[system_event(schema.entity_type.id, "metadata.seeded")])
            .await;
        assert!(matches!(err, Err(DomainError::ValidationError(_))));
    }

    #[tokio::test]
    async fn update_missing_type_returns_not_found() {
        let db = mem_db().await;
        let repo = SurrealMetadataRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let schema = sample_schema(1);
        let err = repo
            .update_entity_type(&schema, &[system_event(schema.entity_type.id, "metadata.entity_type.updated")])
            .await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
    }

    #[tokio::test]
    async fn ensure_schema_is_idempotent() {
        let db = mem_db().await;
        let repo = SurrealMetadataRepository::new(db);
        repo.ensure_schema().await.unwrap();
        repo.ensure_schema().await.unwrap();
    }

    #[tokio::test]
    async fn global_system_type_is_resolvable_for_any_company() {
        let db = mem_db().await;
        let repo = SurrealMetadataRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();

        // Системный тип в глобальной компании "" (как при system.bootstrap).
        let mut schema = sample_schema(1);
        schema.entity_type.code = "company".to_string();
        schema.entity_type.is_system = true;
        for field in schema.fields.iter_mut() {
            field.entity_type = "company".to_string();
        }
        for state in schema.states.iter_mut() {
            state.entity_type = "company".to_string();
        }
        for transition in schema.transitions.iter_mut() {
            transition.entity_type = "company".to_string();
        }
        for form in schema.forms.iter_mut() {
            form.entity_type = "company".to_string();
        }
        for action in schema.actions.iter_mut() {
            action.entity_type = "company".to_string();
        }
        for relation in schema.relations.iter_mut() {
            relation.entity_type = "company".to_string();
        }
        repo.create_entity_type(
            &schema,
            &[system_event(schema.entity_type.id, "metadata.seeded")],
        )
        .await
        .unwrap();

        // Из любой компании системный тип виден благодаря фолбэку на company_id="".
        let other_company = Uuid::new_v4().to_string();
        let by_code = repo
            .get_entity_type_by_code(&other_company, "company")
            .await
            .unwrap();
        assert!(by_code.is_system);
        assert_eq!(by_code.company_id, "");
        let assembled = repo.get_schema(&other_company, "company").await.unwrap();
        assert_eq!(assembled.entity_type.code, "company");
        assert_eq!(assembled.fields.len(), 1);

        // Кастомный глобальный (company_id="", is_system=false) тип НЕ протекает.
        let mut custom = sample_schema(1);
        custom.entity_type.code = "note".to_string();
        repo.create_entity_type(
            &custom,
            &[system_event(custom.entity_type.id, "metadata.seeded")],
        )
        .await
        .unwrap();
        let err = repo.get_entity_type_by_code(&other_company, "note").await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
    }
}