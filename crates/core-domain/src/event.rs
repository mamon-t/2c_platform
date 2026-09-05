use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Version;

/// Type of the stream an event belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    Object,
    User,
    Module,
}

/// A fact that has already happened and is written to the Event Store.
/// Stored in the `events` collection, an append-only journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub stream_type: StreamType,
    pub stream_id: String,
    pub event_type: String,
    pub version: Version,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
    pub company_id: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

/// Snapshot of the actor who produced the event, kept for readable audit
/// history even if the person's name or employment later changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub actor_user_id: String,
    pub actor_login: String,
    pub actor_full_name: String,
    pub ip_address: Option<String>,
}