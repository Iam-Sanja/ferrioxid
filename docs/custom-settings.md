# Custom settings

[Overview](index.md) · Shared file: [settings.md](settings.md) · **Implemented file:** `crates/fexu-migrations/src/project.rs`.

## Purpose

Application values are under `[custom.<name>]`. Fexu stores it on the read `ProjectConfig` and converts a section into a structure that implements `serde::Deserialize`. There is no second parser and no singleton.

## File

```toml
[custom.shop]
currency = "EUR"
items_per_page = 25
open = true
```

`shop` and each key are lower case, numerals and underscores, maximum 32 characters, and begin with one letter. Keys containing `password`, `secret`, `token` or `credential` are rejected. The error message does not contain the value.

## Reading

```rust
#[derive(serde::Deserialize)]
struct ShopSettings {
    currency: String,
    items_per_page: u64,
    open: bool,
}

let settings = fexu::migrations::Settings::discover(std::path::Path::new("."))?;
let shop: ShopSettings = settings.project().custom_settings("shop")?;
```

`custom_settings` delivers `custom.shop ist nicht konfiguriert.` if the section is missing. If a field does not fit, the message is `custom.shop passt nicht zur erwarteten Struktur.`.

## Types

The parser knows only three forms in this family:

- `true` and `false` become Boolean.
- A non-empty digit sequence becomes `u64`.
- Any other value remains a string.

There are no decimal numbers, negative numbers and lists here. The existing typed sections remain responsible for this.

## Limitations

A section does not replace a framework setting. `custom.server` is its own name and does not change `server.port`. Unknown sections outside of `custom.*` remain errors. Existing files without `custom` remain valid.
