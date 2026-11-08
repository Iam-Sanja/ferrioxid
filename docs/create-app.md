# App erzeugen

[For overview](index.md) · CLI: [cli.md](cli.md) · **actual file:** `crates/ferrioxid-cli/src/apps.rs`].

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

`src/apps/mod.rs` only gets the line `pub mod shop;` if it is missing. `main.rs` is only changed if it contains the mark `// ferrioxid:apps:end`. Otherwise, the output calls the line `.install(shop::ShopApp)?`.

The generated type implements `FerrioxidApplication`. The route `shop.index` is located on `/` and is attached under the prefix `/shop`. The answer is JSON `{"app":"shop"}`. Templates and static files are logged into isolated mode. `admin::attach` is intentionally empty.

The name follows the same rules as `AppConfig`: lower case, numerals, `_`, maximum 32 characters, no path and none of the reserved names.

`ferrioxid new` remains unchanged. The template version remains `0.24`. A separate crate does not generate the command. This is what [reusable-apps.md](reusable-apps.md)] stands for.

The project itself records the migrations:

```toml
[app.shop]
migrations = "src/apps/shop/migrations"
```

This section does not load the Rust app. `install` remains necessary. `ferrioxid migrate up` applies the files, `install` does not.
