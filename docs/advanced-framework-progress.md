# Advanced framework progress

Local evidence only. Nothing in this file is a merged release, a GitHub Actions result, or a closed issue.

## Block 2 — ORM and migrations

Branch: `feat/orm-migrations-lifecycle`, created from `main` at `53a95aa`. Implementation commit: `f3a663243b030eb3f391032c17a69314ff78abc3`. Progress note: `3bb51b951f904a7bb092c1910c7af8c7dc3332ed`.

Block 1 is not on this branch. Its implementation `d556bcc3bbfa5b624c5d27ef13d2682ee06b5652` remains on `fix/runtime-security-docker`. This branch does not relocate migration files or change the runtime-security work. Blocks 3 and 4 are also separate branches from `main`.

| Issue | Local implementation | Local tests | GitHub CI | Merge |
|---|---|---|---|---|
| [#64](https://github.com/Iam-Sanja/ferrioxid/issues/64) | Many-to-many, keyset and one annotation were already on `main`. This branch adds `load_one_to_one`. | SQLite and PostgreSQL 18.6, same example code | Not run | Not merged |
| [#81](https://github.com/Iam-Sanja/ferrioxid/issues/81) | `Select::aggregate` with group, measures, having and named filters | Same SQLite and PostgreSQL run | Not run | Not merged |
| [#82](https://github.com/Iam-Sanja/ferrioxid/issues/82) | `#[field(json)]`, canonical documents, forms, admin, fixtures and OpenAPI | Same run, plus the form binding test | Not run | Not merged |
| [#83](https://github.com/Iam-Sanja/ferrioxid/issues/83) | Read-only `migrate plan`, live SQLite and PostgreSQL catalog, checksum conflicts | SQLite file bytes unchanged; PostgreSQL disposable database | Not run | Not merged |

The shared application code is `crates/ferrioxid-db/examples/grouped_report`. `sqlite_grouped_report_scope_json_and_one_to_one` and the ignored `postgres_grouped_report_scope_json_and_one_to_one` both call it. The PostgreSQL test was executed locally on 12 November 2026 against PostgreSQL 18.6 through `DisposablePostgres`. The admin URL was the local `postgres` database. The populated databases `ferrioxid` (15 public tables) and `ferrioxid_migrations_down_test` (1 public table) had the same counts after the run. No `ferrioxid_test_*` database remained.

`postgres_plan_detects_live_drift_on_a_disposable_database` also passed on that server. A clean applied schema produced no finding. An added column was reported as drift. The disposable database was dropped.

## SQLite and PostgreSQL differences

- UUID columns are text on SQLite and `uuid` on PostgreSQL. Bigint aggregates are cast to `integer` on SQLite and `bigint` on PostgreSQL because PostgreSQL `SUM` of bigint is numeric.
- Average is computed in Rust from the integer sum and count. It is truncated toward zero at six decimal places. Overflow of `sum * 1_000_000` is an error. SQL `AVG` is not used.
- Character length uses SQLite `length` and PostgreSQL `char_length`. Both count characters for the text stored by this API. The bound is 0..=200 and applies only to text and choice columns.
- JSON is SQLite `text` plus `json_valid`, and PostgreSQL `jsonb`. Path operators, containment and JSON equality filters are not generated. Object keys in the canonical text are sorted.
- A JSON `null` document is distinct from SQL `NULL`.
- Boolean, decimal and JSON columns cannot be grouped. Decimal and non-bigint columns cannot be summed.
- Reports return at most 100 groups and have no offset. `HAVING` accepts only integer measures. Ordering by the average is rejected.
- One-to-one is a unique UUID foreign key plus an explicit loader. It is not a separate macro relation and it does not load automatically.
- `migrate plan` reads `sqlite_master` and `PRAGMA` on SQLite, and `information_schema` plus `pg_catalog` on PostgreSQL. History tables named `_ferrioxid_*` are ignored. Pending migrations are not compared as if they were already applied. A checksum or history-order error suppresses the live comparison.
- PostgreSQL baseline, automatic type changes to or from JSON, and MySQL remain unimplemented.
- Changing a column type is still limited to the existing supported pairs. Destructive changes still need `--allow-destructive` on `migrate up` and `migrate down`. The plan command does not perform them.

## Still open

- Issues stay open until the branch is pushed, reviewed, green in GitHub CI and merged.
- `postgres_keyset_annotations_and_many_to_many` was not executed in this pass. It targets whatever `DATABASE_URL` points at and drops its own tables there, so it was not pointed at the populated `ferrioxid` database.
- There is no automatic eager loading, no JSON path query, no decimal average and no offset inside a grouped report.
- Multi-column foreign-key order on PostgreSQL is taken from the catalog join. The plan test covers a table without a foreign key. A schema with secondary indexes and foreign keys still uses the same name and column comparison as the SQLite baseline.

## Other blocks

Blocks 3 and 4 are not part of this branch. `feat/native-admin` contains the record audit timeline. `feat/developer-apps` contains the project-file and environment aliases. Neither is mixed into these commits. GitHub CI has not run for this branch.
