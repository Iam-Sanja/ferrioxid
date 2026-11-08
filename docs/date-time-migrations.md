# Migrations for date and time

**Stand:** v0.19.

`LogicalType` has `date`, `time` and `date_time`. The snapshot and the checksum treat them like the previous types.

| Backend | Date | Time | DateTime |
|---|---|---|---|
| SQLite | `TEXT` | `TEXT` | `TEXT` |
| PostgreSQL | `DATE` | `TIME` | `TIMESTAMPTZ` |

SQLite stores the canonical string. Sorting and prefix filters follow the ISO order. PostgreSQL binds the chrono values typed. The image is present in the schema renderer. A PostgreSQL runtime test of this version was not executed.

## Neue Spalten

A new nullable or mandatory column is created via `ferrioxid migrate make`. myshop attaches `events` as `migrations/0004_events.json` to `0003_internal_notes`. The files `0001_initial.json`, `0002_categories_and_product_relations.json`, `0003_internal_notes.json` and `crates/ferrioxid-auth/migrations/0002_admin_journal.json` are unchanged.

## Blocked conversion

`String` and `I64` remain the only supported type change. `String` after `Date`, `Time` or `DateTime` generates a rejection in `plan_change`. There is no silent repair of invalid stock lines. A later, explicitly tested data conversion is not part of v0.19.

Nullable and defaults follow the previous rules. A bool default does not apply to date fields.
