use crate::core::*;
use chrono::{DateTime, Utc};
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityType {
    pub _id: Id,
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub kind: EntityKind,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityField {
    pub _id: Id,
    pub entity_type_id: Id,
    pub code: String,
    pub name: String,
    pub field_kind: FieldKind,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub options: Option<serde_json::Value>,
    pub order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityState {
    pub _id: Id,
    pub entity_type_id: Id,
    pub code: String,
    pub name: String,
    pub is_initial: bool,
    pub color: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTransition {
    pub _id: Id,
    pub entity_type_id: Id,
    pub code: String,
    pub name: String,
    pub from_state: String,
    pub to_state: String,
    pub action_code: Option<String>,
    pub script_code: Option<String>,
    pub require_signature: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAction {
    pub _id: Id,
    pub entity_type_id: Id,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub target_states: Option<Vec<String>>,
    pub script_code: Option<String>,
    pub require_signature: bool,
    pub created_at: DateTime<Utc>,
}

pub struct MetaService;

impl MetaService {
    pub fn new() -> Self {
        Self
    }

    pub fn create_entity_type(
        &self,
        company_id: CompanyId,
        code: &str,
        name: &str,
        kind: EntityKind,
    ) -> EntityType {
        let now = Utc::now();
        EntityType {
            _id: uuid::Uuid::new_v4(),
            company_id,
            code: code.to_string(),
            name: name.to_string(),
            kind,
            description: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn create_field(
        &self,
        entity_type_id: Id,
        code: &str,
        name: &str,
        field_kind: FieldKind,
        order: i32,
    ) -> EntityField {
        let now = Utc::now();
        EntityField {
            _id: uuid::Uuid::new_v4(),
            entity_type_id,
            code: code.to_string(),
            name: name.to_string(),
            field_kind,
            required: false,
            default_value: None,
            options: None,
            order,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn create_state(
        &self,
        entity_type_id: Id,
        code: &str,
        name: &str,
        is_initial: bool,
        order: i32,
    ) -> EntityState {
        EntityState {
            _id: uuid::Uuid::new_v4(),
            entity_type_id,
            code: code.to_string(),
            name: name.to_string(),
            is_initial,
            color: None,
            order,
        }
    }

    pub fn create_transition(
        &self,
        entity_type_id: Id,
        code: &str,
        name: &str,
        from_state: &str,
        to_state: &str,
    ) -> EntityTransition {
        EntityTransition {
            _id: uuid::Uuid::new_v4(),
            entity_type_id,
            code: code.to_string(),
            name: name.to_string(),
            from_state: from_state.to_string(),
            to_state: to_state.to_string(),
            action_code: None,
            script_code: None,
            require_signature: false,
        }
    }

    pub fn create_action(
        &self,
        entity_type_id: Id,
        code: &str,
        name: &str,
    ) -> EntityAction {
        EntityAction {
            _id: uuid::Uuid::new_v4(),
            entity_type_id,
            code: code.to_string(),
            name: name.to_string(),
            description: None,
            target_states: None,
            script_code: None,
            require_signature: false,
            created_at: Utc::now(),
        }
    }
}
