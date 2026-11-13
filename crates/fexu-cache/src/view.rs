//! Expliziter Fragment-Cache. Er hängt sich nicht an Templates.

use crate::backend::CacheBackend;
use crate::error::CacheError;
use crate::typed::TypedCache;
use std::future::Future;
use std::time::Duration;

pub struct FragmentCache<B> {
    cache: TypedCache<B>,
    namespace: String,
}

impl<B: CacheBackend> FragmentCache<B> {
    pub fn new(cache: TypedCache<B>, namespace: &str) -> Result<Self, CacheError> {
        let namespace = format!("view.{namespace}");
        crate::backend::validate_namespace(&namespace)?;
        Ok(Self { cache, namespace })
    }

    pub async fn render<F, Fut>(
        &self,
        key: &str,
        ttl: Duration,
        produce: F,
    ) -> Result<String, CacheError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<String, CacheError>>,
    {
        self.cache
            .get_or_set(&self.namespace, key, Some(ttl), produce)
            .await
    }

    pub async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        self.cache.delete(&self.namespace, key).await
    }

    pub async fn invalidate_all(&self) -> Result<(), CacheError> {
        self.cache.invalidate_namespace(&self.namespace).await
    }
}
