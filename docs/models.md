# Models

[Overview](index.md) · Components: [model-components.md](model-components.md) · Timestamps: [model-timestamps.md](model-timestamps.md) · **Implemented:** `FerrioxidModel` in `crates/ferrioxid-macros`.

A model is a Rust struct with `#[derive(FerrioxidModel)]`. That struct supplies columns, repository methods, JSON, REST routes and admin metadata.

```rust
use ferrioxid::prelude::*;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "products",
    api = "/api/v1/products",
    allow_anonymous = true
)]
pub struct Product {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1, format = "slug")]
    pub name: String,
}
```

`id` remains `Uuid` with `#[field(primary_key, auto)]`. Supported field types are `Uuid`, `String`, `i64`, `bool`, `NaiveDate`, `NaiveTime`, `DateTime<Utc>` and `Option` thereof, except for `id`.

`#[field(format = "slug")]` and `#[field(format = "email")]` check the value on the server side when creating and PATCH. An unknown `format` or `managed` value is a compile error. `#[field(min = 1)]` remains the rule for positive numbers. `#[field(required)]` rejects empty strings.

JSON is flat. A reused field in JSON is called the column, not the field group.
