//! Fehler der Authentifizierung. HTTP-Antworten übernehmen nur die festen Texte.

use ferrum_core::FerrumError;

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
    #[error("{0}")]
    Validation(String),
    #[error("database operation failed")]
    Database(String),
}

impl AuthError {
    pub fn database(error: impl std::fmt::Display) -> Self {
        let text = error.to_string();
        tracing::error!(error = %text, "auth database operation failed");
        Self::Database(text)
    }
}

impl From<AuthError> for FerrumError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::InvalidCredentials => {
                FerrumError::Unauthorized("Invalid email or password".into())
            }
            AuthError::Unauthenticated => {
                FerrumError::Unauthorized("Authentication required".into())
            }
            AuthError::Forbidden => FerrumError::Forbidden,
            AuthError::CsrfRejected => {
                FerrumError::Denied("csrf token is missing or invalid".into())
            }
            AuthError::OriginRejected => FerrumError::Denied("origin is not allowed".into()),
            AuthError::Validation(message) => FerrumError::Validation(message),
            AuthError::RateLimited | AuthError::Database(_) => {
                FerrumError::Denied("request rejected".into())
            }
        }
    }
}
