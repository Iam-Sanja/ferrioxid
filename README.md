# Ferrum v0.1 — Model-first Rust backend (experimental)

> **Status: source-code prototype; detail-route extension not yet compiled in this environment.** The initial release compiled in the user's Cursor environment after an orphan-trait fix. The new changes in this archive have not been compiled in the generation environment (Rust/Cargo unavailable). Run the commands below locally and resolve any incompatibilities before use.
>
> This is **not** a complete Django replacement. The first milestone covers a Rust derive macro, shared model metadata, a PostgreSQL repository, and generated list/create/detail API routes.

## Quick start (Linux/macOS; VS Code terminal)

Prerequisites: current stable Rust/Cargo, Docker with Compose, free localhost ports 5432 and 3000.

```bash
docker compose up -d db
cargo check --workspace
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

In another terminal:

```bash
curl http://127.0.0.1:3000/healthz
curl -i http://127.0.0.1:3000/api/v1/products
curl -i -X POST http://127.0.0.1:3000/api/v1/products \
  -H 'Content-Type: application/json' \
  -d '{"name":"Laptop Pro","price_cents":129900,"active":true}'
curl http://127.0.0.1:3000/api/v1/products
```

The bootstrap command uses `CREATE TABLE IF NOT EXISTS` and is **not** a migration engine. Bootstrap before serving.

### Project files

- `examples/myshop/src/models/product.rs` — developer-facing model
- `crates/ferrum-macros/src/lib.rs` — Rust derive macro
- `crates/ferrum-core/src/lib.rs` — descriptors, errors and core trait
- `crates/ferrum-db/src/lib.rs` — repository contract and database connection
- `crates/ferrum-api/src/lib.rs` — route registration, handler adapters and bootstrap
- `crates/ferrum/src/lib.rs` — public framework API
- `crates/ferrum-cli/src/main.rs` — experimental informational `ferrum doctor` CLI

## Model example

```rust
use ferrum::prelude::*;

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(required, min_length = 3, max_length = 150)]
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

**Included in source:** workspace; macros for `Uuid`, `String`, `i64`, `bool` and `Option` of those types except a nullable `id`; typed create and update DTOs; `PatchField` for missing, null and value; trimmed string validation; Boolean SQL defaults; generated list, read-by-UUID, create, patch and delete; parameterized SQL; SeaORM PostgreSQL; explicit dev schema bootstrap; Docker Compose database; example app and CLI doctor.

**Not included:** real schema-diff migrations; relationships; admin UI; auth; RBAC; OpenAPI; pagination beyond an explicit 100-row limit with `truncated` indication; sorting/filter UI; optimistic version columns; PostgreSQL integration tests in default CI (optional local test is provided and was not executed successfully on a machine without PostgreSQL); Dockerfile for the API; multi-database support. Do not assume otherwise.

## Development checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

## Ausführliche Entwicklerdokumentation (Deutsch)

**[Dokumentationsportal: docs/index.md](docs/index.md)** – Architektur, Modulgrenzen, Datenflüsse, Codegenerierung, Datenbank, API, CLI, Konfiguration, Validierung, Sicherheit, Tests und Beispielanwendung. Für geplante Module wie Migrationen, Authentifizierung, Admin, OpenAPI, Plugins und Deployment gibt es jeweils ein eigenes technisches Entwurfsdokument.

Jede Seite trennt **IST (v0.1 implementiert)** von **SOLL (nur geplant)**. Änderungen am Framework müssen auch in der passenden Moduldokumentation nachgetragen werden. Der [Implementierungsleitfaden](docs/implementation-guide.md) definiert dafür einen verbindlichen Änderungs- und Testprozess.

Siehe außerdem [Architektur](docs/architecture.md), [Roadmap](docs/roadmap.md) und [Tests](docs/testing.md).

## API list response

GET requests return `{ "data": [...], "limit": 100, "truncated": false }`.
`truncated` is `true` when the table contains more than 100 records. This is **not pagination**.

`#[field(required)]` on a String rejects blank/whitespace-only text, and length
constraints count trimmed Unicode characters. Boolean defaults are persisted
in SQL DDL, but `CREATE TABLE IF NOT EXISTS` **does not update old tables**.
If you bootstrapped a table before this change, recreate it **only in disposable
local development databases** to get the new `DEFAULT TRUE` constraint.

## Optional PostgreSQL integration test

Start PostgreSQL with `docker compose up -d db`, then run this command in the
workspace root (Linux/macOS):

```bash
DATABASE_URL=postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum cargo test -p myshop --test postgres_api -- --ignored --nocapture
```

PowerShell:

```powershell
$env:DATABASE_URL="postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum"; cargo test -p myshop --test postgres_api -- --ignored --nocapture
```

The ignored integration test **requires `DATABASE_URL`** (and fails if unset); it **drops its own fixed test table**
`ferrum_v01_integration_products`; use only a disposable development database.
It checks HTTP POST/GET, UUID/values, failed validation, the optional Boolean
default, and the 100-record truncation indicator. Normal CI does not run it.

`ferrum doctor` currently prints setup hints only; it is **not** a diagnostic.
`.env.example` is documentation and is not loaded automatically.

### Update-Hinweis für bestehende Cursor-Projekte

Vor dem Überschreiben lokale Änderungen sichern, insbesondere `crates/ferrum-api/src/lib.rs` und `Cargo.lock`. Dieses Archiv enthält kein `Cargo.lock`; die vorhandene lokale Datei beibehalten. `target/` nicht ersetzen. Danach `cargo fmt --all`, Clippy und die Tests ausführen.
