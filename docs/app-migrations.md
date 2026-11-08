# App migrations

[For overview](index.md) · Engine: [migrations.md](migrations.md) · **IST files:** `crates/ferrioxid-migrations/src/packages.rs`, `runner.rs`, `inspect.rs`].

All app migrations remain in `_ferrum_migrations`. Auth remains in `_ferrum_auth_migrations`. There is no second history table.

## Kennung

The file retains the ID `0001_initial`. The `name` field remains `initial`. The checksum is formed over the file content with this unqualified ID.

The history line of a new app is `shop:0001_initial`. The reader for the app removes the prefix `shop:` and compares the rest with the file ID. The previous reader ignores lines containing `:`. An old line `0001_initial` is therefore not assigned to an app and is not executed again.

Two apps can both have a file `0001_initial.json`. The history IDs `shop:0001_initial` and `blog:0001_initial` are different.

## Reihenfolge

`depends_on` calls other app names. `package_order` sorted topologically. Ready apps come by name. A cycle, missing dependency, self-reference, and duplicate name fail before a file is applied.

`ferrioxid migrate up` first applies Auth, then the previous project directory, then the packages from `[app.<name>]`. A second run does not apply anything again. An amended checksum is a conflict and is not applied. The server only starts when `registry.migrations` describes the same set. The CLI cannot read the Rust page without running the application and therefore does not claim the match.

`migrate status`, `check` and `doctor` include pending packet migrations. A missing `ferrum.toml` with a pure `--migrations-dir` remains the previous empty packet case. An existing, invalid `ferrum.toml` is an error. The diagnosis does not create a history table and a SQLite file.

## Bestehende Projekte

Projects without `[app.<name>]` retain their unqualified IDs. Applied rows and their checksums are not rewritten. Tables are not renamed. Installing an app does not create a migration.

myshop `0001` to `0005` remain the project history. They are not an app package.
