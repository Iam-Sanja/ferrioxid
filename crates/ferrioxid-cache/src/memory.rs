//! Begrenzter Prozessspeicher. Er ist kein gemeinsamer Cache mehrerer Instanzen.

use crate::backend::{validate_key, validate_namespace, CacheBackend};
use crate::error::CacheError;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLimits {
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_entries: 1024,
            max_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone)]
pub struct MemoryCache {
    inner: Arc<MemoryInner>,
}

struct MemoryInner {
    state: Mutex<MemoryState>,
    now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
}

struct MemoryState {
    entries: HashMap<(String, String), Slot>,
    bytes: usize,
    limits: MemoryLimits,
}

struct Slot {
    bytes: Vec<u8>,
    stored_at_ms: i64,
    expires_at_ms: Option<i64>,
}

impl MemoryCache {
    pub fn new(limits: MemoryLimits) -> Result<Self, CacheError> {
        Self::with_now(
            limits,
            Arc::new(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as i64)
                    .unwrap_or(0)
            }),
        )
    }

    pub fn with_now(
        limits: MemoryLimits,
        now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
    ) -> Result<Self, CacheError> {
        if limits.max_entries == 0 || limits.max_bytes == 0 {
            return Err(CacheError::TooLarge);
        }
        Ok(Self {
            inner: Arc::new(MemoryInner {
                state: Mutex::new(MemoryState {
                    entries: HashMap::new(),
                    bytes: 0,
                    limits,
                }),
                now_ms,
            }),
        })
    }

    fn now(&self) -> i64 {
        (self.inner.now_ms)()
    }
}

#[async_trait]
impl CacheBackend for MemoryCache {
    async fn get_raw(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        validate_namespace(namespace)?;
        validate_key(key)?;
        let mut state = lock(&self.inner.state);
        let map_key = (namespace.to_owned(), key.to_owned());
        let expired = state
            .entries
            .get(&map_key)
            .is_some_and(|slot| expired(slot, self.now()));
        if expired {
            remove_key(&mut state, &map_key);
            return Ok(None);
        }
        Ok(state.entries.get(&map_key).map(|slot| slot.bytes.clone()))
    }

    async fn put_raw(
        &self,
        namespace: &str,
        key: &str,
        bytes: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        validate_namespace(namespace)?;
        validate_key(key)?;
        if ttl.is_some_and(|ttl| ttl.is_zero()) {
            return Err(CacheError::InvalidKey);
        }
        let mut state = lock(&self.inner.state);
        if bytes.len() > state.limits.max_bytes {
            return Err(CacheError::TooLarge);
        }
        let map_key = (namespace.to_owned(), key.to_owned());
        remove_key(&mut state, &map_key);
        evict_expired(&mut state, self.now());
        while state.entries.len() >= state.limits.max_entries
            || state.bytes.saturating_add(bytes.len()) > state.limits.max_bytes
        {
            if !evict_one(&mut state) {
                return Err(CacheError::TooLarge);
            }
        }
        let now = self.now();
        let expires_at_ms = ttl.map(|ttl| now.saturating_add(millis(ttl)));
        state.bytes = state.bytes.saturating_add(bytes.len());
        state.entries.insert(
            map_key,
            Slot {
                bytes,
                stored_at_ms: now,
                expires_at_ms,
            },
        );
        Ok(())
    }

    async fn delete_raw(&self, namespace: &str, key: &str) -> Result<(), CacheError> {
        validate_namespace(namespace)?;
        validate_key(key)?;
        let mut state = lock(&self.inner.state);
        remove_key(&mut state, &(namespace.to_owned(), key.to_owned()));
        Ok(())
    }

    async fn invalidate_namespace(&self, namespace: &str) -> Result<(), CacheError> {
        validate_namespace(namespace)?;
        let mut state = lock(&self.inner.state);
        let keys: Vec<_> = state
            .entries
            .keys()
            .filter(|(entry_namespace, _)| entry_namespace == namespace)
            .cloned()
            .collect();
        for key in keys {
            remove_key(&mut state, &key);
        }
        Ok(())
    }
}

fn lock(mutex: &Mutex<MemoryState>) -> std::sync::MutexGuard<'_, MemoryState> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn expired(slot: &Slot, now: i64) -> bool {
    slot.expires_at_ms.is_some_and(|expires| expires <= now)
}

fn evict_expired(state: &mut MemoryState, now: i64) {
    let keys: Vec<_> = state
        .entries
        .iter()
        .filter(|(_, slot)| expired(slot, now))
        .map(|(key, _)| key.clone())
        .collect();
    for key in keys {
        remove_key(state, &key);
    }
}

fn evict_one(state: &mut MemoryState) -> bool {
    let Some(key) = state
        .entries
        .iter()
        .min_by(|(left_key, left), (right_key, right)| {
            (
                left.expires_at_ms.unwrap_or(i64::MAX),
                left.stored_at_ms,
                left_key,
            )
                .cmp(&(
                    right.expires_at_ms.unwrap_or(i64::MAX),
                    right.stored_at_ms,
                    right_key,
                ))
        })
        .map(|(key, _)| key.clone())
    else {
        return false;
    };
    remove_key(state, &key);
    true
}

fn remove_key(state: &mut MemoryState, key: &(String, String)) {
    if let Some(slot) = state.entries.remove(key) {
        state.bytes = state.bytes.saturating_sub(slot.bytes.len());
    }
}

fn millis(ttl: Duration) -> i64 {
    i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX).max(1)
}
