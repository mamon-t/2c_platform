use crate::core::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub _id: Id,
    pub user_id: UserId,
    pub company_id: CompanyId,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Id>,
    pub entity_type: Option<String>,
    pub object_id: Option<Id>,
    pub changes: Option<serde_json::Value>,
    pub event_id: Option<Id>,
    pub signature_ref: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

pub struct AuditService;

impl AuditService {
    pub fn new() -> Self {
        Self
    }

    pub fn log(
        &self,
        ctx: &AuditContext,
        action: &str,
        target_type: &str,
        target_id: Option<Id>,
        changes: Option<serde_json::Value>,
    ) -> AuditEntry {
        AuditEntry {
            _id: uuid::Uuid::new_v4(),
            user_id: ctx.user_id.clone(),
            company_id: ctx.company_id.clone(),
            action: action.to_string(),
            target_type: target_type.to_string(),
            target_id,
            entity_type: None,
            object_id: None,
            changes,
            event_id: None,
            signature_ref: None,
            ip_address: ctx.ip_address.clone(),
            user_agent: ctx.user_agent.clone(),
            occurred_at: Utc::now(),
        }
    }
}
