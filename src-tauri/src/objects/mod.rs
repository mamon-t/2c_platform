pub mod service;
pub mod indexes;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::{CompanyId, Id, UserId};
use crate::core::ObjectState;

/// Универсальный объект — хранит все сущности (документы, справочники и т.д.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub _id: Id,
    pub entity_type_id: String,
    pub kind: String,
    pub company_id: CompanyId,
    pub state: ObjectState,
    pub data: serde_json::Value,
    pub computed: Option<serde_json::Value>,
    pub number: Option<String>,
    pub date: Option<String>,
    pub parent_id: Option<String>,
    pub version: i64,
    pub created_by: UserId,
    pub updated_by: UserId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Снимок версии объекта (для истории изменений)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSnapshot {
    pub _id: Id,
    pub object_id: String,
    pub version: i64,
    pub data: serde_json::Value,
    pub state: ObjectState,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub reason: Option<String>,
}

/// Результат往后操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPage {
    pub objects: Vec<Object>,
    pub total_count: i64,
    pub has_more: bool,
}

/// Фильтры для поиска объектов
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ObjectFilters {
    pub entity_type_id: Option<String>,
    pub state: Option<String>,
    pub parent_id: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Входные данные для создания объекта
#[derive(Debug, Clone, Deserialize)]
pub struct CreateObjectInput {
    pub entity_type_id: String,
    pub data: serde_json::Value,
    pub parent_id: Option<String>,
    pub date: Option<String>,
}

/// Входные данные для обновления объекта
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateObjectInput {
    pub data: serde_json::Value,
    pub version: i64,
    pub reason: Option<String>,
}
