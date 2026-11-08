# Apps

[Zur Übersicht](index.md) · Registry: [app-registry.md](app-registry.md) · Migrationen: [app-migrations.md](app-migrations.md) · Wiederverwendung: [reusable-apps.md](reusable-apps.md) · Generator: [create-app.md](create-app.md)

**ACT.** Functional status v0.25. The cargo version remains `0.1.0`. The template version of new projects remains `0.24`. An app is a Rust type that passes models, routes, admins, migrations, templates, and static files to the existing engines. There is no second router, authentic, model or admin engine and no reloading of unknown crates.

## Django and ferrioxide

| Django | Ferrioxid |
|---|---|
| `INSTALLED_APPS` |`App::install` with a type that implements `FerrioxidApplication`|
| `AppConfig` | `AppConfig::new("shop").label(...).prefix(...).depends_on(...)` |
|`apps.py` and Dynamic Import|Rust code in binary. `ferrum.toml` does not load a code|
| `shop.Product` und `inventory.Product` |separate table names. An app name does not change SQL or API paths|
| `migrations/` je App |same directory, history ID `shop:0001_initial`|
| `{% url "shop:product_detail" id=product.id %}` | `{{ url(name="shop.detail", id=product.id) }}` |
| App-Templates |Files under `{app}/`, for example `shop/list.html`|
| App-Staticfiles |`/static/{app}/...` on the existing Staticfiles delivery|

## Registrieren

```rust
use axum::routing::get;
use ferrioxid::apps::{AppConfig, AppError, FerrioxidApplication, Registration};
use ferrioxid::urls::{UrlError, UrlSet};

pub struct ShopApp;

impl FerrioxidApplication for ShopApp {
    fn config(&self) -> AppConfig {
        AppConfig::new("shop").label("Shop").prefix("/shop")
    }

    fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), AppError> {
        registry.routes(routes()?)?;
        Ok(())
    }
}

pub fn routes() -> Result<UrlSet, UrlError> {
    UrlSet::new().route("shop.list", "/items", get(list))
}

async fn list() -> axum::response::Response {
    ferrioxid::views::json(&ferrioxid::serde_json::json!({"app": "shop"}))
}
```

```rust
let app = ferrioxid::App::new(database)
    .install(ShopApp)?
    .install(BlogApp)?
    .seal()?;
let router = app.into_router();
```

`install` checks the name, rejects duplicates and reserved names and calls `contribute`. `seal` checks the migration graph, the match with `[app.<name>]` and, if available, `AdminRegistry::seal`. `seal` does not apply migration. `try_into_router` performs this check if it is still missing. `into_router` remains without `Result` and does not start in case of an error. Without apps and without `[app.<name>]`, the start does not change. The life cycle is in [app-lifecycle.md](app-lifecycle.md)].

Without `install`, `App::new`, `register`, `urls` and `merge` remain valid. `FerrioxidApplication::register_jobs` has an empty standard method. `install`, `seal` and `into_router` do not call and do not start a worker. The application’s worker binary calls `register_jobs` on the same `JobRegistry`. An example is `examples/notes`. See [jobs.md](jobs.md) and [workers.md](workers.md)].

## What an App Can Sign Up

- `routes`: a `UrlSet`. The namespace before the point must be the app name. A prefix is prescribed. `/` becomes the prefix itself.
- `model`: the existing `FerrioxidModel` repository. An already occupied table is `app.model.duplicate`.
- `admin_model`, `admin_view`, `admin_action`, `admin_filter`, `admin_widget`: the existing `AdminRegistry`.
- `migrations`: a relative directory without `..`.
- `templates`: HTML under `{app}/`. `TemplateMode::Isolated` rejects a project file of the same name. `ProjectOverride` reads them instead.
- `static_files`: Dateien unter `/static/{app}`.
- `project`: the `ProjectConfig` from `with_settings`. Own values come from `[custom.shop]` over `custom_settings`.

The registration does not award a staff, a superuser or a model right.

## Namen

An app name begins with a lowercase letter and then contains only lowercase letters, numbers and `_`. Not more than 32 characters. `admin`, `auth`, `api`, `static`, `healthz` and `uploads` are reserved.

A URL prefix must not be `/admin` or `/auth` and must not break out of the path.

## Lebenszyklus

1. `App::new` and optionally `with_settings` and `with_admin`.
2. `install` per app, in the desired order.
3. `contribute` reports models, routes and the other components.
4. Conflicts on names, paths, tables and templates end `install`.
5. `seal` checks dependencies and the admin registry. The registry will not accept another app afterwards.
6. `ferrioxid migrate up` applies pending migrations. This is a step of its own.
7. `into_router` attaches static files, templates and the `RouteMap`.
8. The server starts.

`with_admin` replaces the admin registry. Apps that register admin extensions come afterwards or the extensions are re-registered after `with_admin`.
