# Apps

[Overview](index.md) · Registry: [app-registry.md](app-registry.md) · Migrations: [app-migrations.md](app-migrations.md) · Reuse: [reusable-apps.md](reusable-apps.md) · Generator: [create-app.md](create-app.md)

**Implemented.** Milestone v0.25. The Cargo version remains `0.1.0`. New projects use template version `0.25` and layout 2; existing projects are unchanged. An app is a Rust type that passes models, routes, admins, migrations, templates, and static files to the existing engines. There is no second router, auth, model or admin engine and no reloading of unknown crates.

Create a project with `ferrioxid new project Shop` and a business module with `ferrioxid new app produkte`. See [structure](project-structure.md) and [CLI migration](project-generator.md#migration-from-old-clilayouts).

## Django and Ferrioxid

| Django | Ferrioxid |
|---|---|
| Project settings and root URLconf | `ferrioxid.toml` and `src/project/{settings,urls}.rs` |
| `INSTALLED_APPS` |`App::install` with a type that implements `FerrioxidApplication`|
| `AppConfig` | `AppConfig::new("shop").label(...).prefix(...).depends_on(...)` |
| `apps.py` and dynamic imports | Rust code compiled into the binary; `ferrioxid.toml` does not load code |
| `shop.Product` and `inventory.Product` | Separate table names; the app name does not change SQL or API paths. |
| `migrations/` per app | Shared migration engine; history ID `shop:0001_initial` |
| `{% url "shop:product_detail" id=product.id %}` | `{{ url(name="shop.detail", id=product.id) }}` |
| App-Templates |Files under `{app}/`, for example `shop/list.html`|
| App-Staticfiles |`/static/{app}/...` on the existing Staticfiles delivery|

## Registration

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

`install` checks the name, rejects duplicates and reserved names and calls `contribute`. `seal` checks the migration graph, the match with `[app.<name>]` and, if available, `AdminRegistry::seal`. `seal` does not apply migration. `try_into_router` performs this check if it is still missing. `into_router` remains without `Result` and does not start in case of an error. Without apps and without `[app.<name>]`, the start does not change. The life cycle is in [app-lifecycle.md](app-lifecycle.md).

`App::new`, `register`, `urls` and `merge` remain valid without `install`. `FerrioxidApplication::register_jobs` has an empty default implementation; `install`, `seal` and `into_router` do not call it or start workers. The application’s worker binary calls `register_jobs` with the shared `JobRegistry`, as shown in `examples/notes`. See [jobs.md](jobs.md) and [workers.md](workers.md).

## What an app can register

- `routes`: a `UrlSet` with names prefixed by the app name and a period. A URL prefix is required; `/` maps to that prefix.
- `model`: the existing `FerrioxidModel` repository. An already occupied table is `app.model.duplicate`.
- `admin_model`, `admin_view`, `admin_action`, `admin_filter`, `admin_widget`: the existing `AdminRegistry`.
- `migrations`: a relative directory without `..`.
- `templates`: HTML under `{app}/`. `TemplateMode::Isolated` rejects a project file of the same name. `ProjectOverride` overlays project `templates/<appname>/` on local templates and supports shared-only files. New generated apps explicitly use this mode.
- `static_files`: files under `/static/{app}`.
- `project`: the `ProjectConfig` supplied by `with_settings`. Read app-specific `[custom.shop]` values through `custom_settings`.

Registration grants no staff status, superuser status or model permissions.

## Names

An app name begins with a lowercase letter and then contains only lowercase letters, numbers and `_`. Not more than 32 characters. `admin`, `auth`, `api`, `static`, `healthz` and `uploads` are reserved.

A URL prefix must not be `/admin` or `/auth` and must not break out of the path.

## Lifecycle

1. `App::new` and optionally `with_settings` and `with_admin`.
2. `install` per app, in the desired order.
3. `contribute` reports models, routes and the other components.
4. Conflicts on names, paths, tables and templates end `install`.
5. `seal` checks dependencies and the admin registry. The registry will not accept another app afterwards.
6. `ferrioxid migrate up` applies pending migrations. This is a step of its own.
7. `into_router` attaches static files, templates and the `RouteMap`.
8. The server starts.

`with_admin` replaces the admin registry. Apps that register admin extensions come afterwards or the extensions are re-registered after `with_admin`.

Shared templates use `export_templates` and `import_templates`. See [templates.md](templates.md). Cross-app foreign keys use `depends_on`, `external_tables` and `resolve_installed_schemas`; the table stays registered in one app. See [app-migrations.md](app-migrations.md). `App::strict_migrations` plus `try_into_router_strict` checks that those migrations are applied before the process serves traffic. It does not migrate.
