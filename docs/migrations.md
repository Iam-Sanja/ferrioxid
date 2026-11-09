# Migration engine

[Overview](index.md) · **Implemented:** `crates/ferrioxid-migrations/`. **Not** the legacy command `myshop --bootstrap`. Date types: [date-time-migrations.md](date-time-migrations.md). Auth `0003_audit_operation` complements `operation_id`. myshop `0005_orders` complements `orders` and `order_lines`. `0001` through `0004` and Auth `0001`/`0002` remain unchanged.

## 1. Architecture

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

The engine does not parse Rust source and depends on neither Axum nor the admin frontend. CRUD and migrations share the type mappings in `crates/ferrioxid-db/src/sql.rs`.

App packages use the same migration-history table. An entry is namespaced as `shop:0001_initial`, while the file ID remains `0001_initial`; see [app-migrations.md](app-migrations.md). Existing unqualified history rows are not rewritten. Startup compares `registry.migrations` with `[app.<name>]`; the CLI reads only the TOML declaration and reports that limit. A field group creates no table on its own: its fields appear in `columns()` of concrete models and therefore in schema diffs.

Auth files are in `crates/ferrioxid-auth/migrations/` and write the history `_ferrioxid_auth_migrations`. `ferrioxid migrate up` applies this directory before the application files. The application history remains `_ferrioxid_migrations`. As a result, `0001_auth_schema` does not collide with `0001_initial`. The server start does not migrate. `0001_initial.json` remains unchanged.

| Responsibility |File|
|---|---|
|Snapshot and Validation| `crates/ferrioxid-migrations/src/snapshot.rs` |
|Diff and risk| `crates/ferrioxid-migrations/src/diff.rs` |
|File format and checksum| `crates/ferrioxid-migrations/src/store.rs` |
| History table | `crates/ferrioxid-migrations/src/history.rs` |
|Implementation| `crates/ferrioxid-migrations/src/runner.rs` |
| Snapshot simulation | `crates/ferrioxid-migrations/src/simulate.rs` |
|SQLite conversion and PostgreSQL-ALTER| `crates/ferrioxid-migrations/src/rebuild.rs` |
| SQLite introspection | `crates/ferrioxid-migrations/src/introspection.rs` |
| Baseline | `crates/ferrioxid-migrations/src/baseline.rs` |
|`ferrioxid.toml` and Schema command| `crates/ferrioxid-migrations/src/project.rs` |
| CLI | `crates/ferrioxid-cli/src/main.rs` |
|myshop registration| `examples/myshop/src/main.rs`, `ferrioxid.toml` |
| DDL | `crates/ferrioxid-db/src/sql.rs` |

`ColumnSpec` and `TableConstraints` remain the model metadata. Snapshots convert them into logical types, unique constraints, indexes and foreign keys. SQL is rendered for a specific `DatabaseKind`. Single-column foreign keys are supported; changing a foreign key on an existing column remains blocked. See [constraints.md](constraints.md).

## 2. Registration

`ferrioxid.toml` in the project or an overarching directory:

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

Logical types include `uuid`, `string`, `i64`, `bool`, `date`, `time` and `date_time`. Unknown format versions, duplicate names and invalid identifiers are rejected. Snapshots contain neither connection details nor passwords. Exactly one non-nullable primary key is required; defaults are currently Boolean only.

Tables may also contain `unique_constraints`, `indexes` and `foreign_keys`. Empty lists are omitted from serialization. Older format-version-1 files without these keys remain valid and keep their checksums; already applied migrations do not change meaning. See [constraints.md](constraints.md) for generation, NULL semantics and blocked foreign-key changes.

The same schemas shall be considered as unchanged even if only the column order differs. No migration is written for this.

## 4. Diff

`plan_change` compares the last confirmed target snapshot with the current model. It does not create a renaming.

| Change | Operation | Execution |
|---|---|---|
| New table | `create_table` | Safe |
|New nullable column or `bool` with default| `add_column` | Safe |
|New `NOT NULL` column without default| `add_column` | Blocked unless `--backfill table.column:kind=value` supplies an explicit value. Otherwise add the column as nullable, fill the rows, then set `NOT NULL`. No value is invented. |
|New primary key columns| `add_column` | Blocked |
|Delete the table| `drop_table` |only with `--allow-destructive`|
|Delete column, no primary key| `drop_column` |only with `--allow-destructive`|
|Delete primary key| `drop_column` | Blocked |
| `string` ↔ `i64` | `alter_column` |only after value check|
| Same logical type, changed nullability or Boolean default | `alter_column` | Only after validating existing values |
|Other type pairs, primary key flag| `alter_column` | Blocked |
| One column removed and another added | Drop plus Add | Blocked; use an explicit rename |
| Replaced table | Drop plus Create | Blocked; an explicit rename is required. |
| Explicit rename | `rename_column`, `rename_table` | Explicit rename commands only; affected indexes are recreated. |
| New unique index | `add_unique_constraint` | Safe; existing duplicates cause the migration to fail. |
| Remove unique index | `drop_unique_constraint` | Existing rows remain; future duplicates become possible. |
| New index | `add_index` | Safe |
| Remove index | `drop_index` | Safe |
| Foreign key on a new column | `add_column` with `foreign_key` | Safe when the column is nullable, has a Boolean default, or an explicit backfill. |
| Add a foreign key on an existing column | `add_foreign_key` | Executed after existing values are checked. SQLite rebuilds the table; PostgreSQL uses `ADD CONSTRAINT`. Invalid values roll the migration back. |
| Remove a foreign key or change its delete rule | `drop_foreign_key` | Blocked; no file is written. |

`migrate make` does not write a file when renaming is suspected. The message contains “not renaming” and refers to `rename-column` or `rename-table`.

## 4.1 Explicit renaming

First change the model to the new name. After that, in the workspace root:

```powershell
cargo run -p ferrioxid-cli -- migrate rename-column --table products --from name --to title
cargo run -p ferrioxid-cli -- migrate rename-table --from products --to items
```

The CLI loads the registered schema. One operation must fully explain this schema from the last migration target. Further deviations result in no file being written. The following `migrate make` will then see no change.

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

`reseal` checks that the operations generate the target snapshot, rewrites the checksum and refuses the file if its ID is already in `_ferrioxid_migrations`. An applied file with changed checksum stops `migrate up`.

## 5. File format

```text
examples/myshop/migrations/0001_initial.json
```

Each file contains format version, ID `0001_name`, name, source snapshot, target snapshot, operations, risks and a SHA-256 checksum. The checksum covers the canonical JSON body without the checksum field. It recognises substantive changes. It does not prove that the database still corresponds exactly to this schema.

Operation variants use the external tag `type` with `snake_case` values: `create_table`, `drop_table`, `add_column`, `drop_column`, `alter_column`, `rename_column`, `rename_table`, `add_unique_constraint`, `drop_unique_constraint`, `add_index`, `drop_index`, `add_foreign_key` and `drop_foreign_key`. Both foreign-key variants are recognized but neither generated nor executed. `add_column` may include an optional `foreign_key`; older files without it remain valid. When loading a file, simulating its operations against the source snapshot must produce the target snapshot. The operation list need not match the automatic diff: a file may therefore contain an explicit rename even when `plan_change` would reject the corresponding automatic change.

The first migration must start from an empty schema. Each subsequent migration must be based exactly on the target snapshot of the previous one. IDs are numbered in four digits, maximum `9999`.

## 6. History

Table `_ferrioxid_migrations`:

| Column | Meaning |
|---|---|
| `id` |Primary key, migration ID|
| `name` | Short name |
| `checksum` |Checksum at time of application|
| `applied_at_unix` | Unix seconds |

The row is written in the same transaction as the schema change. If the migration fails, there is neither the schema change nor the history entry. There is no stored error status. An applied ID is not executed again. A missing file or a different checksum stops `migrate up`.

A history entry does not guarantee that no one has changed the schema from the outside afterwards. Ferrioxid compares the history with the files, but does not read back the complete live schema.

## 7. CLI

From the workspace root:

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

`status` shows backend, auth migrations and app migrations separately, applied or pending, checksums and conflicts. If the model schema could be loaded, it indicates whether it fits the last app migration target. Pending migrations leave the command successful. Checksum, unknown IDs and sequence errors terminate it with Exitcode 1. The diagnosis opens SQLite with `mode=ro` and does not create a missing file. The database URL is not output. `--database-url` and `--migrations-dir` overwrite project and environment. Without URL, `DATABASE_URL` applies, otherwise `sqlite://db.sqlite3?mode=rwc` in the working directory. The joint report is published in [migration-diagnostics.md](migration-diagnostics.md).

Deletion risks (`drop_table`, `drop_column`) indicate `--allow-destructive` to the CLI before `up`. A `rebuild_column` risk is not a deletion release. A failed check ends with exit code 1 and is not issued as a success. `baseline` without `--confirm` is such a case, even if the schema fits.

## 8. SQLite

SQLite is the tested backend. `connect` keeps a connection and sets `PRAGMA foreign_keys=ON`. Tests use temporary files. `PRAGMA foreign_keys` is not switched inside a transaction, because SQLite would ignore that change. A table rebuild turns enforcement off and back on around its transaction on the same physical connection. That connection is not returned to the pool while enforcement is off; if it cannot be restored, it is closed. `PRAGMA foreign_key_check` must be empty before the history row is committed.

`CREATE TABLE` including Foreign Keys, `CREATE UNIQUE INDEX`, `CREATE INDEX`, secure `ADD COLUMN` optionally with `REFERENCES`, `DROP INDEX`, `RENAME TABLE`, `RENAME COLUMN`, as well as `DROP TABLE` and `DROP COLUMN` only after express approval. Each migration runs in a transaction along with the history entry. A unique index that violates existing lines rolls back this transaction.

### Table rebuild

SQLite implements `string` ↔ `i64` conversions and changes to nullability or Boolean defaults by rebuilding the table, since it cannot generally perform them with `ALTER COLUMN`. The sequence in `rebuild.rs` is:

1. Check initial and target columns. Unsupported pairs, primary key changes and type changes without verified conversion are rejected.
2. A view that belongs to the table aborts the rebuild before DDL. Indexes and unique constraints are recreated from the target snapshot. Trigger SQL is read from `sqlite_master` and executed again after the rename. The runner suspends foreign-key enforcement on the connection that owns the transaction. `PRAGMA defer_foreign_keys = ON` covers the table swap, and `foreign_key_check` must be empty before commit.
3. If restricted to `NOT NULL`, count the existing `NULL` values. If the number is greater than 0, it is aborted. No substitute value is used.
4. Read and validate the affected column in Rust. Preserve `NULL`. Values must match the logical type; Booleans must be `0` or `1`. Converting `i64` to `string` uses decimal notation. Converting `string` to `i64` requires an exact round trip through `i64::to_string`, rejecting `01`, `+1`, `1.0`, `-0` and empty text. A successful SQL `CAST` alone is insufficient.
5. Create `ferrioxid_rebuild_<tabelle>` with the target schema.
6. `INSERT INTO ... SELECT` copies the remaining columns. Only the tested type change is used as `CAST`. `name` is not guessed as `title`.
7. Compare `COUNT(*)` from source and copy.
8. Delete the old table and rename the copy to the old name.
9. Compare the number of lines again.
10. `PRAGMA integrity_check` must deliver `ok`. `PRAGMA foreign_key_check` must be empty.
11. Write the history entry and commit the transaction.

The remaining columns are not loaded into the process memory. The checked column will continue to be read in full so that invalid values are noticed before the `CAST`.

### Rollback

In this transaction, SQLite can be fully rolled back: `CREATE TABLE`, `INSERT`, `DROP TABLE`, `ALTER TABLE ... RENAME` and the history entry. If one step fails, the transaction is rejected. The failed migration is not considered to be applied. Foreign-key enforcement is restored even when the rebuild rolls back. Changes outside this transaction, in particular an already established `ensure_history` in the normal `migrate up`, will remain. The baseline creates the history table only in the confirmation transaction.

## 9. PostgreSQL

The same list of operations is executed, but not through the SQLite rebuild. `CREATE TABLE`, `CREATE INDEX`, secure `ADD COLUMN`, `RENAME` and released deletions use the PostgreSQL dialect. A violating Unique Index or Foreign Key makes the transaction fail. `add_foreign_key` adds a constraint after the same orphan check used on SQLite. `drop_foreign_key` stays blocked on both dialects. A required column with an explicit backfill is added nullable, updated with that literal, then set `NOT NULL`. A supported column change also checks the values in Rust and then executes `ALTER COLUMN`: type over `TYPE ... USING`, default over `SET DEFAULT` or `DROP DEFAULT`, nullity over `SET NOT NULL` or `DROP NOT NULL`. Unsupported changes are also rejected here.

Baseline on PostgreSQL immediately breaks off and does not write a history. There is no silent partial takeover.

The `postgres_migration_roundtrip` test is `#[ignore]` and requires `DATABASE_URL`. It was not executed against a server here. An ignored test is not a success.

Not implemented: Advisory locks against parallel migrators and `down`.

## 10. Errors and transactions

Before executing SQL, the runner checks checksums, ordering and whether each operation is supported and authorized. A rejected operation stops the run. Earlier successful migrations remain applied; the failed migration and later files are not applied.

`CREATE TABLE` without `IF NOT EXISTS` hits an existing table, rolls back and does not mark it as migrated. The message refers to a new database. Existing files are not deleted.

## 11. Bootstrap

`cargo run -p myshop -- --bootstrap` is Legacy. The command does not write a schema and history.

- History present: cancellation, reference to `migrate status`.
- Table `products` without history: cancellation, no deletion, no marking as migrated.
- Empty database: cancellation, reference to `migrate make` and `migrate up`.

`App::bootstrap_schema` remains a test assistant for empty databases without history. If `_ferrioxid_migrations` exists, this function does not create anything. It is not a migration and does not recognize schema drift.

The HTTP server does not migrate at startup.

### Transition of an old SQLite file

`ferrioxid migrate baseline` handles a SQLite file whose schema already corresponds to the last migration destination. The command does not change a table or a row. Without `--confirm`, no history is written, and the process ends with an error that mentions the successful verification and the missing confirmation.

Procedure:

1. Open connection. PostgreSQL is rejected.
2. Inspect the schema through `sqlite_master`, `PRAGMA table_info`, `PRAGMA index_list`, `PRAGMA index_info` and `PRAGMA foreign_key_list`. Ignore `_ferrioxid_migrations`, `sqlite_%` and the primary-key autoindex.
3. Existing history entries reject the baseline.
4. The migration chain must be consistent from the empty schema.
5. Reject the entire chain if any file contains `drop_table`, `drop_column`, `drop_index`, `drop_unique_constraint`, `drop_foreign_key`, `add_foreign_key` or an `alter_column` that changes logical type. The final live schema cannot prove that these historical operations occurred; manual migration is required.
6. Allowed chain operations are `create_table`, `add_column`, `add_unique_constraint`, `add_index`, `rename_column`, `rename_table` and `alter_column` within the same logical type.
7. Comparison covers tables, columns, SQL types, nullability, primary keys, defaults, unique-index names, secondary-index names and foreign keys, including source column, target table, target column, `ON DELETE` and `ON UPDATE NO ACTION`. `text` matches `uuid` or `string`. `integer`, `int` and `bigint` match `i64`, or `bool` when a Boolean is expected. Boolean defaults `1`/`true` and `0`/`false` are equivalent. Parentheses and quotes are stripped from defaults. An additional user index or foreign key blocks the baseline.
8. A missing or additional column or table is blocked. A suitable table name is not enough.
9. Only `--confirm` writes the history entries in a transaction.

After that, `migrate up` will not run the already marked files again.

## 12. Tests

`cargo test --workspace` runs the snapshot, diff and SQLite tests, including the CLI run against a temporary file. Covered are explicit column and table renaming, successful and failed conversion, blocked `NOT NULL` for existing `NULL` values, matching and deviating baselines, a destructive chain without baseline and `reseal` for an unchanged and already applied file. The development database `db.sqlite3` is not used. PostgreSQL remains ignored.

## 13. Not implemented

`migrate down`, subsequent foreign key change on existing columns, baseline for PostgreSQL, substitute values for `NULL`, advisory lock, automatic migration at server startup, MySQL, many-to-many and automatic eager loading. Explicit batch reads are described in [relations.md](relations.md). Type changes other than `string` ↔ `i64` and other than nullity or bool default of the same type are rejected.
