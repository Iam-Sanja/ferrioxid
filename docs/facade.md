# Fexu – public framework facade

[Overview](index.md) · **Implemented file:** `crates/fexu/src/lib.rs`.

## Task (Implemented)

The `fexu`-Crate is the public entry point. It exports `App`, `FexuModel` (derive macro) and wrapper namespaces for the libraries used. The `prelude` block exports `App`, `FexuModel`, `Uuid`.

### Current exports

- `fexu::App`, `fexu::pages`, `fexu::web`
- `fexu::forms`, `fexu::static_files`, `fexu::storage`, `fexu::cache`, `fexu::middleware`
- `fexu::core` (`fexu-core`)
- `fexu::db` (`fexu-db`)
- `fexu::FexuModel` (Macro)
- `fexu::sea_orm`, `fexu::serde`, `fexu::serde_json`, `fexu::uuid`, `fexu::async_trait`
- `fexu::prelude::{App, FexuModel, Uuid}`

### Developer usage

```rust
use fexu::prelude::*;
#[derive(FexuModel, fexu::serde::Serialize, fexu::serde::Deserialize)]
#[fexu(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)] pub id: Uuid,
    #[field(required)] pub name: String,
}
```

**SHOULD:** In the long term, if possible, only need this facade as a direct dependency; The required internal types are specifically re-exported. No secret semantic change to the public macro.

## Compatibility rules

1. The macro references generated paths such as `::fexu::db::...`: changes to re-exports directly affect each user model.
2. New trait bound or new mandatory fields can be build-breaking.
3. The macro generates public DTO names such as `ProductCreate`; changing them requires a migration plan.
4. Do not use `prelude` as a collection point for all internal implementation details.
5. Examples and tests must work **only on the public facade**.

## Acceptance criteria

The `myshop` app compiles against public exports; a new sample project must not require private Crate internals. Proof of this by `cargo check -p myshop`.
