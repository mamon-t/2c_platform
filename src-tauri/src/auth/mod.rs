use crate::core::*;
use chrono::{DateTime, Utc, Duration};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub company_id: String,
    pub role_id: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub _id: Id,
    pub company_id: CompanyId,
    pub username: String,
    pub display_name: String,
    pub password_hash: String,
    pub role_id: RoleId,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub _id: Id,
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct AuthService {
    jwt_secret: String,
    token_ttl_hours: i64,
}

impl AuthService {
    pub fn new(jwt_secret: &str) -> Self {
        Self {
            jwt_secret: jwt_secret.to_string(),
            token_ttl_hours: 24,
        }
    }

    pub fn create_token(
        &self,
        user_id: &UserId,
        company_id: &CompanyId,
        role_id: &RoleId,
    ) -> PlatformResult<String> {
        let now = Utc::now();
        let exp = (now + Duration::hours(self.token_ttl_hours)).timestamp() as usize;

        let claims = Claims {
            sub: user_id.0.to_string(),
            company_id: company_id.0.to_string(),
            role_id: role_id.0.to_string(),
            exp,
            iat: now.timestamp() as usize,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| PlatformError::Auth(format!("Ошибка создания токена: {e}")))
    }

    pub fn verify_token(&self, token: &str) -> PlatformResult<Claims> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(|e| PlatformError::Auth(format!("Невалидный токен: {e}")))
    }

    pub fn hash_password(&self, password: &str) -> PlatformResult<String> {
        use argon2::password_hash::{SaltString, PasswordHasher};
        use argon2::Argon2;

        let salt = SaltString::generate(&mut rand::rngs::OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| PlatformError::Auth(format!("Ошибка хеширования: {e}")))
    }

    pub fn verify_password(
        &self,
        password: &str,
        hash: &str,
    ) -> PlatformResult<bool> {
        use argon2::password_hash::{PasswordHash, PasswordVerifier};
        use argon2::Argon2;

        let parsed = PasswordHash::new(hash)
            .map_err(|e| PlatformError::Auth(format!("Невалидный хеш: {e}")))?;

        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }
}
