use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Состояние жизненного цикла учётной записи пользователя.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    /// Приглашение отправлено, смена пароля ещё ожидается.
    Invited,
    Active,
    /// Отключена вручную администратором.
    Disabled,
    /// Заблокирована из-за повторных неудачных входов или явного действия.
    Locked,
    /// Учётная запись выведена из эксплуатации и сохранена для истории аудита
    /// (удаление пользователей с историей запрещено ТЗ).
    Archived,
}

/// Тип контактного канала, согласно расширенной модели пользователя.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactChannelType {
    Email,
    Phone,
    Telegram,
}

/// Назначения, которые может выполнять контактный канал.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactPurpose {
    /// Используется как основной идентификатор входа.
    Login,
    /// Получает системные уведомления.
    Notification,
    /// Используется для криптографической подписи / проверки.
    Signing,
}

/// Расширенная учётная запись пользователя. Пароли хранятся как хеши Argon2id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub login: String,
    pub password_hash: String,
    pub status: UserStatus,
    /// Коды ролей, назначенных этому пользователю.
    pub role_ids: Vec<String>,
    pub failed_login_count: u32,
    pub locked_until: Option<DateTime<Utc>>,
    pub must_change_password: bool,
    pub locale: String,
    pub timezone: String,
    /// Ссылается на запись `persons`, содержащую данные о личности.
    pub person_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Данные о личности физического лица, хранятся отдельно от учётной записи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub id: Uuid,
    pub user_id: Uuid,
    pub last_name: String,
    pub first_name: String,
    pub middle_name: Option<String>,
    /// Предпочитаемое отображаемое имя, например `"Иванов Иван Иванович"`.
    pub display_name: String,
}

/// Контактный канал пользователя, например `ivan@example.com`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContact {
    pub id: Uuid,
    pub user_id: Uuid,
    pub channel_type: ContactChannelType,
    pub value: String,
    /// Только один контакт на канал `channel_type` может быть основным.
    pub is_primary: bool,
    pub is_verified: bool,
    pub purposes: Vec<ContactPurpose>,
}

/// Трудовой профиль пользователя в конкретной компании.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCompanyProfile {
    pub id: Uuid,
    pub user_id: Uuid,
    pub company_id: Uuid,
    pub employee_number: Option<String>,
    pub position: Option<String>,
    pub department: Option<String>,
    pub is_primary: bool,
    pub is_active: bool,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
}

/// Криптографический сертификат, прикреплённый к профилю пользователя для подписи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCertificate {
    pub id: Uuid,
    pub user_id: Uuid,
    /// Код провайдера, например `"cryptopro"` (интеграция Фазы 16).
    pub provider_code: String,
    /// Ссылка/идентификатор сертификата, выданного провайдером.
    pub certificate_ref: String,
    pub subject: String,
    pub issuer: String,
    pub fingerprint: String,
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_user() -> User {
        User {
            id: Uuid::new_v4(),
            login: "ivanov".to_string(),
            password_hash: "$argon2id$test".to_string(),
            status: UserStatus::Active,
            role_ids: vec!["accountant".to_string()],
            failed_login_count: 0,
            locked_until: None,
            must_change_password: false,
            locale: "ru-RU".to_string(),
            timezone: "Europe/Moscow".to_string(),
            person_id: Some(Uuid::new_v4()),
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        }
    }

    #[test]
    fn user_serde_round_trip() {
        let user = sample_user();
        let json = serde_json::to_value(&user).unwrap();
        let back: User = serde_json::from_value(json).unwrap();
        assert_eq!(user, back);
    }

    #[test]
    fn person_contact_profile_cert_serde_round_trip() {
        let user_id = Uuid::new_v4();
        let person = Person {
            id: Uuid::new_v4(),
            user_id,
            last_name: "Иванов".to_string(),
            first_name: "Иван".to_string(),
            middle_name: Some("Иванович".to_string()),
            display_name: "Иванов Иван Иванович".to_string(),
        };
        let contact = UserContact {
            id: Uuid::new_v4(),
            user_id,
            channel_type: ContactChannelType::Email,
            value: "ivan@example.com".to_string(),
            is_primary: true,
            is_verified: false,
            purposes: vec![ContactPurpose::Login, ContactPurpose::Notification],
        };
        let profile = UserCompanyProfile {
            id: Uuid::new_v4(),
            user_id,
            company_id: Uuid::new_v4(),
            employee_number: Some("0001".to_string()),
            position: Some("Бухгалтер".to_string()),
            department: None,
            is_primary: true,
            is_active: true,
            valid_from: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            valid_to: None,
        };
        let certificate = UserCertificate {
            id: Uuid::new_v4(),
            user_id,
            provider_code: "cryptopro".to_string(),
            certificate_ref: "ref-1".to_string(),
            subject: "CN=Ivanov".to_string(),
            issuer: "CN=CA".to_string(),
            fingerprint: "AA:BB:CC".to_string(),
            is_active: true,
        };

        for value in [
            serde_json::to_value(&person).unwrap(),
            serde_json::to_value(&contact).unwrap(),
            serde_json::to_value(&profile).unwrap(),
            serde_json::to_value(&certificate).unwrap(),
        ] {
            serde_json::from_value::<serde_json::Value>(value).unwrap();
        }
        assert_eq!(
            serde_json::from_value::<Person>(serde_json::to_value(&person).unwrap()).unwrap(),
            person
        );
    }

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(UserStatus::Locked).unwrap(),
            "locked".to_string()
        );
        assert_eq!(
            serde_json::from_str::<UserStatus>("\"archived\"").unwrap(),
            UserStatus::Archived
        );
        assert_eq!(
            serde_json::to_value(ContactChannelType::Telegram).unwrap(),
            "telegram".to_string()
        );
    }
}