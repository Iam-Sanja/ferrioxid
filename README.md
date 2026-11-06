# Ferrum — Model-first Rust backend (experimental)

Funktionsstand v0.8.1. Die Cargo-Version bleibt `0.1.0`. Die Entwicklerdokumentation ist Deutsch. Der Admin liegt unter `/admin`, die Beschreibung in `docs/admin.md`. Vor `cargo build` einmal `npm run build` in `apps/admin-ui` ausführen, damit das Frontend im Binary liegt. `FERRUM_CSRF_SECRET` ist in Produktion 64 Hex-Zeichen.

Authentifizierung, Sitzungen und Rollen beschreibt `docs/auth.md`. Geschützte myshop-Ressource ist `/api/v1/internal_notes`. Produkte und Kategorien bleiben ausdrücklich öffentliche Demo-Routen.

> This is **not** a complete Django replacement. The current milestone covers a Rust derive macro, shared model metadata, one SeaORM/SeaQuery repository for SQLite and PostgreSQL, and generated list, detail, create, patch and delete routes.

SQLite is the default local database. Docker and PostgreSQL are optional.

## Quick start

Prerequisites: current stable Rust and Cargo. No database server is required.

```powershell
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

Unset `DATABASE_URL` uses `sqlite://db.sqlite3?mode=rwc` in the process working directory. The file is created on first connect and is gitignored.

Create and update the schema with migrations, from the workspace root:

```powershell
cargo run -p ferrum-cli -- migrate make initial
cargo run -p ferrum-cli -- migrate up
cargo run -p ferrum-cli -- migrate status
cargo run -p ferrum-cli -- migrate rename-column --table products --from name --to title
cargo run -p ferrum-cli -- migrate baseline --confirm
cargo run -p ferrum-cli -- admin create-user --email admin@example.com --superuser
cargo run -p myshop
```

`myshop` already contains `examples/myshop/migrations/0001_initial.json`. On a new SQLite file, `migrate up` is enough. `ferrum.toml` points the CLI at the myshop model registry. The CLI does not parse Rust source. `--bootstrap` is legacy: it no longer creates tables, does not write migration history, and does not delete existing files.

In another terminal:

```bash
curl http://127.0.0.1:3000/healthz
curl -i http://127.0.0.1:3000/api/v1/products
curl -i -X POST http://127.0.0.1:3000/api/v1/products \
  -H 'Content-Type: application/json' \
  -d '{"name":"Laptop Pro","price_cents":129900,"active":true}'
curl http://127.0.0.1:3000/api/v1/products
```

`myshop --bootstrap` does not create or migrate a schema. Use `ferrum migrate up` before serving.

### Project files

- `examples/myshop/src/models/product.rs` — developer-facing model
- `crates/ferrum-macros/src/lib.rs` — Rust derive macro
- `crates/ferrum-core/src/lib.rs` — descriptors, errors and core trait
- `crates/ferrum-db/src/lib.rs` — repository contract and database connection
- `crates/ferrum-db/src/sql.rs` — SeaQuery DDL and CRUD for SQLite and PostgreSQL
- `crates/ferrum-api/src/lib.rs` — route registration, handler adapters and bootstrap
- `crates/ferrum/src/lib.rs` — public framework API
- `crates/ferrum-cli/src/main.rs` — experimental informational `ferrum doctor` CLI
- `crates/ferrum-admin/` — Admin-API, Rechteprüfung und Auslieferung der Oberfläche
- `apps/admin-ui/` — React-Oberfläche, gebaut nach `apps/admin-ui/dist`

## Model example

```rust
use ferrum::prelude::*;

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(required, index, min_length = 3, max_length = 150)]
    pub name: String,
    #[field(min = 0)]
    pub price_cents: i64,
    #[field(default = true)]
    pub active: bool,
}
```

Registration is explicit: `App::new(db).register::<Product>()`.

Registration generates these routes for the model path:

```http
GET     /api/v1/products
GET     /api/v1/products/{id}
POST    /api/v1/products
PATCH   /api/v1/products/{id}
DELETE  /api/v1/products/{id}
```

`GET /{id}` and `PATCH /{id}` return the typed model. A missing row is `404`
with Ferrum problem JSON. `DELETE /{id}` returns `204` with an empty body when
a row was removed, and `404` when it was already gone. `{id}` must be a UUID;
Axum rejects other path values with its own response. Routes inherit
`allow_anonymous` and return `403` when it is false.

`PATCH` with `{}` returns the current row unchanged. A missing JSON key keeps
the stored value. JSON `null` on a non-null column is `422`. JSON `null` on an
`Option<T>` column stores SQL `NULL`. String rules use the trimmed length and
store the original text.

## Important security notice

`allow_anonymous = true` is only for a disposable **local demo**. Models default to denied access. There is no production-ready login, authentication, authorization, rate limiting, CSRF system, or administrative UI in v0.1. **Do not expose the demo API to the public internet.** The example binds to `127.0.0.1` by default. Any other `BIND_ADDRESS` may expose the service to additional networks; `allow_anonymous = true` is only appropriate for localhost demos. The sample PostgreSQL password is for local development only.

## Scope and limitations

**Included in source:** workspace; macros for `Uuid`, `String`, `i64`, `bool` and `Option` of those types except a nullable `id`; typed create and update DTOs; `PatchField`; trimmed string validation; Boolean defaults rendered per dialect; generated list, read-by-UUID, create, patch and delete; single-column and composite unique constraints; secondary indexes; single-column foreign keys with `restrict`, `cascade` and `set_null`; lazy belongs-to and has-many methods; SeaQuery SQL for SQLite and PostgreSQL; versioned migrations for those objects, explicit renames, checked SQLite rebuilds, explicit destructive drops, and a confirm-only SQLite baseline; SQLite migration, constraint and HTTP tests in `cargo test --workspace`; optional Docker Compose PostgreSQL; the myshop category/product example and CLI.

**Not included:** many-to-many, eager or batch loading, changing a foreign key on an existing column, heuristic renames, `migrate down`, PostgreSQL baseline, admin UI, auth, RBAC, OpenAPI, pagination beyond 100 rows, optimistic version columns, `DateTime` or JSON columns, MySQL, and a Dockerfile for the API. The ignored PostgreSQL tests are not part of the default run. Do not assume otherwise.

## Development checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

## Ausführliche Entwicklerdokumentation (Deutsch)

**[Dokumentationsportal: docs/index.md](docs/index.md)** – Architektur, Modulgrenzen, Datenflüsse, Codegenerierung, Datenbank, API, CLI, Konfiguration, Validierung, Sicherheit, Tests und Beispielanwendung. Constraints stehen in [docs/constraints.md](docs/constraints.md), Beziehungen in [docs/relations.md](docs/relations.md), Migrationen in [docs/migrations.md](docs/migrations.md). Authentifizierung, Admin, OpenAPI, Plugins und Deployment bleiben Entwürfe, soweit die jeweilige Seite das so ausweist.

Jede Seite trennt **IST** von **SOLL (nur geplant)**. Der Funktionsstand dieser Constraints und Beziehungen ist v0.6. Die Cargo-Version bleibt `0.1.0`. Änderungen am Framework müssen auch in der passenden Moduldokumentation nachgetragen werden. Der [Implementierungsleitfaden](docs/implementation-guide.md) definiert dafür einen verbindlichen Änderungs- und Testprozess.

Siehe außerdem [Architektur](docs/architecture.md), [Roadmap](docs/roadmap.md) und [Tests](docs/testing.md).

## API list response

GET requests return `{ "data": [...], "limit": 100, "truncated": false }`.
`truncated` is `true` when the table contains more than 100 records. This is **not pagination**.

`#[field(required)]` on a String rejects blank/whitespace-only text, and length
constraints count trimmed Unicode characters. Boolean defaults are persisted
in SQL DDL. PostgreSQL stores them as boolean defaults. SQLite stores them as integer `1` or `0`. `CREATE TABLE IF NOT EXISTS` **does not update old tables**.

## Databases

| Backend | How to select it | Role |
|---|---|---|
| SQLite file | leave `DATABASE_URL` unset | default local development |
| SQLite memory | `sqlite::memory:` | tests; Ferrum uses one pooled connection |
| PostgreSQL | `DATABASE_URL=postgres://...` | supported server backend |

Optional PostgreSQL, after a server is actually running:

```powershell
docker compose up -d db
$env:DATABASE_URL="postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum"
cargo test -p myshop --test postgres_api -- --ignored --nocapture
```

The ignored test **requires `DATABASE_URL`** and drops its fixed test tables. Use only a disposable database. `cargo test --workspace` already runs the same HTTP flow against SQLite memory and does not need this step.

`ferrum doctor` prints setup hints only. `.env.example` is not loaded automatically.

`FerrumModel::schema_sql` now takes `DatabaseKind` and returns `String`. Callers of the old `schema_sql() -> &'static str` must pass the backend.
