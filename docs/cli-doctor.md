# Diagnostics and project checks

[Overview](index.md) · **Implemented file:** `crates/ferrioxid-cli/src/diagnose.rs`.

## Purpose

`ferrioxid doctor` checks the toolchain and project. `ferrioxid check` validates configuration, migrations and compilation without changing application data.

## Responsible files

- `crates/ferrioxid-cli/src/diagnose.rs`
- `crates/ferrioxid-migrations/src/project.rs`

## Public interface

```powershell
ferrioxid doctor
ferrioxid check
```

Status lines are `[OK]`, `[WARN]` and `[ERROR]`. An error gives exit code 1. `check --strict` gives exit code 2 if only warnings remain. Without `--strict`, warnings remain with Exitcode 0.

## Configuration

It reads `ferrioxid.toml`. `DATABASE_URL` may replace the database entry. `check` rejects an empty `DATABASE_URL`. If `FERRIOXID_CSRF_SECRET` is set, it must have 64 hex characters. A missing value is not an error here, because the runtime can generate a development key.

## Data flow

`doctor` runs `rustc --version` and `cargo --version`, then checks `ferrioxid.toml`, `Cargo.toml`, `src/main.rs`, `src/lib.rs` and the migration directory. Existing databases are inspected read-only; a missing SQLite file produces a warning without being created. PostgreSQL connection failures are errors. For fullstack projects, it explains that the admin is embedded in the Rust binary and needs no Node.js. It does not infer storage route registration from `main.rs`.

`check` checks the environment, both migration histories and executes `cargo check`. If the database file is available and `schema_command` is set, it compares the model. `FERRIOXID_CHECK_SKIP_SCHEMA=1` and `FERRIOXID_CHECK_SKIP_CARGO=1` skip these steps in tests. It does not call `migrate up` and does not delete anything.

## Error cases

| Case | Status |
|---|---|
|`rustc` or `cargo` is missing| Error |
|`ferrioxid.toml` or `src/lib.rs` is missing| Error |
| `server.host` is `0.0.0.0` | Error |
|SQLite file is missing| Warning |
| Docker is missing | Not an error; Docker is optional and is not probed. |
|Checksum conflict| Error in `check` |

`doctor` in the Ferrioxid workspace itself reports missing `src/main.rs` and `src/lib.rs` because the workspace root is not an application. The command applies to generated projects and to `examples/myshop` only if there is a separate `ferrioxid.toml` there. The workspace file remains valid for `migrate`.

A loopback `server.port` is probed with a bind that is released immediately. A busy port is a warning in `doctor` and an error in `ferrioxid dev`. `ferrioxid dev` prints that it does not apply migrations. A declared `[app.<name>]` with a missing migrations directory is an error. The command still does not load the app crate; `App::install` remains the registration.

## Security rules

Connection strings are never printed. `doctor` and `check` create neither SQLite files nor history tables; `check` writes no migrations.

## Examples

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

A missing SQLite file is not created. If it exists, it is read with `mode=ro`. `doctor` does not start `schema_command`. `check` and `migrate status` compare the model if the file exists and a schema can be loaded. The joint report is published in [migration-diagnostics.md](migration-diagnostics.md).
