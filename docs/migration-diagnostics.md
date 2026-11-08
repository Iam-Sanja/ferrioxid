# Migrationsdiagnose

[For overview](index.md) · Migrations: [migrations.md](migrations.md) · CLI: [cli.md](cli.md)]

**IST:** `crates/ferrioxid-migrations/src/inspect.rs`. The cargo version remains `0.1.0`.

## Aufgabe

The same reading test applies to `ferrioxid migrate status`, `ferrioxid check`, `ferrioxid doctor`, `ferrioxid dev` and `ferrioxid upgrade --check` and `--dry-run`, respectively. It does not apply migration, does not create a history table, and does not create an SQLite file.

Auth files are in `crates/ferrioxid-auth/migrations`. The story is called `_ferrum_auth_migrations`. App files are in the project’s `migrations_dir`. The story is called `_ferrum_migrations`. Declared app packages from `[app.<name>]` are in the same history under `shop:0001_initial`. The CLI reads this declaration and not `registry.migrations`. The line `app.migration.runtime` says that. The process start compares both sides.

## Bericht

Each deviation is a `Diagnostic` with range `Auth` or `App`, migration ID, level `Ok`, `Warn` or `Error`, stable code and text. Applied, pending and conflicted files remain separate.

Stabile Codes:

| Code | Stufe | Bedeutung |
|---|---|---|
| `migration.auth.pending` | Warnung |Auth file is not in `_ferrum_auth_migrations`|
| `migration.app.pending` | Warnung |App file is not in `_ferrum_migrations`|
| `migration.history.missing` | Warnung |The history table is missing. It is not invested|
| `migration.checksum_mismatch` | Fehler |Saved checksum and file do not match|
| `migration.unknown` | Fehler |An applied ID no longer has a file|
| `migration.order` | Fehler |The order of files and history is wrong|
| `migration.schema_conflict` | Warnung |The loaded model does not correspond to the migration target|
| `migration.auth.table_missing` | Fehler |`0002_admin_journal` or `0003_audit_operation` is applied, the table or `operation_id` is missing|
| `migration.database.missing` |Warning in Check, Doctor, Dev and Upgrade. Error in `migrate status`|The SQLite file does not exist|
| `migration.database.unavailable` | Fehler |The existing file is not a readable database. This is not a success|

`0002_admin_journal` relates to `_ferrum_admin_audit` and `_ferrum_admin_drafts`. `0003_audit_operation` relates to the column `operation_id`. As long as these files are only pending, this is a warning and not a silent creation.

## Reading without writing

SQLite diagnoses open an existing file with `mode=ro`. A missing file does not lead to a `connect`. PostgreSQL is connected to the same URL, the queries are `SELECT`. `ensure_history` is reserved for writing commands.

The URL is `DATABASE_URL`, otherwise `database.url` from `ferrum.toml`. The same resolution is used by the generated program. A URL that is calculated only in the application code does not see the CLI.

## Schema

`ferrioxid migrate status` loads the model via `--schema-file` or `schema_command`. `ferrioxid check` does the same after the database file is present. `FERRUM_SCHEMA_FILE` replaces the command. `FERRUM_CHECK_SKIP_SCHEMA=1` skips it in `check`. `doctor`, `dev` and `upgrade` do not start `schema_command`. Without a loaded scheme, the diagnosis explicitly reports this and claims no match.

## Orders

`migrate status` remains successful for pending migrations and names `ferrioxid migrate up`. Conflicts, a missing file and an unreadable database terminate it with Exitcode 1.

`check` terminates warnings only with `--strict` and exit code 2. Without `--strict`, the exit code remains 0 as long as there is no error. `cargo check` then runs. `FERRUM_CHECK_SKIP_CARGO=1` omits this step and is intended for testing.

`doctor` explains the same situation and proposes `ferrioxid migrate up`. Warnings do not change its exit code. An unreadable database is an error. An uncreated SQLite file is a warning. Storage routes are not guessed from the source code. A missing storage directory is only a warning if the directory itself is missing.

`dev` prints the warning before the build. Pending migrations do not stop the start. If the status could not be read, it is there. There is no success report about an unchecked database.

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

`FERRUM_LOCALE=en` switches the headings in English. The codes remain English.

## PowerShell

```powershell
$env:FERRUM_LOCALE = "de"
ferrioxid migrate status
ferrioxid check
ferrioxid check --strict
ferrioxid doctor
```
