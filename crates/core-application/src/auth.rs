//! Аутентификация и сессии (ТЗ v3.1, §10b): логин по паролю Argon2id, выпуск
//! JWT, аудит `user.login`/`user.login_failed`/`user.logout` и политика
//! «5 неудачных входов → блокировка на 15 минут».

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use core_domain::audit::{AuditEntry, AuditResult, AuditTarget};
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::password::verify_password;
use core_domain::user::User;
use serde_json::json;
use uuid::Uuid;

use crate::ports::{AuditRepository, AuthToken, TokenManager, UserRepository};

/// Число неудачных входов до временной блокировки учётной записи.
const MAX_FAILED_LOGIN_ATTEMPTS: u32 = 5;
/// Длительность временной блокировки после серии неудачных входов.
const LOCK_DURATION: Duration = Duration::from_secs(15 * 60);

/// Общее сообщение для всех неудач аутентификации: не раскрывает, существует
/// ли учётная запись или неверен только пароль.
const LOGIN_FAILED_MESSAGE: &str = "неверный логин или пароль";

/// Сервис аутентификации: проверяет учётные данные, выпускает токены и пишет
/// операционный аудит.
pub struct AuthService {
    users: Arc<dyn UserRepository>,
    audit: Arc<dyn AuditRepository>,
    tokens: Arc<dyn TokenManager>,
}

impl AuthService {
    pub fn new(
        users: Arc<dyn UserRepository>,
        audit: Arc<dyn AuditRepository>,
        tokens: Arc<dyn TokenManager>,
    ) -> Self {
        Self { users, audit, tokens }
    }

    /// Аутентифицирует пользователя по логину и паролю.
    ///
    /// При успехе сбрасывается счётчик неудачных входов и возвращается токен
    /// доступа; при неудаче пишется аудит `user.login_failed` и при превышении
    /// лимита учётная запись блокируется через `locked_until`.
    ///
    /// # Errors
    ///
    /// Все неудачи возвращают `DomainError::ValidationError(LOGIN_FAILED_MESSAGE)`,
    /// чтобы не раскрывать существование учётной записи.
    pub async fn login(
        &self,
        login: &str,
        password: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthToken, DomainError> {
        let user = match self.users.get_by_login(login).await {
            Ok(user) => user,
            Err(_) => {
                self.log_failed_login(login, "неверный логин или пароль", ip_address.clone(), user_agent.clone())
                    .await;
                return Err(DomainError::ValidationError(LOGIN_FAILED_MESSAGE.to_string()));
            }
        };

        let (_, company_id) = self.primary_profile(&user).await;
        match user.status {
            core_domain::user::UserStatus::Disabled => {
                self.log_failed_login_for_user(&user, company_id, "учётная запись отключена", ip_address, user_agent)
                    .await;
                return Err(DomainError::ValidationError(LOGIN_FAILED_MESSAGE.to_string()));
            }
            core_domain::user::UserStatus::Archived => {
                self.log_failed_login_for_user(&user, company_id, "учётная запись архивирована", ip_address, user_agent)
                    .await;
                return Err(DomainError::ValidationError(LOGIN_FAILED_MESSAGE.to_string()));
            }
            core_domain::user::UserStatus::Locked => {
                self.log_failed_login_for_user(&user, company_id, "учётная запись заблокирована", ip_address, user_agent)
                    .await;
                return Err(DomainError::ValidationError(LOGIN_FAILED_MESSAGE.to_string()));
            }
            _ => {}
        }

        if user.locked_until.map(|until| until > Utc::now()).unwrap_or(false) {
            self.log_failed_login_for_user(&user, company_id, "учётная запись временно заблокирована", ip_address, user_agent)
                .await;
            return Err(DomainError::ValidationError(LOGIN_FAILED_MESSAGE.to_string()));
        }

        if !verify_password(password, &user.password_hash) {
            let failed = user.failed_login_count + 1;
            let locked_until = if failed >= MAX_FAILED_LOGIN_ATTEMPTS {
                Some(Utc::now() + chrono::Duration::from_std(LOCK_DURATION).unwrap_or_default())
            } else {
                None
            };
            let mut updated = user.clone();
            updated.failed_login_count = if locked_until.is_some() { 0 } else { failed };
            updated.locked_until = locked_until;
            updated.updated_at = Utc::now();
            self.store_user(&updated).await;
            self.log_failed_login_for_user(&user, company_id, "неверный пароль", ip_address, user_agent)
                .await;
            return Err(DomainError::ValidationError(LOGIN_FAILED_MESSAGE.to_string()));
        }

        let mut updated = user.clone();
        updated.failed_login_count = 0;
        updated.locked_until = None;
        updated.updated_at = Utc::now();
        self.store_user(&updated).await;

        let full_name = self
            .users
            .get_person(&user.id)
            .await
            .map(|person| person.display_name)
            .unwrap_or_else(|_| user.login.clone());
        let (position, company_id) = self.primary_profile(&user).await;

        let actor = ActorSnapshot {
            user_id: Some(user.id),
            login: user.login.clone(),
            full_name,
            position,
            company_id,
            ip_address: ip_address.clone(),
        };
        let token = self.tokens.issue(&actor)?;

        let audit_entry = AuditEntry {
            id: Uuid::new_v4(),
            action: "user.login".to_string(),
            actor,
            target: Some(AuditTarget {
                entity_type: Some("user".to_string()),
                entity_id: Some(user.id),
                entity_code: Some(user.login.clone()),
                company_id,
            }),
            result: AuditResult::Success,
            details: Some(json!({ "token_ttl_sec": 0 })),
            ip_address,
            user_agent,
            company_id,
            timestamp: Utc::now(),
        };
        self.audit.log(audit_entry).await?;
        Ok(token)
    }

    /// Фиксирует выход пользователя из системы. Для командного слоя, где
    /// аутентифицированный исполнитель недоступен в обработчике, `actor`
    /// опционален (тогда логируется анонимный выход).
    pub async fn logout(
        &self,
        actor: Option<ActorSnapshot>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), DomainError> {
        let actor = actor.unwrap_or_else(ActorSnapshot::anonymous);
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            action: "user.logout".to_string(),
            actor: actor.clone(),
            target: Some(AuditTarget {
                entity_type: Some("user".to_string()),
                entity_id: actor.user_id,
                entity_code: Some(actor.login.clone()),
                company_id: actor.company_id,
            }),
            result: AuditResult::Success,
            details: None,
            ip_address,
            user_agent,
            company_id: actor.company_id,
            timestamp: Utc::now(),
        };
        self.audit.log(entry).await
    }

    /// Аудит неудачного входа для несуществующей учётной записи (без утечки,
    /// существовала ли она — `entity_id` остаётся `None`).
    async fn log_failed_login(
        &self,
        login: &str,
        reason: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) {
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            action: "user.login_failed".to_string(),
            actor: ActorSnapshot {
                user_id: None,
                login: login.to_string(),
                full_name: "Аутентификация".to_string(),
                position: None,
                company_id: None,
                ip_address: ip_address.clone(),
            },
            target: Some(AuditTarget {
                entity_type: Some("user".to_string()),
                entity_id: None,
                entity_code: Some(login.to_string()),
                company_id: None,
            }),
            result: AuditResult::Failure { reason: reason.to_string() },
            details: None,
            ip_address,
            user_agent,
            company_id: None,
            timestamp: Utc::now(),
        };
        let _ = self.audit.log(entry).await;
    }

    /// Аудит неудачного входа для существующей учётной записи.
    async fn log_failed_login_for_user(
        &self,
        user: &User,
        company_id: Option<Uuid>,
        reason: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) {
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            action: "user.login_failed".to_string(),
            actor: ActorSnapshot {
                user_id: None,
                login: user.login.clone(),
                full_name: "Аутентификация".to_string(),
                position: None,
                company_id,
                ip_address: ip_address.clone(),
            },
            target: Some(AuditTarget {
                entity_type: Some("user".to_string()),
                entity_id: Some(user.id),
                entity_code: Some(user.login.clone()),
                company_id,
            }),
            result: AuditResult::Failure { reason: reason.to_string() },
            details: None,
            ip_address,
            user_agent,
            company_id,
            timestamp: Utc::now(),
        };
        let _ = self.audit.log(entry).await;
    }

    /// Записывает обновления счётчика входа и блокировки в Трубу и на Доску.
    async fn store_user(&self, user: &User) {
        let event = Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::User,
            stream_id: user.id.to_string(),
            event_type: "user.updated".to_string(),
            version: 0,
            payload: json!({
                "failed_login_count": user.failed_login_count,
                "locked_until": user.locked_until,
                "updated_at": user.updated_at,
            }),
            metadata: ActorSnapshot::system(),
            company_id: String::new(),
            correlation_id: Uuid::new_v4().to_string(),
            causation_id: None,
            occurred_at: Utc::now(),
        };
        let _ = self.users.update(user, &[event]).await;
    }

    async fn primary_profile(&self, user: &User) -> (Option<String>, Option<Uuid>) {
        let Ok(profiles) = self.users.list_profiles(&user.id).await else {
            return (None, None);
        };
        let primary = profiles
            .iter()
            .find(|p| p.is_primary && p.is_active)
            .or_else(|| profiles.iter().find(|p| p.is_active))
            .or(profiles.first());
        match primary {
            Some(profile) => (profile.position.clone(), Some(profile.company_id)),
            None => (None, None),
        }
    }
}