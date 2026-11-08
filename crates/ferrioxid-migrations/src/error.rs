//! Fehler der Migrations-Engine. Die Texte sind für die CLI bestimmt.

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct MigrationError(pub String);

impl MigrationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
