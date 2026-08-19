use crate::core::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    InApp,
    Email,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus {
    Pending,
    Sent,
    Failed,
    Read,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTemplate {
    pub _id: Id,
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub channel: NotificationChannel,
    pub subject_template: Option<String>,
    pub body_template: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationOutbox {
    pub _id: Id,
    pub company_id: CompanyId,
    pub template_code: String,
    pub channel: NotificationChannel,
    pub recipient_user_id: UserId,
    pub subject: Option<String>,
    pub body: String,
    pub status: NotificationStatus,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
}

pub struct NotificationService;

impl NotificationService {
    pub fn new() -> Self {
        Self
    }

    pub fn create_outbox_entry(
        &self,
        company_id: CompanyId,
        template_code: &str,
        channel: NotificationChannel,
        recipient_user_id: UserId,
        subject: Option<String>,
        body: String,
    ) -> NotificationOutbox {
        NotificationOutbox {
            _id: uuid::Uuid::new_v4(),
            company_id,
            template_code: template_code.to_string(),
            channel,
            recipient_user_id,
            subject,
            body,
            status: NotificationStatus::Pending,
            attempts: 0,
            last_error: None,
            created_at: Utc::now(),
            sent_at: None,
        }
    }
}
