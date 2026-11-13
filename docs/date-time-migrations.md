# Migrations for date and time

**Milestone:** v0.19.

`LogicalType` has `date`, `time` and `date_time`. The snapshot and the checksum treat them like the previous types.

| Backend | Date | Time | DateTime |
|---|---|---|---|
| SQLite | `TEXT` | `TEXT` | `TEXT` |
| PostgreSQL | `DATE` | `TIME` | `TIMESTAMPTZ` |

SQLite stores canonical strings, so sorting and prefix filters follow ISO ordering. PostgreSQL binds typed chrono values, and the schema renderer maps them to the types above. The original v0.19 verification did not include a PostgreSQL runtime test.

## New columns

A new nullable or mandatory column is created via `fexu migrate make`. myshop attaches `events` as `migrations/0004_events.json` to `0003_internal_notes`. The files `0001_initial.json`, `0002_categories_and_product_relations.json`, `0003_internal_notes.json` and `crates/fexu-auth/migrations/0002_admin_journal.json` are unchanged.

## Blocked conversion

Type conversion is supported only between `String` and `I64`. Converting `String` to `Date`, `Time` or `DateTime` is rejected by `plan_change`; existing rows are not silently repaired. Explicit temporal data conversion remains unimplemented.

Nullable and defaults follow the previous rules. A bool default does not apply to date fields.
