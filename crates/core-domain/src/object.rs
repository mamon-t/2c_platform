use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{AggregateId, Version};

/// Kind of a business object as declared by its entity type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Document,
    Catalog,
    Register,
    Task,
    Contract,
    Project,
    Setting,
    Custom,
}

/// Universal business object stored in the `objects` collection.
/// A document is an object with `kind == Document`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: AggregateId,
    /// Reference to the object's entity type descriptor (`entity_types`).
    pub entity_type: String,
    pub kind: ObjectKind,
    pub company_id: String,
    /// Current state code from the type's state machine.
    pub state: String,
    /// User-provided field values.
    pub data: serde_json::Value,
    /// Computed values (formula fields, balances).
    pub computed: serde_json::Value,
    /// Document number; unique per entity type and company, assigned on post.
    pub number: Option<String>,
    pub date: Option<NaiveDate>,
    pub parent_id: Option<AggregateId>,
    /// Incremented on every write to enable OCC.
    pub version: Version,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}