//! Argon2id im PHC-Format. Die Parameter stehen im Hash und bleiben prüfbar.

use crate::error::AuthError;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use std::sync::OnceLock;

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = hasher()?
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AuthError::Validation("password could not be hashed".into()))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, encoded: &str) -> Result<bool, AuthError> {
    let parsed = PasswordHash::new(encoded)
        .map_err(|_| AuthError::Validation("password hash is invalid".into()))?;
    Ok(hasher()?
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Gleiche Rechenzeit, wenn die E-Mail nicht existiert.
pub fn verify_against_dummy(password: &str) {
    let dummy = dummy_hash();
    let _ = verify_password(password, dummy);
}

fn hasher() -> Result<Argon2<'static>, AuthError> {
    let params = Params::new(19_456, 2, 1, None)
        .map_err(|_| AuthError::Validation("argon2 parameters are invalid".into()))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password("ferrum-timing-placeholder").unwrap_or_else(|_| String::new())
    })
}

pub fn validate_new_password(password: &str) -> Result<(), AuthError> {
    if password.chars().count() < 12 || password.chars().count() > 128 {
        return Err(AuthError::Validation(
            "password must contain between 12 and 128 characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{hash_password, validate_new_password, verify_password};

    #[test]
    fn correct_and_wrong_passwords_use_different_salts() {
        let first = hash_password("correct-horse-battery").expect("hash");
        let second = hash_password("correct-horse-battery").expect("hash");
        assert_ne!(first, second);
        assert!(first.starts_with("$argon2id$"));
        assert!(verify_password("correct-horse-battery", &first).unwrap());
        assert!(!verify_password("wrong-horse-battery", &first).unwrap());
        assert!(verify_password("correct-horse-battery", &second).unwrap());
    }

    #[test]
    fn invalid_hash_is_rejected() {
        let error = verify_password("correct-horse-battery", "not-a-hash").unwrap_err();
        assert!(error.to_string().contains("invalid"));
    }

    #[test]
    fn password_change_replaces_the_hash() {
        let current = hash_password("current-password-1").expect("hash");
        validate_new_password("short").unwrap_err();
        let updated = hash_password("updated-password-1").expect("hash");
        assert!(verify_password("current-password-1", &current).unwrap());
        assert!(!verify_password("current-password-1", &updated).unwrap());
        assert!(verify_password("updated-password-1", &updated).unwrap());
    }
}
