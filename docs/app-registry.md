# App registry

[Overview](index.md) · Apps: [apps.md](apps.md) · **Implemented file:** `crates/fexu-api/src/apps.rs`.

The registry belongs to `App`; there is no global mutable singleton. App order follows the `install` calls.

## Types

`AppConfig` has the technical name, an optional display name, an optional URL prefix and optional dependencies.

`FexuApplication` has two methods: `config` and `contribute`. `contribute` receives a `Registration` with the lifetime of the current `install`.

`Registration` delegates to the existing registries:

- Routes to `UrlSet` and the path list of `App`.
- Models to `register`.
- Admin extensions to `AdminRegistry`.
- Migrations to `PackageMigration`.
- Templates to `Site`.
- Static files to `fexu_static::mount`.

`AppError.code` names the cause, for example `app.duplicate`, `app.sealed`, `app.route.namespace`, `app.model.duplicate`, `app.template.conflict`, `app.static.path` and `app.migration`.

## Sealing

`seal` makes the registry immutable. Repeating it succeeds without changes; calling `install` afterwards returns `app.sealed`.

The migration graph is only checked here because a later installed app can fulfill a dependency. A cycle, a missing dependency and a duplicate app name in the graph are `app.migration` and do not write anything.

`try_into_router` and `into_router` pick up `seal` when apps are installed or `[app.<name>]` is declared. An error prevents the start. An explicit `seal` remains valid and is not executed twice. Without apps and without declaration, the previous starting path remains.

## fexu.toml

There is no `[apps] enabled = ["shop"]` section. A string would not load an unknown app, and a silent ignoring would be misleading.

`[app.shop]` declares only the migration directory and `depends_on` for the CLI. The CLI does not read Rust. The behavior of the app comes exclusively from `install`.

```toml
[app.shop]
migrations = "src/apps/shop/migrations"
depends_on = "accounts"

[custom.shop]
currency = "EUR"
```

`[app]` without a name is an error. The path must be relative and must not contain `..`. If `migrations` is missing, this is also an error.

If either migration list is nonempty, `seal` requires matching app names, `depends_on` values and directories. Missing `with_settings` produces `app.migration.undeclared`; differing declarations produce `app.migration.mismatch`. `fexu check`, `fexu doctor` and `fexu migrate status` only read `fexu.toml` and report `app.migration.runtime`; they do not verify the Rust registration.
