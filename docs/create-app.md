# Create an app

[Overview](index.md) · CLI: [cli.md](cli.md) · **actual file:** `crates/ferrioxid-cli/src/apps.rs`.

```powershell
ferrioxid generate app shop
```

The command creates these files and does not overwrite anything:

```text
src/apps/mod.rs
src/apps/shop/mod.rs
src/apps/shop/models.rs
src/apps/shop/urls.rs
src/apps/shop/views.rs
src/apps/shop/admin.rs
src/apps/shop/migrations/
src/apps/shop/templates/shop/index.html
src/apps/shop/static/shop/shop.css
```

`src/apps/mod.rs` receives `pub mod shop;` only if it is missing. `main.rs` is modified only when it contains the `// ferrioxid:apps:end` marker. Otherwise, the command prints the `.install(shop::ShopApp)?` call to add manually.

The generated type implements `FerrioxidApplication`. Its `shop.index` route uses `/` relative to the `/shop` prefix and returns `{"app":"shop"}` as JSON. Templates and static files are registered in isolated mode. `admin::attach` is intentionally empty.

The name follows the same rules as `AppConfig`: lower case, numerals, `_`, maximum 32 characters, no path and none of the reserved names.

`ferrioxid new` and template version `0.24` remain unchanged. This command does not generate a separate crate; see [reusable-apps.md](reusable-apps.md) for that structure.

The project itself records the migrations:

```toml
[app.shop]
migrations = "src/apps/shop/migrations"
```

This section does not load the Rust app. `install` remains necessary. `ferrioxid migrate up` applies the files, `install` does not.
