//! Optionaler, typisierter Cache. Der mitgelieferte Speicher gilt nur im aktuellen Prozess.
//!
//! Ein externes Backend implementiert [`CacheBackend`]. Ferrioxid liefert keinen
//! Redis-Client und behandelt den Prozessspeicher nicht als verteilten Cache.

mod backend;
mod error;
mod memory;
mod typed;
mod view;

pub use backend::CacheBackend;
pub use error::CacheError;
pub use memory::{MemoryCache, MemoryLimits};
pub use typed::TypedCache;
pub use view::FragmentCache;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Note {
        label: String,
    }

    fn clock(now: Arc<AtomicU32>) -> Arc<dyn Fn() -> i64 + Send + Sync> {
        Arc::new(move || i64::from(now.load(Ordering::SeqCst)) * 1000)
    }

    fn open_cache(entries: usize, bytes: usize) -> TypedCache<MemoryCache> {
        TypedCache::new(
            MemoryCache::new(MemoryLimits {
                max_entries: entries,
                max_bytes: bytes,
            })
            .expect("limits"),
            1,
        )
    }

    #[tokio::test]
    async fn ttl_namespace_and_version_stay_isolated() {
        let now = Arc::new(AtomicU32::new(10));
        let memory = MemoryCache::with_now(
            MemoryLimits {
                max_entries: 8,
                max_bytes: 4096,
            },
            clock(now.clone()),
        )
        .expect("memory");
        let cache = TypedCache::new(memory, 1);
        let note = Note {
            label: "alpha".into(),
        };
        cache
            .set("tenant.a", "note", &note, Some(Duration::from_secs(5)))
            .await
            .expect("set");
        cache
            .set(
                "tenant.b",
                "note",
                &Note {
                    label: "beta".into(),
                },
                None,
            )
            .await
            .expect("other");
        let loaded: Note = cache
            .get("tenant.a", "note")
            .await
            .expect("get")
            .expect("hit");
        assert_eq!(loaded, note);
        now.store(16, Ordering::SeqCst);
        assert!(cache
            .get::<Note>("tenant.a", "note")
            .await
            .expect("expired")
            .is_none());
        let other: Note = cache
            .get("tenant.b", "note")
            .await
            .expect("kept")
            .expect("still there");
        assert_eq!(other.label, "beta");
        cache.invalidate_namespace("tenant.b").await.expect("drop");
        assert!(cache
            .get::<Note>("tenant.b", "note")
            .await
            .expect("gone")
            .is_none());

        let raw = b"FXC1\x00\x00\x00\x02{}".to_vec();
        cache
            .backend()
            .put_raw("tenant.a", "old", raw, None)
            .await
            .expect("raw");
        assert_eq!(
            cache.get::<Note>("tenant.a", "old").await,
            Err(CacheError::VersionMismatch)
        );
        cache
            .backend()
            .put_raw("tenant.a", "bad", b"nope".to_vec(), None)
            .await
            .expect("bad");
        assert_eq!(
            cache.get::<Note>("tenant.a", "bad").await,
            Err(CacheError::Corrupt)
        );
        assert_eq!(
            cache
                .set("bad key", "note", &note, None)
                .await
                .expect_err("namespace"),
            CacheError::InvalidKey
        );
    }

    #[tokio::test]
    async fn memory_bounds_and_separate_instances() {
        let cache = open_cache(2, 256);
        cache.set("app", "one", &"alpha", None).await.expect("one");
        cache.set("app", "two", &"beta", None).await.expect("two");
        cache
            .set("app", "three", &"gamma", None)
            .await
            .expect("three");
        let left: Option<String> = cache.get("app", "one").await.expect("read");
        let kept: String = cache
            .get("app", "three")
            .await
            .expect("read")
            .expect("kept");
        assert!(left.is_none());
        assert_eq!(kept, "gamma");
        let huge = "x".repeat(300);
        assert_eq!(
            cache.set("app", "huge", &huge, None).await,
            Err(CacheError::TooLarge)
        );
        let other = open_cache(4, 256);
        assert!(other
            .get::<String>("app", "three")
            .await
            .expect("other process")
            .is_none());
    }

    #[tokio::test]
    async fn concurrent_get_or_set_runs_the_producer_once() {
        let cache = Arc::new(open_cache(16, 4096));
        let calls = Arc::new(AtomicU32::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let cache = cache.clone();
            let calls = calls.clone();
            tasks.push(tokio::spawn(async move {
                cache
                    .get_or_set("app", "shared", Some(Duration::from_secs(30)), || {
                        let calls = calls.clone();
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(40)).await;
                            Ok(Note {
                                label: "once".into(),
                            })
                        }
                    })
                    .await
            }));
        }
        for task in tasks {
            let note = task.await.expect("join").expect("value");
            assert_eq!(note.label, "once");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fragment_cache_invalidates_only_its_namespace() {
        let memory = MemoryCache::new(MemoryLimits {
            max_entries: 16,
            max_bytes: 4096,
        })
        .expect("memory");
        let records = TypedCache::new(memory.clone(), 1);
        let fragments = FragmentCache::new(TypedCache::new(memory, 1), "home").expect("view");
        records
            .set("records", "home", &"data", None)
            .await
            .expect("data");
        let calls = AtomicU32::new(0);
        let first = fragments
            .render("index", Duration::from_secs(60), || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Ok("page".to_owned()) }
            })
            .await
            .expect("render");
        let second = fragments
            .render("index", Duration::from_secs(60), || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Ok("other".to_owned()) }
            })
            .await
            .expect("cached");
        assert_eq!(first, second);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        fragments.invalidate_all().await.expect("view namespace");
        let kept: String = records
            .get("records", "home")
            .await
            .expect("untouched")
            .expect("data remains");
        assert_eq!(kept, "data");
        let third = fragments
            .render("index", Duration::from_secs(60), || async {
                Ok("fresh".to_owned())
            })
            .await
            .expect("fresh");
        assert_eq!(third, "fresh");
    }

    #[tokio::test]
    async fn ttl_boundary_frees_bytes_and_eviction_is_deterministic() {
        let now = Arc::new(AtomicU32::new(10));
        let memory = MemoryCache::with_now(
            MemoryLimits {
                max_entries: 2,
                max_bytes: 48,
            },
            clock(now.clone()),
        )
        .expect("memory");
        let cache = TypedCache::new(memory, 1);
        let value = "x".repeat(20);
        cache
            .set("app", "live", &value, Some(Duration::from_secs(5)))
            .await
            .expect("set");
        now.store(15, Ordering::SeqCst);
        assert!(cache
            .get::<String>("app", "live")
            .await
            .expect("boundary")
            .is_none());
        cache
            .set("app", "after", &value, None)
            .await
            .expect("bytes released by expiry");

        let frozen = Arc::new(AtomicU32::new(5));
        let memory = MemoryCache::with_now(
            MemoryLimits {
                max_entries: 2,
                max_bytes: 4096,
            },
            clock(frozen),
        )
        .expect("frozen");
        let cache = TypedCache::new(memory, 1);
        cache.set("app", "b", &"beta", None).await.expect("b");
        cache.set("app", "c", &"gamma", None).await.expect("c");
        cache.set("app", "a", &"alpha", None).await.expect("a");
        assert!(cache
            .get::<String>("app", "b")
            .await
            .expect("evicted")
            .is_none());
        assert_eq!(
            cache
                .get::<String>("app", "c")
                .await
                .expect("kept")
                .unwrap(),
            "gamma"
        );
        cache.invalidate_namespace("app").await.expect("drop");
        cache
            .set("app", "again", &value, None)
            .await
            .expect("bytes released by invalidation");
        assert!(cache
            .get::<String>("app", "c")
            .await
            .expect("namespace")
            .is_none());
        let other = TypedCache::new(
            MemoryCache::new(MemoryLimits {
                max_entries: 2,
                max_bytes: 4096,
            })
            .expect("other"),
            1,
        );
        assert!(other
            .get::<String>("app", "again")
            .await
            .expect("isolated")
            .is_none());
    }

    #[tokio::test]
    async fn instant_get_or_set_does_not_drop_waiters() {
        let cache = Arc::new(open_cache(32, 8192));
        for round in 0..40 {
            let calls = Arc::new(AtomicU32::new(0));
            let mut tasks = Vec::new();
            for _ in 0..8 {
                let cache = cache.clone();
                let calls = calls.clone();
                let key = format!("fast-{round}");
                tasks.push(tokio::spawn(async move {
                    cache
                        .get_or_set("app", &key, None, || {
                            let calls = calls.clone();
                            async move {
                                calls.fetch_add(1, Ordering::SeqCst);
                                Ok::<_, CacheError>("ready".to_owned())
                            }
                        })
                        .await
                }));
            }
            let finished = tokio::time::timeout(Duration::from_secs(2), async {
                let mut values = Vec::new();
                for task in tasks {
                    values.push(task.await.expect("join").expect("value"));
                }
                values
            })
            .await
            .expect("waiters finished");
            assert!(finished.iter().all(|value| value == "ready"));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn parallel_reads_and_writes_release_bytes() {
        let cache = Arc::new(open_cache(64, 4096));
        let mut tasks = Vec::new();
        for index in 0..16 {
            let cache = cache.clone();
            tasks.push(tokio::spawn(async move {
                let key = format!("k{index}");
                cache
                    .set("hammer", &key, &format!("value-{index}"), None)
                    .await
                    .expect("set");
                let loaded: Option<String> = cache.get("hammer", &key).await.expect("get");
                assert!(loaded.is_some());
                cache.delete("hammer", &key).await.expect("delete");
            }));
        }
        for task in tasks {
            task.await.expect("join");
        }
        cache
            .invalidate_namespace("hammer")
            .await
            .expect("invalidate");
        for index in 0..16 {
            assert!(cache
                .get::<String>("hammer", &format!("k{index}"))
                .await
                .expect("cleared")
                .is_none());
        }
        let big = "x".repeat(2500);
        cache
            .set("hammer", "full", &big, None)
            .await
            .expect("fits after release");
        cache.delete("hammer", "full").await.expect("free");
        cache
            .set("hammer", "full", &big, None)
            .await
            .expect("reused");
        let other = open_cache(8, 4096);
        assert!(other
            .get::<String>("hammer", "full")
            .await
            .expect("isolated")
            .is_none());
    }
}
