//! Konfiguration für einen späteren S3-kompatiblen Speicher.
//!
//! Es gibt keinen Netzwerkclient. `prepare` prüft nur die Form und liest
//! Zugangsdaten aus den genannten Umgebungsvariablen, ohne sie zu protokollieren.

use crate::StorageError;

pub const ACCESS_KEY_ENV: &str = "FERRIOXID_S3_ACCESS_KEY";
pub const SECRET_KEY_ENV: &str = "FERRIOXID_S3_SECRET_KEY";

/// Geprüfte Zielangabe. Geheimnisse stehen hier nicht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Plan {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_env: String,
    pub secret_key_env: String,
}

/// Kurzlebige Zugangsdaten. `Debug` zeigt die Werte nicht.
pub struct S3Credentials {
    pub access_key: String,
    pub secret_key: String,
}

impl std::fmt::Debug for S3Credentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3Credentials")
            .field("access_key", &"[redacted]")
            .field("secret_key", &"[redacted]")
            .finish()
    }
}

pub fn prepare(
    endpoint: &str,
    region: &str,
    bucket: &str,
    access_key_env: &str,
    secret_key_env: &str,
) -> Result<S3Plan, StorageError> {
    let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
    if !endpoint.starts_with("https://") || endpoint.contains([' ', '?', '#']) {
        return Err(StorageError::Failed);
    }
    if region.len() < 2
        || region.len() > 32
        || !region
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '-')
    {
        return Err(StorageError::Failed);
    }
    if bucket.len() < 3
        || bucket.len() > 63
        || bucket.contains("..")
        || bucket.contains(['/', '\\', ' '])
        || !bucket.chars().all(|char| {
            char.is_ascii_lowercase() || char.is_ascii_digit() || char == '-' || char == '.'
        })
    {
        return Err(StorageError::Failed);
    }
    if !env_name(access_key_env) || !env_name(secret_key_env) || access_key_env == secret_key_env {
        return Err(StorageError::Failed);
    }
    Ok(S3Plan {
        endpoint,
        region: region.to_owned(),
        bucket: bucket.to_owned(),
        access_key_env: access_key_env.to_owned(),
        secret_key_env: secret_key_env.to_owned(),
    })
}

pub fn credentials_from_env(plan: &S3Plan) -> Result<S3Credentials, StorageError> {
    let access_key = std::env::var(&plan.access_key_env).map_err(|_| StorageError::Failed)?;
    let secret_key = std::env::var(&plan.secret_key_env).map_err(|_| StorageError::Failed)?;
    if access_key.len() < 16
        || secret_key.len() < 16
        || access_key.chars().any(char::is_control)
        || secret_key.chars().any(char::is_control)
    {
        return Err(StorageError::Failed);
    }
    Ok(S3Credentials {
        access_key,
        secret_key,
    })
}

fn env_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_uppercase())
        && chars.all(|char| char.is_ascii_uppercase() || char.is_ascii_digit() || char == '_')
        && (3..=64).contains(&name.len())
}

#[cfg(test)]
mod tests {
    use super::{credentials_from_env, prepare, S3Credentials};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn remote_plan_rejects_plaintext_endpoints_and_does_not_connect() {
        assert!(prepare(
            "http://storage.example",
            "eu-central-1",
            "media",
            "FERRIOXID_S3_ACCESS_KEY",
            "FERRIOXID_S3_SECRET_KEY"
        )
        .is_err());
        assert!(prepare(
            "https://storage.example",
            "eu-central-1",
            "../media",
            "FERRIOXID_S3_ACCESS_KEY",
            "FERRIOXID_S3_SECRET_KEY"
        )
        .is_err());
        let plan = prepare(
            "https://storage.example",
            "eu-central-1",
            "app-media",
            "FERRIOXID_TEST_S3_ACCESS",
            "FERRIOXID_TEST_S3_SECRET",
        )
        .expect("plan");
        assert_eq!(plan.endpoint, "https://storage.example");
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::remove_var("FERRIOXID_TEST_S3_ACCESS");
        std::env::remove_var("FERRIOXID_TEST_S3_SECRET");
        assert!(credentials_from_env(&plan).is_err());
        std::env::set_var("FERRIOXID_TEST_S3_ACCESS", "test-access-key-value");
        std::env::set_var("FERRIOXID_TEST_S3_SECRET", "test-secret-key-value");
        let credentials = credentials_from_env(&plan).expect("env");
        let rendered = format!("{credentials:?}");
        assert!(!rendered.contains("test-access-key-value"));
        assert!(!rendered.contains("test-secret-key-value"));
        assert!(rendered.contains("[redacted]"));
        let explicit = S3Credentials {
            access_key: "test-access-key-value".into(),
            secret_key: "test-secret-key-value".into(),
        };
        assert!(!format!("{explicit:?}").contains("test-access"));
        std::env::remove_var("FERRIOXID_TEST_S3_ACCESS");
        std::env::remove_var("FERRIOXID_TEST_S3_SECRET");
    }
}
