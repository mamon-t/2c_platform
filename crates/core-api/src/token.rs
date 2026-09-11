//! JWT-менеджер токенов для транспортного слоя (Фаза 10b).
//!
//! Токен подписывается секретом (HS256). В claims копируются поля `ActorSnapshot`,
//! чтобы `parse` восстанавливал исполнителя без обращения к БД.

use std::time::Duration;

use chrono::Utc;
use core_application::ports::{AuthToken, TokenManager};
use core_domain::error::DomainError;
use core_domain::event::ActorSnapshot;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

/// Конфигурация JWT-токенов.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Секрет подписи HS256 (из окружения, не хардкодить).
    pub secret: String,
    /// Время жизни access-токена.
    pub access_ttl: Duration,
}

/// Claims, зашитые в токен.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Option<String>,
    login: String,
    full_name: String,
    position: Option<String>,
    company_id: Option<String>,
    exp: usize,
    iat: usize,
}

/// Менеджер токенов на основе JWT HS256.
pub struct JwtTokenManager {
    config: JwtConfig,
}

impl JwtTokenManager {
    /// Создаёт менеджер с заданной конфигурацией.
    pub fn new(config: JwtConfig) -> Self {
        Self { config }
    }
}

impl TokenManager for JwtTokenManager {
    fn issue(&self, actor: &ActorSnapshot) -> Result<AuthToken, DomainError> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::from_std(self.config.access_ttl).unwrap_or_default();
        let claims = Claims {
            sub: actor.user_id.map(|id| id.to_string()),
            login: actor.login.clone(),
            full_name: actor.full_name.clone(),
            position: actor.position.clone(),
            company_id: actor.company_id.map(|id| id.to_string()),
            exp: expires_at.timestamp() as usize,
            iat: now.timestamp() as usize,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.secret.as_bytes()),
        )
        .map_err(|e| DomainError::Storage(format!("подпись токена: {e}")))
        .map(|access_token| AuthToken {
            access_token,
            expires_at,
            token_type: "Bearer",
        })
    }

    fn parse(&self, token: &str) -> Result<ActorSnapshot, DomainError> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| DomainError::PermissionDenied(format!("некорректный или просроченный токен: {e}")))?;
        let claims = data.claims;
        Ok(ActorSnapshot {
            user_id: claims.sub.and_then(|sub| Uuid::parse_str(&sub).ok()),
            login: claims.login,
            full_name: claims.full_name,
            position: claims.position,
            company_id: claims.company_id.and_then(|id| Uuid::parse_str(&id).ok()),
            ip_address: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> JwtTokenManager {
        JwtTokenManager::new(JwtConfig {
            secret: "test-secret".to_string(),
            access_ttl: Duration::from_secs(600),
        })
    }

    #[tokio::test]
    async fn round_trip_restores_actor() {
        let manager = manager();
        let actor = ActorSnapshot {
            user_id: Some(Uuid::new_v4()),
            login: "alice".to_string(),
            full_name: "Алиса".to_string(),
            position: Some("Бухгалтер".to_string()),
            company_id: Some(Uuid::new_v4()),
            ip_address: None,
        };
        let token = manager.issue(&actor).unwrap();
        assert_eq!(token.token_type, "Bearer");
        assert!(token.expires_at > Utc::now());
        let restored = manager.parse(&token.access_token).unwrap();
        assert_eq!(restored.user_id, actor.user_id);
        assert_eq!(restored.login, "alice");
        assert_eq!(restored.full_name, "Алиса");
        assert_eq!(restored.position, actor.position);
        assert_eq!(restored.company_id, actor.company_id);
    }

    #[tokio::test]
    async fn system_token_parses_without_user_id() {
        let manager = manager();
        let actor = ActorSnapshot::system();
        let token = manager.issue(&actor).unwrap();
        let restored = manager.parse(&token.access_token).unwrap();
        assert!(restored.is_system());
        assert_eq!(restored.login, "system");
    }

    #[tokio::test]
    async fn wrong_secret_and_malformed_token_are_rejected() {
        let manager = manager();
        let other = JwtTokenManager::new(JwtConfig {
            secret: "other-secret".to_string(),
            access_ttl: Duration::from_secs(600),
        });
        let token = manager.issue(&ActorSnapshot::system()).unwrap();
        assert!(other.parse(&token.access_token).is_err());
        assert!(manager.parse("не-токен").is_err());
    }
}