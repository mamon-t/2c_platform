// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Сообщения — двусторонняя/групповая коммуникация.
//!
//! Комнаты: direct (личный диалог), group, document (обсуждение документа).
//! Привязка к документам: комната типа document создаётся автоматически
//! при проведении, участники видят переписку рядом с документом.

pub mod commands;
pub mod indexes;
pub mod service;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Коллекции ──────────────────────────────────────────────

pub const COL_ROOMS: &str = "messaging_rooms";
pub const COL_MESSAGES: &str = "messaging_messages";
pub const COL_READS: &str = "messaging_reads";

// ── Типы комнат ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RoomType {
    Direct,
    Group,
    Document,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingRoom {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: String,
    pub room_type: RoomType,
    /// Название (для group/document; для direct генерируется из участников)
    #[serde(default)]
    pub title: Option<String>,
    /// UUID участников
    pub members: Vec<String>,
    /// Привязка к объекту платформы (для document-комнат)
    #[serde(default)]
    pub entity_ref: Option<serde_json::Value>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub last_message_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub is_archived: bool,
}

/// Сообщение в комнате.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingMessage {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: String,
    pub room_id: String,
    pub author_id: String,
    pub content: String,
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub is_deleted: bool,
    #[serde(default)]
    pub edited_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Статус прочтения комнаты пользователем.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomRead {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: String,
    pub room_id: String,
    pub user_id: String,
    pub last_read_message_id: String,
    pub updated_at: DateTime<Utc>,
}
