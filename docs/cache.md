# Cache

[Overview](index.md)

**Implemented** as an optional, process-local cache in `ferrioxid-cache`. The facade re-exports it as `ferrioxid::cache`. There is no Redis client and no required extra service. An in-process cache is not a distributed cache. Two `MemoryCache` values do not share entries.

## API

`TypedCache::new(backend, version)` offers `get`, `set`, `delete`, `invalidate_namespace` and `get_or_set`. Keys are split into a namespace and a key. Values are JSON with a `FXC1` prefix and a `u32` version. A version mismatch or corrupt bytes is an error. The cache does not recompute silently.

`get_or_set` coalesces concurrent producers inside one process with `tokio::sync::Notify`. Waiters subscribe before they read the result, so a producer that finishes immediately cannot drop the wakeup. A second process, or a second `MemoryCache`, runs its own producer. Invalidating a namespace frees its bytes for later inserts. Eviction order is earliest expiry, then oldest insert time, then key order.

`MemoryLimits` rejects zero `max_entries` or `max_bytes`. The defaults used by callers should stay small. The crate tests use 1024 entries and 8 MiB as an upper example, not a requirement. TTL is an optional duration from an injectable millisecond clock.

`FragmentCache` uses the namespace `view.{name}`. Invalidating one fragment does not clear another. View caching is opt-in. There is no silent global cache.

`CacheBackend` is the extension point. Implementing it does not add a network client.

## What not to store

The cache does not filter secrets. Do not store passwords, session tokens, TOTP secrets, recovery codes or signed-download keys. Tenant scope is the namespace the caller chooses. A shared namespace mixes tenants.

Stale data is the caller's problem. Set a TTL or call `invalidate_namespace` after a write. A stampede inside one process is reduced by `get_or_set`. That does not coordinate other processes.

## Tests

`cargo test -p ferrioxid-cache` covers TTL, namespaces, version errors, memory bounds, separate instances, fragment invalidation and concurrent `get_or_set`.
