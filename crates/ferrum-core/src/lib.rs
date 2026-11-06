//! Shared contracts, independent of HTTP and database implementations.

#[derive(Debug, Clone, Copy)]
pub struct ModelDescriptor {
    pub name: &'static str,
    pub table: &'static str,
    pub api_path: &'static str,
    /// Explicit opt-in for unauthenticated access (demo only).
    pub allow_anonymous: bool,
}

pub trait FerrumModel: Send + Sync + 'static {
    fn descriptor() -> &'static ModelDescriptor;
    fn schema_sql() -> &'static str;
}

#[derive(Debug, thiserror::Error)]
pub enum FerrumError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("permission denied")]
    Forbidden,
    #[error("resource not found")]
    NotFound,
    #[error("database operation failed")]
    Database(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl FerrumError {
    pub fn database<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Database(Box::new(error))
    }
}
