//! Fehler der Authentifizierung. HTTP-Antworten übernehmen nur die festen Texte.

use ferrioxid_core::FerrioxidError;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid email or password")]
    InvalidCredentials,
    #[error("authentication required")]
    Unauthenticated,
    #[error("permission denied")]
    Forbidden,
    #[error("too many login attempts")]
    RateLimited,
    #[error("origin is not allowed")]
    OriginRejected,
    #[error("csrf token is missing or invalid")]
    CsrfRejected,
    #[error("multi-factor authentication is required")]
    MfaPending(PendingLogin),
    #[error("{0}")]
    Validation(String),
    #[error("database operation failed")]
    Database(String),
}

/// Einmaliger zweiter Schritt. Der Wert ist kein Sitzungstoken.
pub struct PendingLogin {
    pub challenge: String,
    pub expires_at: i64,
}

impl std::fmt::Debug for PendingLogin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PendingLogin")
            .field("challenge", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl AuthError {
    pub fn database(error: impl std::fmt::Display) -> Self {
        let text = error.to_string();
        tracing::error!(error = %text, "auth database operation failed");
        Self::Database(text)
    }
}

impl From<AuthError> for FerrioxidError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::InvalidCredentials => {
                FerrioxidError::Unauthorized("Invalid email or password".into())
            }
            AuthError::Unauthenticated => {
                FerrioxidError::Unauthorized("Authentication required".into())
            }
            AuthError::Forbidden => FerrioxidError::Forbidden,
            AuthError::CsrfRejected => {
                FerrioxidError::Denied("csrf token is missing or invalid".into())
            }
            AuthError::OriginRejected => FerrioxidError::Denied("origin is not allowed".into()),
            AuthError::Validation(message) => FerrioxidError::Validation(message),
            AuthError::RateLimited | AuthError::Database(_) | AuthError::MfaPending(_) => {
                FerrioxidError::Denied("request rejected".into())
            }
        }
    }
}
