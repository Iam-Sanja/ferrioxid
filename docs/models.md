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

`id` remains `Uuid` with `#[field(primary_key, auto)]`. Supported field types are `Uuid`, `String`, `i64`, `bool`, `Decimal`, `NaiveDate`, `NaiveTime`, `DateTime<Utc>`, `Option` of those except `id`, and an enum with `#[field(choice)]`.

## Decimal and choices

`Decimal` comes from `rust_decimal` and is re-exported as `ferrioxid::core::Decimal`. A field declares its limits:

```rust
#[field(precision = 12, scale = 2)]
pub price: Decimal,
```

Precision is `1..=28` and scale is at most the precision. A value with extra fractional digits, such as `1.005` at scale 2, is a validation error. Ferrioxid does not round it. `0.1 + 0.2` stays `0.3` because the arithmetic is decimal, not binary floating point. `i64` minor units such as `price_cents` stay valid.

SQLite stores the unscaled coefficient as a 16-byte `BLOB`. PostgreSQL stores the same bytes as `BYTEA`. The high bit is inverted and the rest is big-endian, so `ORDER BY` and range filters follow numeric order. JSON and fixture text use a decimal string. `rust_decimal` also accepts a JSON number when its lexical token is an exact decimal; writers in this crate emit strings. There is no currency column.

```rust
#[derive(Clone, Copy, FerrioxidChoice)]
pub enum ProductStatus {
    #[choice(value = "draft", de = "Entwurf", en = "Draft")]
    Draft,
    #[choice(value = "active", de = "Aktiv", en = "Active")]
    Active,
}

#[field(choice)]
pub status: ProductStatus,
```

The stored value is the `value` string. Unknown JSON fails deserialization. The generated `CREATE TABLE` adds `CHECK (status IN (...))`. Admin forms render a select and list cells use the German or English label. `forms::required_choice`, `forms::choice_options`, `forms::render_choice_select` and `forms::required_decimal` cover hand-written forms.

Changing precision, scale, or the set of stored choice values is refused. Existing rows are not rewritten. Add a new column and backfill it. Replacing a free `String` with a choice is allowed only when every existing value is already one of the stored values; otherwise the migration rolls back. A new `NOT NULL` decimal or choice column needs an explicit text backfill such as `"0.00"` or `"draft"`. The runner encodes a decimal backfill into the 16-byte form. Applied migration checksums are not rewritten.

`examples/myshop` keeps `price_cents` and adds `price` plus `ProductStatus`. `Order.status` stays a free string; `Order.state` is `Option<OrderStatus>`. Migration `0006_prices_and_status` adds those columns. `price` and product `status` are backfilled with `0.00` and `draft`. `state` is nullable.

`#[field(format = "slug")]` and `#[field(format = "email")]` check the value on the server side when creating and PATCH. An unknown `format` or `managed` value is a compile error. `#[field(min = 1)]` remains the rule for positive numbers. `#[field(required)]` rejects empty strings.

JSON is flat. A reused field in JSON is called the column, not the field group.
