# App migrations

[Overview](index.md) · Engine: [migrations.md](migrations.md) · **Implemented files:** `crates/fexu-migrations/src/packages.rs`, `runner.rs`, `inspect.rs`.

All app migrations remain in `_fexu_migrations`. Auth remains in `_fexu_auth_migrations`. There is no second history table.

## Identifier

The file retains the ID `0001_initial`. The `name` field remains `initial`. The checksum is formed over the file content with this unqualified ID.

A new app’s history ID is `shop:0001_initial`. The app-specific reader removes `shop:` before comparing it with the file ID. The legacy reader ignores IDs containing `:`. Existing unqualified IDs such as `0001_initial` remain in the legacy history and are not reassigned to an app.

Two apps can both have a file `0001_initial.json`. The history IDs `shop:0001_initial` and `blog:0001_initial` are different.

## Order

`depends_on` lists other app names. `package_order` sorts apps topologically, ordering ready apps by name. Cycles, missing dependencies, self-references and duplicate names fail before any migration is applied. The same order is used when `migrate make --app` loads every installed app schema and resolves foreign keys. A foreign key may target a table owned by an app named in the child's direct `depends_on`. The generated snapshot records that dependency and the referenced tables as external catalog data; those tables are not created again by the child migration. SQLite and PostgreSQL still receive a real `REFERENCES` constraint and the model's `ON DELETE` action. Removing a required dependency, naming an unknown target, or declaring the same table in two apps produces an error and does not rewrite an applied file. A dependency change without a table change does not by itself write a new migration. The next real migration stores the current catalog, while `migrate up` rechecks the live `depends_on` graph before applying anything.

`fexu migrate up` applies auth migrations first, then job migrations, then the legacy project directory, then packages declared in `[app.<name>]`. Repeating the command does not reapply migrations. A changed checksum is a conflict. At server startup, the registered migration packages must match the declarations; the CLI cannot verify Rust registration without running the application.

`migrate status`, `check` and `doctor` report pending package migrations. Using only `--migrations-dir` without `fexu.toml` retains legacy behavior with no app packages. An existing invalid `fexu.toml` is an error. Diagnostics create neither a history table nor a SQLite file.

## Generating app migrations

In layout 2, use `fexu migrate make initial --app produkte`. The CLI selects the compiled app schema and declared migration directory. Unknown apps and incompatible explicit schema/directory flags are rejected. Empty projects/apps need no business migration. Root `migrate make` retains project history, not an aggregate of all app schemas.

## Existing projects

Projects without `[app.<name>]` retain their unqualified IDs. Applied rows and their checksums are not rewritten. Tables are not renamed. Installing an app does not create a migration.

myshop `0001` to `0005` remain the project history. They are not an app package.
