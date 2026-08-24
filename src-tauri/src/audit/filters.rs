// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::AuditEntryView;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditFilters {
    pub actions: Vec<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub entity_type: Option<String>,
    pub user_id: Option<String>,
    pub search: Option<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub after: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
}

impl AuditFilters {
    pub fn effective_limit(&self) -> i64 {
        self.limit.unwrap_or(50).min(200).max(1) as i64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPage {
    pub entries: Vec<AuditEntryView>,
    pub total_count: i64,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub prev_cursor: Option<String>,
}
