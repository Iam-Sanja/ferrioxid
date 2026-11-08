# Migrations-Engine

[For overview](index.md) · **IST:** `crates/ferrioxid-migrations/`. **Not** the legacy command `myshop --bootstrap`.] Date types: [date-time-migrations.md](date-time-migrations.md)]. Auth `0003_audit_operation` complements `operation_id`. myshop `0005_orders` complements `orders` and `order_lines`. `0001` through `0004` and Auth `0001`/`0002` remain unchanged.

## 1. Architektur

```text
Rust-Modelle
  → FerrioxidModel / ColumnSpec
  → explizite Registrierung
  → SchemaSnapshot
  → Diff
  → Migrationsdatei
  → Runner
       → SQLite
       → PostgreSQL
```

The engine does not read Rust source text. It is not attached to Axum and not to the admin frontend. CRUD and migrations use the same type image in `crates/ferrioxid-db/src/sql.rs`.

App packages use the same table. The history ID is `shop:0001_initial`, the file remains `0001_initial`. Details: [app-migrations.md](app-migrations.md)]. Existing unqualified lines are not rewritten. The process start compares `registry.migrations` with `[app.<name>]`. The CLI only sees the TOML page and identifies that. A field group without a specific model does not generate a table. Columns are in `columns()` of the concrete models and thus in the schema diff.

Auth files are in `crates/ferrioxid-auth/migrations/` and write the history `_ferrum_auth_migrations`. `ferrioxid migrate up` applies this directory before the application files. The application history remains `_ferrum_migrations`. As a result, `0001_auth_schema` does not collide with `0001_initial`. The server start does not migrate. `0001_initial.json` remains unchanged.

| Aufgabe |File|
|---|---|
|Snapshot and Validation| `crates/ferrioxid-migrations/src/snapshot.rs` |
|Diff and risk| `crates/ferrioxid-migrations/src/diff.rs` |
|File format and checksum| `crates/ferrioxid-migrations/src/store.rs` |
| Historientabelle | `crates/ferrioxid-migrations/src/history.rs` |
|Implementation| `crates/ferrioxid-migrations/src/runner.rs` |
| Snapshot-Simulation | `crates/ferrioxid-migrations/src/simulate.rs` |
|SQLite conversion and PostgreSQL-ALTER| `crates/ferrioxid-migrations/src/rebuild.rs` |
| SQLite-Introspektion | `crates/ferrioxid-migrations/src/introspection.rs` |
| Baseline | `crates/ferrioxid-migrations/src/baseline.rs` |
|`ferrum.toml` and Schema command| `crates/ferrioxid-migrations/src/project.rs` |
| CLI | `crates/ferrioxid-cli/src/main.rs` |
|myshop registration| `examples/myshop/src/main.rs`, `ferrum.toml` |
| DDL | `crates/ferrioxid-db/src/sql.rs` |

`ColumnSpec` remains the model metadata. The snapshot translates them into logical types. SQL is only created for `DatabaseKind`. Unique constraints and indices are not present in the model metadata and are therefore missing in the snapshot. Foreign key relationships are not implemented.

## 2. Registrierung

`ferrum.toml` in the project or an overarching directory:

```toml
migrations_dir = "examples/myshop/migrations"
schema_command = "cargo run -p myshop --quiet -- --print-schema"
```

`myshop --print-schema` writes the JSON of the registered models to stdout. Tests and scripts can instead pass `--schema-file`. Without this integration, `migrate make` cannot know the model.

## 3. Snapshot

Format version `1`. Tables are sorted by name. Columns keep the order of the model. The same input generates the same compact JSON.

```json
{
  "format_version": 1,
  "tables": [
    {
      "name": "products",
      "columns": [
        {"name": "id", "type": "uuid", "primary_key": true, "nullable": false},
        {"name": "name", "type": "string", "primary_key": false, "nullable": false},
        {"name": "active", "type": "bool", "primary_key": false, "nullable": false, "default": true}
      ]
    }
  ]
}
```

Logical types are `uuid`, `string`, `i64` and `bool`. Unknown format versions, duplicate names and invalid identifiers will be rejected. A snapshot contains no connection data and no passwords. Exactly one non-nullable primary key is mandatory. Defaults are only available for `bool`.

In addition, a table may contain `unique_constraints`, `indexes` and `foreign_keys`. Empty lists are not serialized. Files of format version 1 without these keys remain valid and keep their checksum. The importance of already applied migrations does not change. Construction, ZERO behavior and the blocked foreign key changes are in [constraints.md](constraints.md)].

The same technical schemes shall be considered as unchanged even if only the column order differs. No migration is written for this.

## 4. Diff

`plan_change` compares the last confirmed target snapshot with the current model. It does not create a renaming.

|Amendment| Operation |Implementation|
|---|---|---|
| Neue Tabelle | `create_table` | sicher |
|New nullable column or `bool` with default| `add_column` | sicher |
|New `NOT NULL` column without default| `add_column` | blockiert |
|New primary key columns| `add_column` | blockiert |
|Delete the table| `drop_table` |only with `--allow-destructive`|
|Delete column, no primary key| `drop_column` |only with `--allow-destructive`|
|Delete primary key| `drop_column` | blockiert |
| `string` ↔ `i64` | `alter_column` |only after value check|
|Same logic type, different nullity or different bool default| `alter_column` |only after value check|
|Other type pairs, primary key flag| `alter_column` | blockiert |
|Column away and other column new| Drop plus Add |Blocked, no renaming|
|Table away and other table new| Drop plus Create |Blocked, no renaming|
|Explicit renaming| `rename_column`, `rename_table` |only through the rename command; affected indices are reinvested|
| Neuer Unique Index | `add_unique_constraint` |safe; Existing duplicates make migration fail|
| Unique Index entfernen | `drop_unique_constraint` |safe for the lines; duplicates are allowed|
| Neuer Index | `add_index` | sicher |
| Index entfernen | `drop_index` | sicher |
|Foreign Key on a new column| `add_column` mit `foreign_key` |Secure if the column is nullable or has a bool default|
|Foreign key on an existing column, remove or delete new rule| `add_foreign_key`, `drop_foreign_key` |Blocked, no file|

`migrate make` does not write a file when renaming is suspected. The message contains “not renaming” and refers to `rename-column` or `rename-table`.

## 4.1 Explicit renaming

First change the model to the new name. After that, in the workspace root:

```powershell
cargo run -p ferrioxid-cli -- migrate rename-column --table products --from name --to title
cargo run -p ferrioxid-cli -- migrate rename-table --from products --to items
```

The CLI loads the registered scheme. One operation must fully explain this scheme from the last migration target. Further deviations result in no file being written. The following `migrate make` will then see no change.

Identifiers shall comply with the snapshot format: letter or underscore, followed by letters, numbers or underscores. They are quoted in SQL. HTTP input is not a source for these names.

Both dialects receive the same SQL:

```sql
ALTER TABLE "products" RENAME COLUMN "name" TO "title"
ALTER TABLE "products" RENAME TO "items"
```

A renaming does not copy the table. If a unique index or a secondary index is attached to the renamed column or table, follow `DROP INDEX` and `CREATE INDEX` with the new deterministic name. Foreign key definitions follow the column renaming of the dialect. Your deletion rule is not changed. An existing index, trigger, view or foreign key blocks a later table conversion.

A file not yet applied may be edited. Thereafter:

```powershell
cargo run -p ferrioxid-cli -- migrate reseal migrations\0002_rename_title.json
```

`reseal` checks that the operations generate the target snapshot, rewrites the checksum and refuses the file if its ID is already in `_ferrum_migrations`. An applied file with changed checksum stops `migrate up`.

## 5. Dateiformat

```text
examples/myshop/migrations/0001_initial.json
```

Each file contains format version, ID `0001_name`, name, source snapshot, target snapshot, operations, risks and a SHA-256 checksum. The checksum covers the canonical JSON body without the checksum field. It recognises substantive changes. It does not prove that the database still corresponds exactly to this scheme.

Die Operationsvarianten sind extern getaggt mit `type` in `snake_case`: `create_table`, `drop_table`, `add_column`, `drop_column`, `alter_column`, `rename_column`, `rename_table`, `add_unique_constraint`, `drop_unique_constraint`, `add_index`, `drop_index`, `add_foreign_key`, `drop_foreign_key`. Die beiden Foreign-Key-Varianten werden erkannt, aber nicht geschrieben und nicht ausgeführt. `add_column` darf ein optionales `foreign_key` tragen. Fehlt es, bleibt das JSON der älteren Dateien gültig. Beim Laden muss die Simulation der Operationen auf dem Ausgangssnapshot den Zielsnapshot ergeben. Die automatische Diff-Liste muss nicht identisch sein. Deshalb kann eine Datei eine explizite Umbenennung enthalten, obwohl `plan_change` dieselbe Änderung ablehnen würde.

The first migration must start from an empty scheme. Each subsequent migration must be based exactly on the target snapshot of the previous one. IDs are numbered in four digits, maximum `9999`.

## 6. Historie

Tabelle `_ferrum_migrations`:

| Spalte | Bedeutung |
|---|---|
| `id` |Primary key, migration ID|
| `name` | Kurzname |
| `checksum` |Checksum at time of application|
| `applied_at_unix` | Unix-Sekunden |

The row is written in the same transaction as the schema change. If the migration fails, there is neither the schema change nor the history entry. There is no stored error status. An applied ID is not executed again. A missing file or a different checksum stops `migrate up`.

A history entry does not guarantee that no one has changed the scheme from the outside afterwards. Ferrioxide compares the history with the files, but does not read back the complete live scheme.

## 7. CLI

Im Workspace-Root:

```powershell
cargo run -p ferrioxid-cli -- migrate make add_note
cargo run -p ferrioxid-cli -- migrate up
cargo run -p ferrioxid-cli -- migrate status
cargo run -p ferrioxid-cli -- migrate rename-column --table products --from name --to title
cargo run -p ferrioxid-cli -- migrate rename-table --from products --to items
cargo run -p ferrioxid-cli -- migrate reseal pfad\zur\datei.json
cargo run -p ferrioxid-cli -- migrate baseline
cargo run -p ferrioxid-cli -- migrate baseline --confirm
```

The installed binary is called `ferrioxid`. `make` does not create a file without a schema change. Risky deletions are in the file and only run with:

```powershell
cargo run -p ferrioxid-cli -- migrate up --allow-destructive
```

`status` shows backend, authentic migrations and app migrations separately, applied or pending, checksums and conflicts. If the model scheme could be loaded, it indicates whether it fits the last app migration target. Pending migrations leave the command successful. Checksum, unknown IDs and sequence errors terminate it with Exitcode 1. The diagnosis opens SQLite with `mode=ro` and does not create a missing file. The database URL is not output. `--database-url` and `--migrations-dir` overwrite project and environment. Without URL, `DATABASE_URL` applies, otherwise `sqlite://db.sqlite3?mode=rwc` in the working directory. The joint report is published in [migration-diagnostics.md](migration-diagnostics.md)].

Deletion risks (`drop_table`, `drop_column`) indicate `--allow-destructive` to the CLI before `up`. A `rebuild_column` risk is not a deletion release. A failed check ends with exit code 1 and is not issued as a success. `baseline` without `--confirm` is such a case, even if the scheme fits.

## 8. SQLite

SQLite is the tested backend. `connect` keeps a connection and sets `PRAGMA foreign_keys=ON`. Tests use temporary files. `PRAGMA foreign_keys` is not switched in a transaction. SQLite would ignore this change there.

`CREATE TABLE` including Foreign Keys, `CREATE UNIQUE INDEX`, `CREATE INDEX`, secure `ADD COLUMN` optionally with `REFERENCES`, `DROP INDEX`, `RENAME TABLE`, `RENAME COLUMN`, as well as `DROP TABLE` and `DROP COLUMN` only after express approval. Each migration runs in a transaction along with the history entry. A unique index that violates existing lines rolls back this transaction.

### Tabellenumbau

`string` ↔ `i64` and changes to nullity and bool default run on SQLite as a copy of the table because SQLite cannot make these changes generally via `ALTER COLUMN`. The sequence in `rebuild.rs`:

1. Check initial and target columns. Unsupported pairs, primary key changes and type changes without verified conversion are rejected.
2. Check foreign keys, explicit indices, triggers and views of the table. If one of these is present, it is aborted before each DDL.
3. If restricted to `NOT NULL`, count the existing `NULL` values. If the number is greater than 0, it is aborted. No substitute value is used.
4. Only read the affected column and check it in Rust. `NULL` remains `NULL`. The same types must match the memory value. Bool must be `0` or `1`. `i64` after `string` is the decimal representation. `string` to `i64` must correspond exactly to `i64::to_string`. `01`, `+1`, `1.0`, `-0` and empty text are rejected. A successful SQL-`CAST` alone does not apply.
5. Create `ferrioxid_rebuild_<tabelle>` with the target scheme.
6. `INSERT INTO ... SELECT` copies the remaining columns. Only the tested type change is used as `CAST`. `name` is not guessed as `title`.
7. Compare `COUNT(*)` from source and copy.
8. Delete the old table and rename the copy to the old name.
9. Compare the number of lines again.
10. `PRAGMA integrity_check` must deliver `ok`. `PRAGMA foreign_key_check` must be empty.
11. Write the history entry and commit the transaction.

The remaining columns are not loaded into the process memory. The checked column will continue to be read in full so that invalid values are noticed before the `CAST`.

### Rollback

In this transaction, SQLite can be fully rolled back: `CREATE TABLE`, `INSERT`, `DROP TABLE`, `ALTER TABLE ... RENAME` and the history entry. If one step fails, the transaction is rejected. The failed migration is not considered to be applied. `PRAGMA foreign_keys` is not changed. Changes outside this transaction, in particular an already established `ensure_history` in the normal `migrate up`, will remain. The baseline creates the history table only in the confirmation transaction.

## 9. PostgreSQL

The same list of operations is executed, but not through the SQLite rebuild. `CREATE TABLE`, `CREATE INDEX`, secure `ADD COLUMN`, `RENAME` and released deletions use the PostgreSQL dialect. A violating Unique Index or Foreign Key makes the transaction fail. `add_foreign_key` and `drop_foreign_key` also remain blocked here because the same migration file must be able to run on SQLite. A supported column change also checks the values in Rust and then executes `ALTER COLUMN`: type over `TYPE ... USING`, default over `SET DEFAULT` or `DROP DEFAULT`, nullity over `SET NOT NULL` or `DROP NOT NULL`. Unsupported changes are also rejected here.

Baseline on PostgreSQL immediately breaks off and does not write a history. There is no silent partial takeover.

The `postgres_migration_roundtrip` test is `#[ignore]` and requires `DATABASE_URL`. It was not executed against a server here. An ignored test is not a success.

Not implemented: Advisory locks against parallel migrators and `down`.

## 10. Errors and transactions

Before the first SQL, the runner checks checks, order and the permissibility of each operation. An unsupported or unreleased operation stops. Already established earlier migrations remain. The failed migration and all later ones are not applied.

`CREATE TABLE` without `IF NOT EXISTS` hits an existing table, rolls back and does not mark it as migrated. The message refers to a new database. Existing files are not deleted.

## 11. Bootstrap

`cargo run -p myshop -- --bootstrap` is Legacy. The command does not write a schema and history.

- History present: cancellation, reference to `migrate status`.
- Table `products` without history: cancellation, no deletion, no marking as migrated.
- Empty database: cancellation, reference to `migrate make` and `migrate up`.

`App::bootstrap_schema` remains a test assistant for empty databases without history. If `_ferrum_migrations` exists, this function does not create anything. It is not a migration and does not recognize schema drift.

The HTTP server does not migrate at startup.

### Transition of an old SQLite file

`ferrioxid migrate baseline` handles a SQLite file whose schema already corresponds to the last migration destination. The command does not change a table or a row. Without `--confirm`, no history is written, and the process ends with an error that mentions the successful verification and the missing confirmation.

Ablauf:

1. Open connection. PostgreSQL is rejected.
2. Read scheme about `sqlite_master`, `PRAGMA table_info`, `PRAGMA index_list`, `PRAGMA index_info` and `PRAGMA foreign_key_list`. `_ferrum_migrations`, `sqlite_%` and the primary key autoindex are ignored.
3. Existing history entries reject the baseline.
4. The migration chain must be consistent from the empty scheme.
5. If a file contains `drop_table`, `drop_column`, `drop_index`, `drop_unique_constraint`, `drop_foreign_key`, `add_foreign_key` or a `alter_column` operation with a changed logical type, the whole chain is rejected. The final diagram does not occupy these steps. The notification requires a manual transfer.
6. Erlaubte Kettenglieder sind `create_table`, `add_column`, `add_unique_constraint`, `add_index`, `rename_column`, `rename_table` und `alter_column` mit derselben logischen Art.
7. Verglichen werden Tabellen, Spalten, SQL-Typen, Nullbarkeit, Primärschlüssel, Defaults, Unique-Index-Namen, sekundäre Index-Namen und Foreign Keys nach Spalten, Zieltabelle, Zielspalte, `ON DELETE` und `ON UPDATE NO ACTION`. `text` entspricht `uuid` oder `string`. `integer`, `int` und `bigint` entsprechen `i64` oder, beim erwarteten Bool, `bool`. Bool-Defaults `1`/`true` und `0`/`false` gelten als gleich. Klammern und Anführungszeichen am Default werden entfernt. Ein zusätzlicher Nutzer-Index oder Foreign Key blockiert.
8. A missing or additional column or table is blocked. A suitable table name is not enough.
9. Only `--confirm` writes the history entries in a transaction.

After that, `migrate up` will not run the already marked files again.

## 12. Tests

`cargo test --workspace` runs the snapshot, diff and SQLite tests, including the CLI run against a temporary file. Covered are explicit column and table renaming, successful and failed conversion, blocked `NOT NULL` for existing `NULL` values, matching and deviating baselines, a destructive chain without baseline and `reseal` for an unchanged and already applied file. The development database `db.sqlite3` is not used. PostgreSQL remains ignored.

## 13. Not implemented

`migrate down`, subsequent foreign key change on existing columns, baseline for PostgreSQL, substitute values for `NULL`, advisory lock, automatic migration at server startup, MySQL, many-to-many and eager loading. Type changes other than `string` ↔ `i64` and other than nullity or bool default of the same type are rejected.
