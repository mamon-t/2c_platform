pub mod actions;
pub mod changes;
pub mod filters;
pub mod macros;
pub mod service;
pub mod indexes;

pub use actions::AuditableAction;
pub use changes::{AuditChanges, FieldChange};
pub use filters::{AuditFilters, AuditPage};
pub use service::{AuditService, MongoAuditService};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{CompanyId, Id, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub _id: Id,
    pub user_id: UserId,
    pub company_id: CompanyId,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub entity_type: Option<String>,
    pub object_id: Option<String>,
    pub changes: Option<AuditChanges>,
    pub event_id: Option<String>,
    pub signature_ref: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

impl AuditEntry {
    pub fn new(
        action: AuditableAction,
        user_id: UserId,
        company_id: CompanyId,
        target_id: Option<String>,
        entity_type: Option<String>,
        object_id: Option<String>,
        changes: Option<AuditChanges>,
        event_id: Option<String>,
        signature_ref: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        Self {
            _id: Uuid::new_v4(),
            user_id,
            company_id,
            action: action.as_str().to_string(),
            target_type: action.target_type().to_string(),
            target_id,
            entity_type,
            object_id,
            changes,
            event_id,
            signature_ref,
            ip_address,
            user_agent,
            occurred_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntryView {
    #[serde(flatten)]
    pub entry: AuditEntry,
    pub user_login: Option<String>,
    pub target_login: Option<String>,
}
