use crate::core::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub _id: Id,
    pub stream_type: String,
    pub stream_id: Id,
    pub event_type: String,
    pub version: i64,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
    pub company_id: CompanyId,
    pub user_id: UserId,
    pub module_code: Option<String>,
    pub correlation_id: Option<Id>,
    pub causation_id: Option<Id>,
    pub signature_ref: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSnapshot {
    pub _id: Id,
    pub object_id: Id,
    pub version: i64,
    pub data: serde_json::Value,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub reason: Option<String>,
}

pub struct EventService;

impl EventService {
    pub fn new() -> Self {
        Self
    }

    pub fn new_event(
        &self,
        stream_type: &str,
        stream_id: Id,
        event_type: &str,
        version: i64,
        payload: serde_json::Value,
        company_id: CompanyId,
        user_id: UserId,
    ) -> Event {
        Event {
            _id: uuid::Uuid::new_v4(),
            stream_type: stream_type.to_string(),
            stream_id,
            event_type: event_type.to_string(),
            version,
            payload,
            metadata: serde_json::Value::Null,
            company_id,
            user_id,
            module_code: None,
            correlation_id: None,
            causation_id: None,
            signature_ref: None,
            occurred_at: Utc::now(),
        }
    }

    pub fn new_snapshot(
        &self,
        object_id: Id,
        version: i64,
        data: serde_json::Value,
        created_by: UserId,
    ) -> ObjectSnapshot {
        ObjectSnapshot {
            _id: uuid::Uuid::new_v4(),
            object_id,
            version,
            data,
            created_by,
            created_at: Utc::now(),
            reason: None,
        }
    }
}
