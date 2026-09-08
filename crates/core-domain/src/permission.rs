//! Модель прав доступа: политики (`permission_policies`) и доступ к записям.
//! Реализация строгого RBAC (ТЗ v3.1, Приложение №7): детерминированный
//! алгоритм проверки через `PermissionManager::check_access`, явные запреты
//! с `deny: true`, приоритеты и deny-by-default.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Область действия политики.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PermissionScopeType {
    /// Политика платформы — действует для любой сферы.
    Platform,
    /// Конкретный модуль, например `Module("invoice")`.
    Module(String),
    /// Метаданные (типы сущностей).
    Metadata,
    /// Сфера не указана.
    #[default]
    None,
}

/// Уровень доступа к записям данных (записи, на которые распространяется политика).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecordAccessLevel {
    /// Только записи, созданные самим пользователем.
    Owned,
    /// Записи, назначенные на роль или пользователя.
    ByRole,
    /// Все записи в рамках компании.
    #[default]
    ByCompany,
    /// Полный доступ без ограничений записей.
    All,
}

/// Политика доступа: разрешение или запрет на действие над объектами.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionPolicy {
    pub id: Uuid,
    /// Машиночитаемый уникальный код, например `"invoice.create"`.
    pub code: String,
    /// Человекочитаемое имя политики.
    pub name: String,
    pub description: Option<String>,
    pub scope_type: PermissionScopeType,
    /// Тип сущности для детализации, например `"invoice"`.
    pub entity_type: Option<String>,
    /// Разрешённые действия: `["create", "read", "update", "*"]`.
    pub actions: Vec<String>,
    /// Уровень доступа к записям, на которые распространяется политика.
    pub record_access: RecordAccessLevel,
    /// `true` — явный запрет; запрет перекрывает все разрешения (deny overrides allow).
    pub deny: bool,
    /// Приоритет при конфликтах политик: большее значение — важнее.
    pub priority: i32,
    /// Модуль-источник политики (`is_system` политики имеют `None`).
    pub module_code: Option<String>,
    /// `true` — системная политика, не изменяется пользователями.
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample(code: &str, actions: Vec<&str>) -> PermissionPolicy {
        PermissionPolicy {
            id: Uuid::new_v4(),
            code: code.to_string(),
            name: code.to_string(),
            description: None,
            scope_type: PermissionScopeType::Platform,
            entity_type: None,
            actions: actions.into_iter().map(str::to_string).collect(),
            record_access: RecordAccessLevel::All,
            deny: false,
            priority: 0,
            module_code: None,
            is_system: true,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        }
    }

    #[test]
    fn serde_round_trip() {
        let policy = sample("admin.*", vec!["*"]);
        let json = serde_json::to_value(&policy).unwrap();
        let back: PermissionPolicy = serde_json::from_value(json).unwrap();
        assert_eq!(policy, back);
    }

    #[test]
    fn enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_value(PermissionScopeType::Module("invoice".to_string())).unwrap(),
            serde_json::json!({ "module": "invoice" })
        );
        assert_eq!(
            serde_json::to_value(RecordAccessLevel::ByCompany).unwrap(),
            "by_company"
        );
    }
}