# Fixtures

[Overview](index.md) · **Implemented in this pull request:** `crates/fexu-migrations/src/fixtures.rs` and `fexu dumpdata` / `fexu loaddata`.

Fixtures move rows. They are not migrations, they do not change tables, and the JSON file does not execute code.

## Format

```json
{
  "fexu": 1,
  "records": [
    {
      "model": "products",
      "pk": "11111111-1111-4111-8111-111111111111",
      "fields": {
        "name": "Power cable",
        "price_cents": 950,
        "price": "9.50",
        "status": "active",
        "active": true,
        "category_id": "22222222-2222-4222-8222-222222222222"
      }
    },
    {
      "model": "categories",
      "pk": "22222222-2222-4222-8222-222222222222",
      "fields": { "name": "Hardware" }
    }
  ]
}
```

`model` is the table name. `pk` is that table's `id`. A table without a single primary key named `id` is rejected before any write. Decimal values are JSON strings, never JSON numbers. Choice values are the stored token (`active`), not the display label. Dates, times, datetimes and UUIDs are strings. Integers are signed 64-bit JSON numbers. Booleans are `true` or `false`. A date must be `YYYY-MM-DD`. A time is `HH:MM` or `HH:MM:SS` and is dumped as `HH:MM:SS`. A datetime must be UTC (`Z` or `+00:00`) and is dumped with `Z`, including a fractional second when one was stored. Other offsets and invalid calendar values are rejected before a write.

The loader orders records by foreign keys. A product may appear before its category in the file. The category is inserted first when both rows are in the same file. A foreign key that points at a row already stored in the database is also accepted, including a parent table declared as another app's external table.

## Commands

```powershell
fexu dumpdata
fexu dumpdata --model products --output fixtures/products.json
fexu dumpdata --app shop
fexu loaddata fixtures/catalog.json
fexu loaddata fixtures/catalog.json --dry-run
fexu loaddata fixtures/catalog.json --upsert
fexu loaddata fixtures/catalog.json --model products
fexu loaddata fixtures/catalog.json --truncate --yes
```

`dumpdata` prints the fixture, or replaces the file named by `--output`. It prints row contents. Do not point it at a database you are not allowed to export.

`loaddata` reads and validates the file before it opens a write. The default is insert-only. An existing primary key is an error, the transaction rolls back, and existing rows stay as they were. `--upsert` updates those rows. `--truncate` deletes every row of the selected tables and then inserts the file. It does nothing unless `--yes` is also present. `--upsert` and `--truncate` together are rejected. `--dry-run` checks the file and references and writes nothing. `--model` keeps one table and does not pull in parent tables automatically. `--app` uses that layout-2 app's schema. `--schema-file` and `--database-url` work as they do for `migrate`.

Exit code `0` means the command finished. Exit code `2` means the invocation itself was refused, such as `--truncate` without `--yes`. Exit code `1` means the file, schema, or database rejected the operation.

## Library

```rust
use fexu::migrations::{dump_fixture, load_fixture, parse_fixture, LoadRequest};

let file = parse_fixture(include_str!("../fixtures/catalog.json"))?;
let request = LoadRequest {
    upsert: true,
    ..LoadRequest::default()
};
let report = load_fixture(&db, &schema, &file, &request).await?;
assert!(report.written);
```

`LoadRequest::default()` inserts and does not delete. `prepare_records` checks models, fields, choices, decimals and cycles without a database. Unknown models, unknown fields, unknown choices, a decimal with too many fractional digits, a missing parent and a foreign-key cycle all return an error. A cycle is not rewritten into nulls.

## Databases and limits

SQLite and PostgreSQL use the same JSON file. Decimal columns are stored as the 16-byte coefficient from [models.md](models.md), not as `REAL` or text, so a later dump prints the same decimal string. Choice columns stay text. On PostgreSQL, UUID, date, time and datetime values are bound as native parameters; SQLite binds the canonical text. Nulls use the same typed parameter family. Identifiers in SQL are quoted and must already be plain identifiers; values are bound parameters.

The import is one transaction. A failure rolls back every insert and every `--truncate` delete from that command. SQLite foreign keys are enabled on the Fexu connection, so a truncate that would orphan a row outside the selected tables fails and rolls back.

These cases are not reordered or repaired:

- a foreign-key cycle, including two rows in the same table that point at each other
- a composite foreign key, or a foreign key that does not target `id`
- a primary key that is not named `id`
- natural keys
- fixture files that contain more than one document

`examples/myshop/fixtures/catalog.json` loads one category and two products. The product list is intentionally written before the category. `examples/myshop/tests/catalog_fixture.rs` loads it, rejects a second plain load, accepts `--upsert` semantics through `LoadRequest`, rolls back a conflicting file, and reads the same rows after the SQLite file is closed and opened again.

`postgres_fixture_roundtrip` is `#[ignore]` and runs in CI when PostgreSQL is available. A local workspace test run does not mark it as passed.
