//! Fehler der Cache-Schicht. Texte enthalten keine gespeicherten Werte.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CacheError {
    #[error("cache key is invalid")]
    InvalidKey,
    #[error("cache value exceeds the memory limit")]
    TooLarge,
    #[error("cache value could not be serialized")]
    Serialization,
    #[error("cached value does not match the expected version")]
    VersionMismatch,
    #[error("cached value is corrupt")]
    Corrupt,
    #[error("cache backend failed")]
    Backend,
}
