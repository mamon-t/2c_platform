// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

pub mod service;
pub mod indexes;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::{CompanyId, Id, UserId};

/// Тип потока
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    Object,
    User,
    Module,
    Device,
}

impl std::fmt::Display for StreamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamType::Object => write!(f, "object"),
            StreamType::User => write!(f, "user"),
            StreamType::Module => write!(f, "module"),
            StreamType::Device => write!(f, "device"),
        }
    }
}

impl std::str::FromStr for StreamType {
    type Err = crate::core::PlatformError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "object" => Ok(StreamType::Object),
            "user" => Ok(StreamType::User),
            "module" => Ok(StreamType::Module),
            "device" => Ok(StreamType::Device),
            _ => Err(crate::core::PlatformError::Validation(format!("Неизвестный stream_type: {s}"))),
        }
    }
}

/// Снимок исполнителя (actor snapshot)
/// Сохраняется в событии, чтобы история оставалась читаемой при смене данных
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSnapshot {
    pub user_id: UserId,
    pub login: String,
    pub full_name: Option<String>,
    pub position: Option<String>,
    pub company_id: CompanyId,
}

/// Событие — append-only запись в Event Store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub _id: Id,
    pub stream_type: StreamType,
    pub stream_id: String,
    pub event_type: String,
    pub version: i64,
    pub payload: serde_json::Value,
    pub metadata: ActorSnapshot,
    pub company_id: CompanyId,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub signature_ref: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

/// Снимок версии объекта
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSnapshot {
    pub _id: Id,
    pub object_id: String,
    pub version: i64,
    pub data: serde_json::Value,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub reason: Option<String>,
}

/// Параметры фильтрации событий
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventFilters {
    pub stream_type: Option<String>,
    pub stream_id: Option<String>,
    pub event_type: Option<String>,
    pub company_id: Option<String>,
    pub correlation_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub after: Option<String>,
}

/// Страница событий
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPage {
    pub events: Vec<Event>,
    pub total_count: i64,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

pub struct EventService;

impl EventService {
    pub fn new() -> Self { Self }
}
