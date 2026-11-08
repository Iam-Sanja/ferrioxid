# Timestamps

[Overview](index.md) · Components: [model-components.md](model-components.md)

`ferrioxid::with_timestamps!` attaches `created_at` and `updated_at` as `DateTime<Utc>` to a concrete model.

```rust
use ferrioxid::prelude::*;

ferrioxid::with_timestamps! {
    #[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
    #[ferrioxid(table = "categories", api = "/api/v1/categories", allow_anonymous = true)]
    pub struct Category {
        #[field(primary_key, auto)]
        pub id: Uuid,
        #[field(required, min_length = 1)]
        pub title: String,
    }
}
```

Both fields are `#[field(managed = "created")]` and `"updated"`. The repository sets it with `ferrioxid::core::utc_now()`. `created_at` remains unchanged during an update. `updated_at` is set for every successful update, even if the remaining fields remain the same.

These fields are omitted from `CategoryCreate` and `CategoryUpdate`. With `deny_unknown_fields`, a JSON body containing `created_at` is rejected. Managed fields are read-only in the admin, and `reject_locked` rejects submitted values.

The clock uses `Utc::now()` unless a test sets `ferrioxid::core::set_test_clock` for the current thread.

`HasTimestamps` provides `created_at()` and `updated_at()` if both roles are present.

`ferrioxid generate model Product --timestamps` wraps the new model in `with_timestamps!`. Without the flag, the existing model template is used. Existing files are never overwritten.

JSON remains flat: `created_at` and `updated_at` stand alongside `title`.
