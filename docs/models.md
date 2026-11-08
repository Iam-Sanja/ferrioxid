# Modelle

[Zur Übersicht](index.md) · Komponenten: [model-components.md](model-components.md) · Zeitstempel: [model-timestamps.md](model-timestamps.md) · **IST:** `FerrioxidModel` in `crates/ferrioxid-macros`.

A model is a rust structure with `#[derive(FerrioxidModel)]`. The same structure provides columns, repository, JSON, REST and admin. There is no second model system.

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
