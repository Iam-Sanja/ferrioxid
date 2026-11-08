# Migration diagnostics

[Overview](index.md) · Migrations: [migrations.md](migrations.md) · CLI: [cli.md](cli.md)

**Implemented:** `crates/ferrioxid-migrations/src/inspect.rs`. The Cargo version remains `0.1.0`.

## Purpose

`ferrioxid migrate status`, `ferrioxid check`, `ferrioxid doctor`, `ferrioxid dev` and `ferrioxid upgrade --check` or `--dry-run` share read-only diagnostics. These checks apply no migrations and create neither history tables nor SQLite files.

Auth files live in `crates/ferrioxid-auth/migrations`, with history in `_ferrioxid_auth_migrations`. Project migrations use `migrations_dir` and `_ferrioxid_migrations`. Declared `[app.<name>]` packages share that table with namespaced IDs such as `shop:0001_initial`. The CLI reads TOML rather than `registry.migrations` and reports this limit as `app.migration.runtime`; startup compares both declarations.

## Report

Each deviation is a `Diagnostic` with range `Auth` or `App`, migration ID, level `Ok`, `Warn` or `Error`, stable code and text. Applied, pending and conflicted files remain separate.

Stable codes:

| Code | Level | Meaning |
|---|---|---|
| `migration.auth.pending` | Warning |Auth file is not in `_ferrioxid_auth_migrations`|
| `migration.app.pending` | Warning |App file is not in `_ferrioxid_migrations`|
| `migration.history.missing` | Warning | The history table is missing; diagnostics do not create it |
| `migration.checksum_mismatch` | Error |Saved checksum and file do not match|
| `migration.unknown` | Error |An applied ID no longer has a file|
| `migration.order` | Error |The order of files and history is wrong|
| `migration.schema_conflict` | Warning |The loaded model does not correspond to the migration target|
| `migration.auth.table_missing` | Error |`0002_admin_journal` or `0003_audit_operation` is applied, the table or `operation_id` is missing|
| `migration.database.missing` |Warning in Check, Doctor, Dev and Upgrade. Error in `migrate status`|The SQLite file does not exist|
| `migration.database.unavailable` | Error |The existing file is not a readable database. This is not a success|

`0002_admin_journal` relates to `_ferrioxid_admin_audit` and `_ferrioxid_admin_drafts`. `0003_audit_operation` relates to the column `operation_id`. As long as these files are only pending, this is a warning and not a silent creation.

## Reading without writing

SQLite diagnoses open an existing file with `mode=ro`. A missing file does not lead to a `connect`. PostgreSQL is connected to the same URL, the queries are `SELECT`. `ensure_history` is reserved for writing commands.

The URL is `DATABASE_URL`, otherwise `database.url` from `ferrioxid.toml`. The same resolution is used by the generated program. A URL that is calculated only in the application code does not see the CLI.

## Schema

`ferrioxid migrate status` loads the model via `--schema-file` or `schema_command`. `ferrioxid check` does the same after the database file is present. `FERRIOXID_SCHEMA_FILE` replaces the command. `FERRIOXID_CHECK_SKIP_SCHEMA=1` skips it in `check`. `doctor`, `dev` and `upgrade` do not start `schema_command`. Without a loaded schema, the diagnosis explicitly reports this and claims no match.

## Commands

`migrate status` remains successful for pending migrations and names `ferrioxid migrate up`. Conflicts, a missing file and an unreadable database terminate it with Exitcode 1.

`check` terminates warnings only with `--strict` and exit code 2. Without `--strict`, the exit code remains 0 as long as there is no error. `cargo check` then runs. `FERRIOXID_CHECK_SKIP_CARGO=1` omits this step and is intended for testing.

`doctor` explains the same situation and proposes `ferrioxid migrate up`. Warnings do not change its exit code. An unreadable database is an error. An uncreated SQLite file is a warning. Storage routes are not guessed from the source code. A missing storage directory is only a warning if the directory itself is missing.

`dev` prints migration warnings before building. Pending migrations do not prevent startup. If status cannot be read, it reports that failure rather than claiming the database was checked.

## Example

```text
Backend: sqlite

Auth-Migrationen:
  0001_auth_schema  angewendet  checksum …
  0002_admin_journal  ausstehend  checksum …
  0003_audit_operation  ausstehend  checksum …

App-Migrationen:
  0001_initial  angewendet  checksum …

Konflikte: keine
Ausstehende Migrationen: 2
  migration.auth.pending 0002_admin_journal ist ausstehend.
  migration.auth.pending 0003_audit_operation ist ausstehend.

Nächster Schritt:
  ferrioxid migrate up
```

`FERRIOXID_LOCALE=en` switches the headings in English. The codes remain English.

## PowerShell

```powershell
$env:FERRIOXID_LOCALE = "de"
ferrioxid migrate status
ferrioxid check
ferrioxid check --strict
ferrioxid doctor
```
