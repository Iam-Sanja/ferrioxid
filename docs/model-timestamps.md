# Zeitstempel

[Zur Übersicht](index.md) · Komponenten: [model-components.md](model-components.md)

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

The fields are missing in `CategoryCreate` and `CategoryUpdate`. A JSON body with `created_at` is `deny_unknown_fields`. In the admin, they are readonly because `managed` makes the admin field read-protected. `reject_locked` rejects them in the form.

The watch is `Utc::now()`, unless a test sets `ferrioxid::core::set_test_clock` for the current thread. This is not a model registry.

`HasTimestamps` provides `created_at()` and `updated_at()` if both roles are present.

`ferrioxid generate model Product --timestamps` includes the new model with `with_timestamps!`. Without the flag, the previous hull remains. Existing files are not overwritten.

JSON remains flat: `created_at` and `updated_at` stand alongside `title`.
