# App-Lebenszyklus

[Zur Übersicht](index.md) · Registry: [app-registry.md](app-registry.md) · Migrationen: [app-migrations.md](app-migrations.md)

`install` checks the name, duplicates and contributions of the app. `seal` then checks the migration graph, the match with `[app.<name>]` and the admin registry. `seal` does not write a migration.

`try_into_router` calls `seal` when apps are installed or `with_settings` sees at least one `[app.<name>]` declaration and has not yet been sealed. A second call for a successful `seal` does not re-check. `into_router` remains for existing projects without `Result`. If there are apps or declarations and the check fails, the server does not start.

Without installed apps and without `[app.<name>]`, `into_router` remains the previous way. myshop uses it unchanged.

The CLI only reads `ferrum.toml`. It does not run the Rust code of the app. `check`, `doctor` and `migrate status` say this explicitly and do not report the TOML migrations as confirmed against `registry.migrations`. The process start is the place that compares both sides. Diagnostic commands do not create a database file and a history table.
