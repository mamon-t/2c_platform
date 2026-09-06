use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Базовая сущность компании. Бизнес-специфика (ИНН, КПП, юридический адрес и т.д.)
/// становится настраиваемой через модель метаданных в Фазе 3, поэтому ядро
/// остаётся нейтральным.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    pub id: Uuid,
    /// Короткий уникальный идентификатор, используемый в ссылках, например `"acme"`.
    pub code: String,
    /// Краткое юридическое название, например `"ООО «Акме»"`.
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