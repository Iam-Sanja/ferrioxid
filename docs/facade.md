# Ferrioxid – public framework facade

[Overview](index.md) · **Implemented file:** `crates/ferrioxid/src/lib.rs`.

## Task (Implemented)

The `ferrioxid`-Crate is the public entry point. It exports `App`, `FerrioxidModel` (derive macro) and wrapper namespaces for the libraries used. The `prelude` block exports `App`, `FerrioxidModel`, `Uuid`.

### Current exports

- `ferrioxid::App`, `ferrioxid::pages`, `ferrioxid::web`
- `ferrioxid::forms`, `ferrioxid::static_files`, `ferrioxid::storage`, `ferrioxid::cache`, `ferrioxid::middleware`
- `ferrioxid::core` (`ferrioxid-core`)
- `ferrioxid::db` (`ferrioxid-db`)
- `ferrioxid::FerrioxidModel` (Macro)
- `ferrioxid::sea_orm`, `ferrioxid::serde`, `ferrioxid::serde_json`, `ferrioxid::uuid`, `ferrioxid::async_trait`
- `ferrioxid::prelude::{App, FerrioxidModel, Uuid}`

### Developer usage

```rust
use ferrioxid::prelude::*;
#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)] pub id: Uuid,
    #[field(required)] pub name: String,
}
```

**SHOULD:** In the long term, if possible, only need this facade as a direct dependency; The required internal types are specifically re-exported. No secret semantic change to the public macro.

## Compatibility rules

1. The macro references generated paths such as `::ferrioxid::db::...`: changes to re-exports directly affect each user model.
2. New trait bound or new mandatory fields can be build-breaking.
3. The macro generates public DTO names such as `ProductCreate`; changing them requires a migration plan.
4. Do not use `prelude` as a collection point for all internal implementation details.
5. Examples and tests must work **only on the public facade**.

## Acceptance criteria

The `myshop` app compiles against public exports; a new sample project must not require private Crate internals. Proof of this by `cargo check -p myshop`.
