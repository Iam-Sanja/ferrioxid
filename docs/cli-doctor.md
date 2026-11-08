# Diagnosis and project review

[For overview](index.md) · **IST file:** `crates/ferrioxid-cli/src/diagnose.rs`.]

## Aufgabe

`ferrioxid doctor` checks the tool chain and the project. `ferrioxid check` checks configuration, migrations and translation without changing data.

## Responsible files

- `crates/ferrioxid-cli/src/diagnose.rs`
- `crates/ferrioxid-migrations/src/project.rs`

## Public interface

```powershell
ferrioxid doctor
ferrioxid check
```

Status lines are `[OK]`, `[WARN]` and `[ERROR]`. An error gives exit code 1. `check --strict` gives exit code 2 if only warnings remain. Without `--strict`, warnings remain with Exitcode 0.

## Konfiguration

It reads `ferrum.toml`. `DATABASE_URL` may replace the database entry. `check` rejects an empty `DATABASE_URL`. If `FERRUM_CSRF_SECRET` is set, it must have 64 hex characters. A missing value is not an error here, because the runtime can generate a development key.

## Datenfluss

`doctor` calls `rustc` and `cargo` with `--version`. After that it searches `ferrum.toml`, `Cargo.toml`, `src/main.rs`, `src/lib.rs` and the migration directory. An existing database is only read. A non-existent SQLite file is a warning and is not created. PostgreSQL is readably connected when the URL points to it. A failure is a mistake. For a fullstack project, `doctor` reports that the admin is in the rust binary. Node.js is not required. Storage routes are not closed from `main.rs`.

`check` checks the environment, both migration histories and executes `cargo check`. If the database file is available and `schema_command` is set, it compares the model. `FERRUM_CHECK_SKIP_SCHEMA=1` and `FERRUM_CHECK_SKIP_CARGO=1` skip these steps in tests. It does not call `migrate up` and does not delete anything.

## Error cases

| Fall | Status |
|---|---|
|`rustc` or `cargo` is missing| Fehler |
|`ferrum.toml` or `src/lib.rs` is missing| Fehler |
| `server.host` ist `0.0.0.0` | Fehler |
|SQLite file is missing| Warnung |
| Docker fehlt |no error; Docker is optional and not tested|
|Checksum conflict| Fehler in `check` |

`doctor` in the ferrioxide workspace itself reports missing `src/main.rs` and `src/lib.rs` because the workspace root is not an application. The command applies to generated projects and to `examples/myshop` only if there is a separate `ferrum.toml` there. The workspace file remains valid for `migrate`.

## Sicherheitsregeln

Connection character strings are not output. `doctor` and `check` do not create an SQLite file or a history table. `check` does not write a migration.

## Beispiele

```powershell
cd demo
ferrioxid doctor
ferrioxid check
ferrioxid check --strict
```

The same call form applies under Linux in the respective shell.

## Tests

`generator.rs` expects success in a fresh API project and an error after `src/lib.rs` is removed. The same file compares the migration bytes before and after `ferrioxid check`.

## Limitations

A missing SQLite file is not created. If it exists, it is read with `mode=ro`. `doctor` does not start `schema_command`. `check` and `migrate status` compare the model if the file exists and a schema can be loaded. The joint report is published in [migration-diagnostics.md](migration-diagnostics.md)].
