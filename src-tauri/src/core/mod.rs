use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Id = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CompanyId(pub Id);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct UserId(pub Id);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EntityTypeId(pub Id);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RoleId(pub Id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditContext {
    pub user_id: UserId,
    pub company_id: CompanyId,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMeta {
    pub id: EntityTypeId,
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub kind: EntityKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Document,
    Catalog,
    Register,
    Task,
    Contract,
    Project,
    Setting,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    String,
    Text,
    Integer,
    Money,
    Date,
    DateTime,
    Boolean,
    Enum,
    Reference,
    Array,
    Table,
    Json,
    File,
    User,
    Company,
    Formula,
    Computed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectState {
    Draft,
    Active,
    Posted,
    Cancelled,
    Archived,
    Deleted,
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Ошибка MongoDB: {0}")]
    Database(String),

    #[error("Сущность не найдена: {0}")]
    NotFound(String),

    #[error("Нарушение прав доступа: {0}")]
    PermissionDenied(String),

    #[error("Невалидные данные: {0}")]
    Validation(String),

    #[error("Ошибка аутентификации: {0}")]
    Auth(String),

    #[error("Ошибка скрипта Rhai: {0}")]
    Script(String),

    #[error("Ошибка подписи: {0}")]
    Crypto(String),

    #[error("Ошибка уведомления: {0}")]
    Notification(String),

    #[error("Внутренняя ошибка: {0}")]
    Internal(String),
}

impl serde::Serialize for PlatformError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<mongodb::error::Error> for PlatformError {
    fn from(err: mongodb::error::Error) -> Self {
        PlatformError::Database(err.to_string())
    }
}

impl From<rhai::EvalAltResult> for PlatformError {
    fn from(err: rhai::EvalAltResult) -> Self {
        PlatformError::Script(err.to_string())
    }
}

pub type PlatformResult<T> = Result<T, PlatformError>;

pub mod middleware;
