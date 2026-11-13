//! Gemeinsame Schnittstelle für Prozessspeicher und einen späteren externen Speicher.
//!
//! Ein Redis- oder anderer Netzwerktreiber ist nicht enthalten. Zwei Prozesse teilen
//! einen [`crate::MemoryCache`] nicht.

use crate::error::CacheError;
use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait CacheBackend: Send + Sync {
    async fn get_raw(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, CacheError>;

    async fn put_raw(
        &self,
        namespace: &str,
        key: &str,
        bytes: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError>;

    async fn delete_raw(&self, namespace: &str, key: &str) -> Result<(), CacheError>;

    async fn invalidate_namespace(&self, namespace: &str) -> Result<(), CacheError>;
}

pub(crate) fn validate_namespace(namespace: &str) -> Result<(), CacheError> {
    let mut chars = namespace.chars();
    let ok = matches!(chars.next(), Some(char) if char.is_ascii_alphanumeric())
        && chars
            .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-' || char == '.')
        && (1..=64).contains(&namespace.len());
    if ok {
        Ok(())
    } else {
        Err(CacheError::InvalidKey)
    }
}

pub(crate) fn validate_key(key: &str) -> Result<(), CacheError> {
    let ok = !key.is_empty()
        && key.len() <= 180
        && !key.contains("..")
        && key
            .chars()
            .all(|char| char.is_ascii_graphic() && char != ' ' && char != '\\');
    if ok {
        Ok(())
    } else {
        Err(CacheError::InvalidKey)
    }
}
