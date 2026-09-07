use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Роль группирует права доступа и назначается пользователям через `role_ids`.
/// Коды разрешающих политик собраны в `permission_policy_codes` (Приложение №7 ТЗ v3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub id: Uuid,
    /// Компания, в рамках которой действует роль.
    pub company_id: Uuid,
    /// Уникальный машиночитаемый код, например `"accountant"`.
    pub code: String,
    /// Человекочитаемое имя, например `"Бухгалтер"`.
    pub name: String,
    pub description: String,
    /// Ссылки на коды политик доступа, закреплённых за ролью.
    pub permission_policy_codes: Vec<String>,
    /// Системные роли (например `admin`) не могут быть удалены или переименованы пользователями.
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn serde_round_trip() {
        let role = Role {
            id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            code: "accountant".to_string(),
            name: "Бухгалтер".to_string(),
            description: "Ведёт учёт".to_string(),
            permission_policy_codes: vec!["accounting.operate".to_string()],
            is_system: false,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        };
        let json = serde_json::to_value(&role).unwrap();
        let back: Role = serde_json::from_value(json).unwrap();
        assert_eq!(role, back);
    }
}