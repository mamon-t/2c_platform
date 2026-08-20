pub mod service;
pub mod indexes;

use crate::core::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityType {
    pub _id: Id,
    pub company_id: Option<CompanyId>,
    pub code: String,
    pub name: String,
    pub kind: EntityKind,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub is_active: bool,
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
    pub is_required: bool,
    pub is_readonly: bool,
    pub default_value: Option<serde_json::Value>,
    pub enum_values: Option<Vec<String>>,
    pub reference_entity: Option<String>,
    pub order: i32,
    pub group_name: Option<String>,
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
    pub is_final: bool,
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
    pub required_policy: Option<String>,
    pub require_signature: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityForm {
    pub _id: Id,
    pub entity_type_id: Id,
    pub code: String,
    pub name: String,
    pub layout: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAction {
    pub _id: Id,
    pub entity_type_id: Id,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub action_type: Option<String>,
    pub is_dangerous: bool,
    pub created_at: DateTime<Utc>,
}

// ── Input types ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEntityTypeInput {
    pub code: String,
    pub name: String,
    pub kind: EntityKind,
    pub description: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEntityTypeInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEntityFieldInput {
    pub entity_type_id: String,
    pub code: String,
    pub name: String,
    pub field_kind: FieldKind,
    pub is_required: Option<bool>,
    pub is_readonly: Option<bool>,
    pub default_value: Option<serde_json::Value>,
    pub enum_values: Option<Vec<String>>,
    pub reference_entity: Option<String>,
    pub group_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEntityFieldInput {
    pub name: Option<String>,
    pub is_required: Option<bool>,
    pub is_readonly: Option<bool>,
    pub default_value: Option<serde_json::Value>,
    pub enum_values: Option<Vec<String>>,
    pub reference_entity: Option<String>,
    pub group_name: Option<String>,
    pub order: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEntityStateInput {
    pub entity_type_id: String,
    pub code: String,
    pub name: String,
    pub is_initial: Option<bool>,
    pub is_final: Option<bool>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEntityStateInput {
    pub name: Option<String>,
    pub color: Option<String>,
    pub is_final: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEntityTransitionInput {
    pub entity_type_id: String,
    pub code: String,
    pub name: String,
    pub from_state: String,
    pub to_state: String,
    pub required_policy: Option<String>,
    pub require_signature: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEntityTransitionInput {
    pub name: Option<String>,
    pub required_policy: Option<String>,
    pub require_signature: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEntityFormInput {
    pub entity_type_id: String,
    pub code: String,
    pub name: String,
    pub layout: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEntityFormInput {
    pub name: Option<String>,
    pub layout: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEntityActionInput {
    pub entity_type_id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub action_type: Option<String>,
    pub is_dangerous: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEntityActionInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub action_type: Option<String>,
    pub is_dangerous: Option<bool>,
}
