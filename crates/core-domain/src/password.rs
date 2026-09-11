//! Хэширование и проверка паролей (Argon2id), ТЗ v3.1, §10b.
//!
//! Пароли никогда не хранятся в открытом виде: `User::password_hash` держит
//! Argon2id-хэш (формат PHC-строки), получаемый здесь с уникальной солью.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

/// Хэширует пароль в Argon2id PHC-строку с уникальной солью.
///
/// # Errors
///
/// Возвращает `Err` только при внутреннем сбое Argon2 (выделение памяти).
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| format!("не удалось хэшировать пароль: {e}"))
}

/// Проверяет пароль против сохранённого Argon2id-хэша.
/// Невалидный хэш (не PHC) трактуется как несовпадение, а не как ошибка.
pub fn verify_password(password: &str, password_hash: &str) -> bool {
    PasswordHash::new(password_hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_round_trip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert_ne!(hash, "correct horse battery staple");
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn hashes_are_salted() {
        let a = hash_password("secret").unwrap();
        let b = hash_password("secret").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn invalid_hash_is_rejected() {
        assert!(!verify_password("secret", "not-a-phc-hash"));
    }
}