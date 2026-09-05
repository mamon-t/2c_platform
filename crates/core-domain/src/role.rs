use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A role groups permissions and is assigned to users through `role_ids`.
/// Permission bindings arrive in Phase 6 (`permission_policies`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub id: Uuid,
    /// Unique machine-readable code, e.g. `"accountant"`.
    pub code: String,
    /// Human-readable name, e.g. `"Бухгалтер"`.
    pub name: String,
    pub description: String,
    /// System roles (e.g. `root`) cannot be deleted or renamed by users.
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
            code: "accountant".to_string(),
            name: "Бухгалтер".to_string(),
            description: "Ведёт учёт".to_string(),
            is_system: false,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        };
        let json = serde_json::to_value(&role).unwrap();
        let back: Role = serde_json::from_value(json).unwrap();
        assert_eq!(role, back);
    }
}