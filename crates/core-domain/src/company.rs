use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Core company entity. Business specifics (INN, KPP, legal address, etc.)
/// become configurable through the metadata model in Phase 3, so the kernel
/// stays neutral.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    pub id: Uuid,
    /// Short unique handle used in references, e.g. `"acme"`.
    pub code: String,
    /// Legal short name, e.g. `"ООО «Акме»"`.
    pub name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample() -> Company {
        Company {
            id: Uuid::new_v4(),
            code: "acme".to_string(),
            name: "ООО «Акме»".to_string(),
            is_active: true,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        }
    }

    #[test]
    fn serde_round_trip() {
        let company = sample();
        let json = serde_json::to_value(&company).unwrap();
        let back: Company = serde_json::from_value(json).unwrap();
        assert_eq!(company, back);
    }
}