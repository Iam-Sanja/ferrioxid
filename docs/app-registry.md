# App-Registry

[For overview](index.md) · Apps: [apps.md](apps.md) · **IST file:** `crates/ferrioxid-api/src/apps.rs`].

The registry is on `App`. There is no global changeable singleton. The order is the order of the `install` calls.

## Typen

`AppConfig` has the technical name, an optional display name, an optional URL prefix and optional dependencies.

`FerrioxidApplication` has two methods: `config` and `contribute`. `contribute` receives a `Registration` with the lifetime of the current `install`.

`Registration` only passes on to the existing registers:

- Routes to `UrlSet` and the path list of `App`.
- Modelle an `register`.
- Admin an `AdminRegistry`.
- Migrations to `PackageMigration`.
- Vorlagen an `Site`.
- Staticfiles an `ferrioxid_static::mount`.

`AppError.code` names the cause, for example `app.duplicate`, `app.sealed`, `app.route.namespace`, `app.model.duplicate`, `app.template.conflict`, `app.static.path` and `app.migration`.

## Versiegeln

`seal` is the point from which the registry is unchangeable. A second `seal` is successful and changes nothing. `install` after that is `app.sealed`.

The migration graph is only checked here because a later installed app can fulfill a dependency. A cycle, a missing dependency and a duplicate app name in the graph are `app.migration` and do not write anything.

`try_into_router` and `into_router` pick up `seal` when apps are installed or `[app.<name>]` is declared. An error prevents the start. An explicit `seal` remains valid and is not executed twice. Without apps and without declaration, the previous starting path remains.

## ferrum.toml

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

`seal` requires that both lists contain the same app name, `depends_on` and directories as soon as a page is not empty. If `with_settings` is missing, the `app.migration.undeclared` is not considered tested. One difference is `app.migration.mismatch`. `ferrioxid check`, `ferrioxid doctor` and `ferrioxid migrate status` only read `ferrum.toml` and write the line `app.migration.runtime`. They do not confirm the Rust comparison.
