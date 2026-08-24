// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Уведомления и сообщения.
//!
//! **Уведомления** — односторонние системные или бизнес-события.
//! Пользователь читает и действует. Модель: `Notification`.
//! **Сообщения** — двусторонняя коммуникация (v0.2).
//!
//! Проекции: событие из Event Store → шаблон → подписка → уведомление.

pub mod commands;
pub mod projection;
pub mod service;

use chrono::{DateTime, Utc};
use crate::core::{CompanyId, UserId};
use serde::{Deserialize, Serialize};

// ── Каналы доставки ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    InApp,
    Email,
}

impl NotificationChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InApp => "inapp",
            Self::Email => "email",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Info,
    Warning,
    Critical,
}

// ── Уведомление (то, что видит пользователь) ──────────────

/// Привязка к объекту платформы для навигации.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_type: String,
    pub entity_id: String,
}

/// Уведомление в коллекции `notifications`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: String,
    /// Кому адресовано
    pub user_id: String,
    /// Тип события: document.approved, handover.overdue…
    pub notification_type: String,
    pub severity: String, // info | warning | critical
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub entity_ref: Option<EntityRef>,
    #[serde(default = "default_channels")]
    pub channels: Vec<String>,
    /// pending → delivered → read → archived
    pub status: String,
    #[serde(default)]
    pub delivered_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub read_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

fn default_channels() -> Vec<String> { vec!["inapp".into()] }

// ── Шаблон уведомления ─────────────────────────────────────

/// company_id = None → глобальный шаблон платформы.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTemplate {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: Option<String>,
    pub event_type: String,
    pub channel: String,
    pub subject: String,
    pub body: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Подписка пользователя ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSubscription {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: String,
    pub user_id: String,
    /// Тип события или "*" для всех
    pub event_type: String,
    pub channels: Vec<String>,
    pub is_muted: bool,
    pub updated_at: DateTime<Utc>,
}

// ── Outbox для гарантированной доставки (email/push v0.3) ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus {
    Pending,
    Delivered,
    Sent,
    Failed,
    Read,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationOutbox {
    #[serde(rename = "_id")]
    pub _id: crate::core::Id,
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


// ── Фабрика outbox-записей (обратная совместимость) ────────

pub struct NotificationService;

impl NotificationService {
    pub fn new() -> Self { Self }

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
            status: crate::notify::NotificationStatus::Pending,
            attempts: 0,
            last_error: None,
            created_at: Utc::now(),
            sent_at: None,
        }
    }
}
