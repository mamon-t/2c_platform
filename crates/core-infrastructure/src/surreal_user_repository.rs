//! Хранилище пользователей, персон, контактов, профилей и сертификатов
//! на базе SurrealDB (проекция «Доска»).

use core_application::ports::UserRepository;
use core_domain::error::DomainError;
use core_domain::event::Event;
use core_domain::user::{
    Person, User, UserCertificate, UserCompanyProfile, UserContact,
};
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

use crate::events::{assign_versions, with_transaction, write_events};

/// Материализованное хранилище расширенной модели пользователя.
pub struct SurrealUserRepository {
    db: Surreal<Any>,
}

impl SurrealUserRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Создаёт таблицы и индексы расширенной модели пользователя идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        const STATEMENTS: &[&str] = &[
            "DEFINE TABLE IF NOT EXISTS users SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_users_login ON users FIELDS login UNIQUE",
            "DEFINE TABLE IF NOT EXISTS persons SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_persons_user ON persons FIELDS user_id",
            "DEFINE TABLE IF NOT EXISTS user_contacts SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_user_contacts_user ON user_contacts FIELDS user_id",
            "DEFINE TABLE IF NOT EXISTS user_company_profiles SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_user_profiles_user_company ON user_company_profiles FIELDS user_id, company_id",
            "DEFINE TABLE IF NOT EXISTS user_certificates SCHEMALESS",
            "DEFINE INDEX IF NOT EXISTS idx_user_certs_user ON user_certificates FIELDS user_id",
        ];
        for stmt in STATEMENTS {
            let mut response = self
                .db
                .query(*stmt)
                .await
                .map_err(|e| DomainError::Storage(format!("users ensure_schema: {e}")))?;
            let _: Option<Value> = response
                .take(0)
                .map_err(|e| DomainError::Storage(format!("users ensure_schema take: {e}")))?;
        }
        Ok(())
    }
}

const USER_FIELDS: &str = "record::id(id) AS id, login, password_hash, status, role_ids, \
    failed_login_count, locked_until, must_change_password, locale, timezone, person_id, \
    created_at, updated_at";
const PERSON_FIELDS: &str = "record::id(id) AS id, user_id, last_name, first_name, middle_name, \
    display_name";
const CONTACT_FIELDS: &str = "record::id(id) AS id, user_id, channel_type, value, is_primary, \
    is_verified, purposes";
const PROFILE_FIELDS: &str = "record::id(id) AS id, user_id, company_id, employee_number, \
    position, department, is_primary, is_active, valid_from, valid_to";
const CERT_FIELDS: &str = "record::id(id) AS id, user_id, provider_code, certificate_ref, \
    subject, issuer, fingerprint, is_active";

fn decode_row<T: serde::de::DeserializeOwned>(
    kind: &str,
    row: Option<Value>,
) -> Result<T, DomainError> {
    row.map(|v| serde_json::from_value(v))
        .transpose()
        .map_err(|e| DomainError::Storage(format!("{kind} decode: {e}")))?
        .ok_or_else(|| DomainError::NotFound(kind.to_string()))
}

fn decode_rows<T: serde::de::DeserializeOwned>(
    kind: &str,
    rows: Vec<Value>,
) -> Result<Vec<T>, DomainError> {
    serde_json::from_value(Value::Array(rows))
        .map_err(|e| DomainError::Storage(format!("{kind} list decode: {e}")))
}

impl UserRepository for SurrealUserRepository {
    async fn create(&self, user: &User, person: &Person, events: &[Event]) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let mut existing_response = txn
                    .query("SELECT 1 FROM users WHERE login = $login LIMIT 1")
                    .bind(("login", user.login.clone()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("user check login: {e}")))?;
                let existing: Option<Value> = existing_response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("user check login take: {e}")))?;
                if existing.is_some() {
                    return Err(DomainError::ValidationError(format!(
                        "Пользователь с логином {} уже существует",
                        user.login
                    )));
                }

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;

                let user_value = serde_json::to_value(user)
                    .map_err(|e| DomainError::Storage(format!("user encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("users", user.id.to_string()))
                    .content(user_value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("user write: {e}")))?;
                let person_value = serde_json::to_value(person)
                    .map_err(|e| DomainError::Storage(format!("person encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("persons", person.id.to_string()))
                    .content(person_value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("person write: {e}")))?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn get(&self, id: &Uuid) -> Result<User, DomainError> {
        let mut response = self
            .db
            .query(format!("SELECT {USER_FIELDS} FROM users WHERE record::id(id) = $id LIMIT 1"))
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("user get: {e}")))?;
        let row: Option<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("user get take: {e}")))?;
        decode_row("Пользователь", row)
    }

    async fn get_by_login(&self, login: &str) -> Result<User, DomainError> {
        let mut response = self
            .db
            .query(format!("SELECT {USER_FIELDS} FROM users WHERE login = $login LIMIT 1"))
            .bind(("login", login.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("user get_by_login: {e}")))?;
        let row: Option<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("user get_by_login take: {e}")))?;
        decode_row("Пользователь", row)
    }

    async fn list(&self) -> Result<Vec<User>, DomainError> {
        let mut response = self
            .db
            .query(format!("SELECT {USER_FIELDS} FROM users ORDER BY login"))
            .await
            .map_err(|e| DomainError::Storage(format!("user list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("user list take: {e}")))?;
        decode_rows("Пользователь", rows)
    }

    async fn update(&self, user: &User, events: &[Event]) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let mut existing_response = txn
                    .query("SELECT 1 FROM users WHERE record::id(id) = $id LIMIT 1")
                    .bind(("id", user.id.to_string()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("user check id: {e}")))?;
                let existing: Option<Value> = existing_response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("user check id take: {e}")))?;
                if existing.is_none() {
                    return Err(DomainError::NotFound(format!("Пользователь {} не найден", user.id)));
                }

                let mut collision_response = txn
                    .query(
                        "SELECT 1 FROM users \
                         WHERE login = $login AND record::id(id) != $id LIMIT 1",
                    )
                    .bind(("login", user.login.clone()))
                    .bind(("id", user.id.to_string()))
                    .await
                    .map_err(|e| DomainError::Storage(format!("user check login: {e}")))?;
                let collision: Option<Value> = collision_response
                    .take(0)
                    .map_err(|e| DomainError::Storage(format!("user check login take: {e}")))?;
                if collision.is_some() {
                    return Err(DomainError::ValidationError(format!(
                        "Логин {} уже занят другим пользователем",
                        user.login
                    )));
                }

                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;

                let value = serde_json::to_value(user)
                    .map_err(|e| DomainError::Storage(format!("user encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("users", user.id.to_string()))
                    .content(value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("user write: {e}")))?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn get_person(&self, user_id: &Uuid) -> Result<Person, DomainError> {
        let mut response = self
            .db
            .query(format!("SELECT {PERSON_FIELDS} FROM persons WHERE user_id = $uid LIMIT 1"))
            .bind(("uid", user_id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("person get: {e}")))?;
        let row: Option<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("person get take: {e}")))?;
        decode_row("Личные данные пользователя", row)
    }

    async fn add_contact(&self, contact: &UserContact, events: &[Event]) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;

                let value = serde_json::to_value(contact)
                    .map_err(|e| DomainError::Storage(format!("contact encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("user_contacts", contact.id.to_string()))
                    .content(value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("contact write: {e}")))?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn list_contacts(&self, user_id: &Uuid) -> Result<Vec<UserContact>, DomainError> {
        let mut response = self
            .db
            .query(format!(
                "SELECT {CONTACT_FIELDS} FROM user_contacts WHERE user_id = $uid ORDER BY channel_type"
            ))
            .bind(("uid", user_id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("contact list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("contact list take: {e}")))?;
        decode_rows("Контакт", rows)
    }

    async fn add_profile(&self, profile: &UserCompanyProfile, events: &[Event]) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;

                let value = serde_json::to_value(profile)
                    .map_err(|e| DomainError::Storage(format!("profile encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("user_company_profiles", profile.id.to_string()))
                    .content(value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("profile write: {e}")))?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn list_profiles(&self, user_id: &Uuid) -> Result<Vec<UserCompanyProfile>, DomainError> {
        let mut response = self
            .db
            .query(format!(
                "SELECT {PROFILE_FIELDS} FROM user_company_profiles WHERE user_id = $uid ORDER BY company_id"
            ))
            .bind(("uid", user_id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("profile list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("profile list take: {e}")))?;
        decode_rows("Профиль сотрудника", rows)
    }

    async fn add_certificate(
        &self,
        certificate: &UserCertificate,
        events: &[Event],
    ) -> Result<(), DomainError> {
        with_transaction(&self.db, |txn| async move {
            let outcome: Result<(), DomainError> = async {
                let mut events = events.to_vec();
                assign_versions(&txn, &mut events).await?;
                write_events(&txn, &events).await?;

                let value = serde_json::to_value(certificate)
                    .map_err(|e| DomainError::Storage(format!("certificate encode: {e}")))?;
                let _: Option<surrealdb::types::Value> = txn
                    .upsert(("user_certificates", certificate.id.to_string()))
                    .content(value)
                    .await
                    .map_err(|e| DomainError::Storage(format!("certificate write: {e}")))?;
                Ok(())
            }
            .await;
            (txn, outcome)
        })
        .await
    }

    async fn list_certificates(&self, user_id: &Uuid) -> Result<Vec<UserCertificate>, DomainError> {
        let mut response = self
            .db
            .query(format!(
                "SELECT {CERT_FIELDS} FROM user_certificates WHERE user_id = $uid ORDER BY fingerprint"
            ))
            .bind(("uid", user_id.to_string()))
            .await
            .map_err(|e| DomainError::Storage(format!("certificate list: {e}")))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|e| DomainError::Storage(format!("certificate list take: {e}")))?;
        decode_rows("Сертификат", rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_application::ports::EventStore;
    use core_domain::event::{ActorSnapshot, Event, StreamType};
    use core_domain::user::{ContactChannelType, ContactPurpose};
    use uuid::Uuid;

    async fn mem_repo() -> (SurrealUserRepository, crate::SurrealEventStore) {
        let db = surrealdb::engine::any::connect("mem://").await.unwrap();
        db.use_ns("test")
            .use_db(Uuid::new_v4().to_string())
            .await
            .unwrap();
        let repo = SurrealUserRepository::new(db.clone());
        repo.ensure_schema().await.unwrap();
        let store = crate::SurrealEventStore::new(db);
        store.ensure_schema().await.unwrap();
        (repo, store)
    }

    fn event(
        stream_type: StreamType,
        stream_id: &str,
        event_type: &str,
        company_id: &str,
    ) -> Event {
        Event {
            id: Uuid::new_v4(),
            stream_type,
            stream_id: stream_id.to_string(),
            event_type: event_type.to_string(),
            version: 0,
            payload: serde_json::json!({}),
            metadata: ActorSnapshot::system(),
            company_id: company_id.to_string(),
            correlation_id: "corr".to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        }
    }

    fn sample_user(id: Uuid) -> User {
        User {
            id,
            login: "ivanov".to_string(),
            password_hash: "$argon2id$test".to_string(),
            status: core_domain::user::UserStatus::Active,
            role_ids: vec!["accountant".to_string()],
            failed_login_count: 0,
            locked_until: None,
            must_change_password: false,
            locale: "ru-RU".to_string(),
            timezone: "Europe/Moscow".to_string(),
            person_id: Some(id),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn sample_person(user_id: Uuid) -> Person {
        Person {
            id: user_id,
            user_id,
            last_name: "Иванов".to_string(),
            first_name: "Иван".to_string(),
            middle_name: Some("Иванович".to_string()),
            display_name: "Иванов Иван Иванович".to_string(),
        }
    }

    #[tokio::test]
    async fn create_user_with_person_persists_both_and_events() {
        let (repo, store) = mem_repo().await;
        let user = sample_user(Uuid::new_v4());
        let person = sample_person(user.id);
        let events = vec![
            event(StreamType::User, &user.id.to_string(), "user.created", "c1"),
            event(
                StreamType::Person,
                &person.id.to_string(),
                "person.created",
                "c1",
            ),
        ];
        repo.create(&user, &person, &events).await.unwrap();

        let got = repo.get(&user.id).await.unwrap();
        assert_eq!(got.login, "ivanov");
        assert_eq!(got.status, core_domain::user::UserStatus::Active);
        assert_eq!(got.person_id, Some(user.id));

        assert_eq!(repo.get_by_login("ivanov").await.unwrap().id, user.id);
        assert_eq!(repo.list().await.unwrap().len(), 1);

        let person_got = repo.get_person(&user.id).await.unwrap();
        assert_eq!(person_got.display_name, "Иванов Иван Иванович");

        let user_stream = store
            .read_stream(StreamType::User, &user.id.to_string())
            .await
            .unwrap();
        assert_eq!(user_stream.len(), 1);
        assert_eq!(user_stream[0].version, 1);
        let person_stream = store
            .read_stream(StreamType::Person, &person.id.to_string())
            .await
            .unwrap();
        assert_eq!(person_stream.len(), 1);
        assert_eq!(person_stream[0].version, 1);
    }

    #[tokio::test]
    async fn duplicate_login_rejected() {
        let (repo, _) = mem_repo().await;
        let user1 = sample_user(Uuid::new_v4());
        let person1 = sample_person(user1.id);
        repo.create(
            &user1,
            &person1,
            &[event(StreamType::User, &user1.id.to_string(), "user.created", "c1")],
        )
        .await
        .unwrap();

        let user2 = User {
            login: "ivanov".to_string(),
            ..sample_user(Uuid::new_v4())
        };
        let person2 = sample_person(user2.id);
        let err = repo
            .create(
                &user2,
                &person2,
                &[event(StreamType::User, &user2.id.to_string(), "user.created", "c1")],
            )
            .await;
        assert!(matches!(err, Err(DomainError::ValidationError(_))));
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn update_user_persists_and_appends_event() {
        let (repo, store) = mem_repo().await;
        let user = sample_user(Uuid::new_v4());
        let person = sample_person(user.id);
        repo.create(
            &user,
            &person,
            &[event(StreamType::User, &user.id.to_string(), "user.created", "c1")],
        )
        .await
        .unwrap();

        let updated = User {
            status: core_domain::user::UserStatus::Disabled,
            must_change_password: true,
            ..user.clone()
        };
        repo.update(
            &updated,
            &[event(StreamType::User, &user.id.to_string(), "user.updated", "c1")],
        )
        .await
        .unwrap();

        let got = repo.get(&user.id).await.unwrap();
        assert_eq!(got.status, core_domain::user::UserStatus::Disabled);
        assert!(got.must_change_password);

        let stream = store
            .read_stream(StreamType::User, &user.id.to_string())
            .await
            .unwrap();
        assert_eq!(stream.len(), 2);
        assert_eq!(stream[1].version, 2);
    }

    #[tokio::test]
    async fn contacts_profiles_certificates_round_trip() {
        let (repo, store) = mem_repo().await;
        let user = sample_user(Uuid::new_v4());
        let person = sample_person(user.id);
        repo.create(
            &user,
            &person,
            &[event(StreamType::User, &user.id.to_string(), "user.created", "c1")],
        )
        .await
        .unwrap();

        let contact = UserContact {
            id: Uuid::new_v4(),
            user_id: user.id,
            channel_type: ContactChannelType::Email,
            value: "ivan@example.com".to_string(),
            is_primary: true,
            is_verified: false,
            purposes: vec![ContactPurpose::Login, ContactPurpose::Notification],
        };
        repo.add_contact(
            &contact,
            &[event(
                StreamType::UserContact,
                &contact.id.to_string(),
                "user_contact.created",
                "c1",
            )],
        )
        .await
        .unwrap();

        let profile = UserCompanyProfile {
            id: Uuid::new_v4(),
            user_id: user.id,
            company_id: Uuid::new_v4(),
            employee_number: Some("0001".to_string()),
            position: Some("Бухгалтер".to_string()),
            department: None,
            is_primary: true,
            is_active: true,
            valid_from: None,
            valid_to: None,
        };
        repo.add_profile(
            &profile,
            &[event(
                StreamType::UserProfile,
                &profile.id.to_string(),
                "user_profile.created",
                "c1",
            )],
        )
        .await
        .unwrap();

        let certificate = UserCertificate {
            id: Uuid::new_v4(),
            user_id: user.id,
            provider_code: "cryptopro".to_string(),
            certificate_ref: "ref-1".to_string(),
            subject: "CN=Ivanov".to_string(),
            issuer: "CN=CA".to_string(),
            fingerprint: "AA:BB:CC".to_string(),
            is_active: true,
        };
        repo.add_certificate(
            &certificate,
            &[event(
                StreamType::UserCert,
                &certificate.id.to_string(),
                "user_cert.created",
                "c1",
            )],
        )
        .await
        .unwrap();

        assert_eq!(repo.list_contacts(&user.id).await.unwrap().len(), 1);
        assert_eq!(repo.list_profiles(&user.id).await.unwrap().len(), 1);
        assert_eq!(repo.list_certificates(&user.id).await.unwrap().len(), 1);

        let contact_stream = store
            .read_stream(StreamType::UserContact, &contact.id.to_string())
            .await
            .unwrap();
        assert_eq!(contact_stream[0].version, 1);
        let profile_stream = store
            .read_stream(StreamType::UserProfile, &profile.id.to_string())
            .await
            .unwrap();
        assert_eq!(profile_stream[0].version, 1);
        let cert_stream = store
            .read_stream(StreamType::UserCert, &certificate.id.to_string())
            .await
            .unwrap();
        assert_eq!(cert_stream[0].version, 1);
    }

    #[tokio::test]
    async fn get_missing_user_returns_not_found() {
        let (repo, _) = mem_repo().await;
        let err = repo.get(&Uuid::new_v4()).await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
        let err = repo.get_by_login("ghost").await;
        assert!(matches!(err, Err(DomainError::NotFound(_))));
    }
}