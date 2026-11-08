# myshop – integration example

[Overview](index.md) · **Implemented files:** `examples/myshop/src/lib.rs`, `examples/myshop/src/main.rs`, `examples/myshop/src/models/category.rs`, `examples/myshop/src/models/product.rs`.

## Purpose

`myshop` demonstrates the public Ferrioxid APIs through the same facade used by other applications. It enables `admin-ui`. Use `ferrioxid new` for an independent project; see [project-generator.md](project-generator.md). Products and categories have anonymous demo routes without object policies; create policies are covered separately in `auth.rs` and `admin_http.rs`. The example serves `/`, `/static/css/app.css` and a `/contact` form that validates without sending email. Uploads require a session. `ferrioxid-middleware` supplies request IDs and security headers.

## Model (Implemented)

`Category` has `id` and a unique `name`. `Product` has `id`, a mandatory indexed name with 3-150 characters, `price_cents`, `active` with default `true` and an optional `category_id`. The field references `on_delete = "restrict"` to `categories.id`. `product.category(&db)` loads the category, `category.products(&db)` loads the products. Both demo APIs are explicitly anonymous. This applies only to the local process. `InternalNote` is below `/api/v1/internal_notes`, requires `notes.read|create|update|delete` and is the protected resource. `0003_internal_notes.json` creates its table.

## Start with SQLite (Implemented)

```powershell
cargo run -p ferrioxid-cli -- migrate up
cargo run -p myshop
```

The commands belong in the workspace root, because there is `ferrum.toml`. Without `DATABASE_URL`, CLI and myshop use `sqlite://db.sqlite3?mode=rwc` in the working directory. `0001_initial.json` creates `products` and remains unchanged. `0002_categories_and_product_relations.json` creates `categories`, adds the nullable `category_id` with Foreign Key and creates `ix_products_name`. The server starts on `127.0.0.1:3000` if `BIND_ADDRESS` is not set and does not migrate itself. User creates `ferrioxid admin create-user`. Login, cookie, `/auth/me`, protected access, roles and logout shows `examples/myshop/tests/auth.rs`.

`cargo run -p myshop -- --bootstrap` aborts without deleting files, creating tables or writing history. `cargo run -p myshop -- --print-schema` outputs all registered models for `schema_command`.

PostgreSQL is optional:

```powershell
$env:DATABASE_URL="postgres://ferrioxid:ferrioxid_dev_only@127.0.0.1:5432/ferrioxid"
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

`.env.example` is not loaded. Docker is not required for SQLite execution.

### API smoke test

```powershell
curl.exe -i -X POST http://127.0.0.1:3000/api/v1/products -H "Content-Type: application/json" -d "{\"name\":\"Laptop\",\"price_cents\":1250}"
curl.exe -i http://127.0.0.1:3000/api/v1/products
```

POST answers 201 with UUID and `active: true`. GET answers 200 with `data`, `limit: 100` and `truncated: false` as long as there are at most 100 lines. `PATCH /api/v1/products/{id}` with `{"name":"Notebook"}` answers 200 and keeps price and `active`. `DELETE` answers 204 without a body. `sqlite_http_roundtrip` checks the same answers in `cargo test --workspace`. The PostgreSQL test remains separate and ignored.

## Limitations

- No production-ready shop application.
- The admin and login are implemented, but product and category CRUD APIs are intentionally public. Relationship methods do not create nested HTTP routes.
- Schema changes come from `ferrioxid migrate`, not `--bootstrap`. An older `products` table without history is not marked at start. If your schema fits the migration chain and the chain does not contain a deletion or type change, `ferrioxid migrate baseline --confirm` can only write the history. Without `--confirm`, the file remains unchanged.
- `allow_anonymous=true` does not operate publicly.

## Tests

`examples/myshop/tests/relations.rs` creates category and product, loads both directions, rejects a duplicate name and an invalid foreign key, and applies `0001` and `0002` to a temporary file. `examples/myshop/tests/postgres_api.rs` includes the SQLite HTTP test and the ignored PostgreSQL test. `examples/myshop/tests/schema_migration.rs` retains `0001` to the original product model. Only the PostgreSQL run deletes fixed test tables in the database selected by `DATABASE_URL`. `--bootstrap` does not run against an unsecured production database. Details are available in [testing.md](testing.md).

## Admin

`Product`, `Category` and `InternalNote` carry `admin = true`. `main.rs` registers them in `AdminRegistry`. Public product and category routes remain anonymous. Internal notes remain protected by `notes`. The admin interface requires staff and the respective model right. It is below `http://127.0.0.1:3000/admin` and does not need Node.js. Migrations do not change as a result.
