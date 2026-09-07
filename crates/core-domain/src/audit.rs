//! Модель операционного аудита (`audit_log`) — отдельная подсистема, отличная
//! от Event Store. Хранит операционные действия пользователей и системы для
//! безопасности, compliance и отладки (раздел 8.5 ТЗ v3.1, Приложение №6).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event::ActorSnapshot;

/// Запись операционного аудита.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    /// Идентификатор действия: "user.login", "module.install", "command.executed".
    pub action: String,
    /// Снимок исполнителя, зафиксированный на момент действия.
    pub actor: ActorSnapshot,
    /// Целевая сущность, на которую направлено действие.
    pub target: Option<AuditTarget>,
    /// Результат выполнения действия.
    pub result: AuditResult,
    /// Дополнительные данные, полезные для отладки.
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    /// Компания, в рамках которой выполнено действие (для аудита по компании).
    pub company_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
}

/// Целевая сущность действия.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTarget {
    /// Тип сущности: "user", "module", "role" и т.п.
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    /// Для модулей, ролей и других сущностей с кодовым обозначением.
    pub entity_code: Option<String>,
    pub company_id: Option<Uuid>,
}

/// Результат действия.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    Success,
    Failure { reason: String },
}

/// Фильтр выборки записей аудита. Используется только серверно
/// (в репозитории и командах), поэтому без сериализации.
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub action: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub target_entity_type: Option<String>,
    pub target_entity_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    /// `Some(true)` — только успешные, `Some(false)` — только неуспешные.
    pub result_success: Option<bool>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}