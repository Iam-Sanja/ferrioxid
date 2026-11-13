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
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).expect("dummy password entropy");
        let password: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        hash_password(&password).expect("dummy password hash")
    })
}

#[cfg(test)]
pub(crate) fn ephemeral_password() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("test entropy");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
pub(crate) fn ephemeral_weak_password() -> String {
    let mut bytes = [0u8; 4];
    getrandom::getrandom(&mut bytes).expect("test entropy");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
    use super::{
        dummy_hash, ephemeral_password, ephemeral_weak_password, hash_password,
        validate_new_password, verify_against_dummy, verify_password,
    };

    #[test]
    fn correct_and_wrong_passwords_use_different_salts() {
        let password = ephemeral_password();
        let wrong = ephemeral_password();
        let first = hash_password(&password).expect("hash");
        let second = hash_password(&password).expect("hash");
        assert_ne!(first, second);
        assert!(first.starts_with("$argon2id$"));
        assert!(first.contains("m=19456"));
        assert!(verify_password(&password, &first).unwrap());
        assert!(!verify_password(&wrong, &first).unwrap());
        assert!(verify_password(&password, &second).unwrap());
    }

    #[test]
    fn invalid_hash_is_rejected() {
        let error = verify_password(&ephemeral_password(), "not-a-hash").unwrap_err();
        assert!(error.to_string().contains("invalid"));
    }

    #[test]
    fn password_change_replaces_the_hash() {
        let current_password = ephemeral_password();
        let updated_password = ephemeral_password();
        let current = hash_password(&current_password).expect("hash");
        validate_new_password(&ephemeral_weak_password()).unwrap_err();
        validate_new_password(&ephemeral_password()).unwrap();
        let updated = hash_password(&updated_password).expect("hash");
        assert!(verify_password(&current_password, &current).unwrap());
        assert!(!verify_password(&current_password, &updated).unwrap());
        assert!(verify_password(&updated_password, &updated).unwrap());
    }

    #[test]
    fn dummy_hash_stays_one_argon2id_value() {
        let first = dummy_hash();
        let second = dummy_hash();
        assert_eq!(first, second);
        assert!(first.starts_with("$argon2id$"));
        assert!(first.contains("m=19456,t=2,p=1"));
        let password = ephemeral_password();
        verify_against_dummy(&password);
        verify_against_dummy(&ephemeral_password());
        assert!(!verify_password(&password, first).unwrap());
    }
}
