# App migrations

[Overview](index.md) · Engine: [migrations.md](migrations.md) · **Implemented files:** `crates/ferrioxid-migrations/src/packages.rs`, `runner.rs`, `inspect.rs`.

All app migrations remain in `_ferrum_migrations`. Auth remains in `_ferrum_auth_migrations`. There is no second history table.

## Identifier

The file retains the ID `0001_initial`. The `name` field remains `initial`. The checksum is formed over the file content with this unqualified ID.

A new app’s history ID is `shop:0001_initial`. The app-specific reader removes `shop:` before comparing it with the file ID. The legacy reader ignores IDs containing `:`. Existing unqualified IDs such as `0001_initial` remain in the legacy history and are not reassigned to an app.

Two apps can both have a file `0001_initial.json`. The history IDs `shop:0001_initial` and `blog:0001_initial` are different.

## Order

`depends_on` lists other app names. `package_order` sorts apps topologically, ordering ready apps by name. Cycles, missing dependencies, self-references and duplicate names fail before any migration is applied.

`ferrioxid migrate up` applies auth migrations first, then the legacy project directory, then packages declared in `[app.<name>]`. Repeating the command does not reapply migrations. A changed checksum is a conflict. At server startup, the registered migration packages must match the declarations; the CLI cannot verify Rust registration without running the application.

`migrate status`, `check` and `doctor` report pending package migrations. Using only `--migrations-dir` without `ferrum.toml` retains legacy behavior with no app packages. An existing invalid `ferrum.toml` is an error. Diagnostics create neither a history table nor a SQLite file.

## Existing projects

Projects without `[app.<name>]` retain their unqualified IDs. Applied rows and their checksums are not rewritten. Tables are not renamed. Installing an app does not create a migration.

myshop `0001` to `0005` remain the project history. They are not an app package.
