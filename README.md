# Ferrioxid — Model-first Rust backend (experimental)

Ferrioxid is a pre-release Rust full-stack web framework with a Django-inspired shape: explicit models, migrations, authentication, a server-rendered admin, and application modules on Axum and SeaORM. The framework name is Ferrioxid. Crate names stay `ferrioxid`, `ferrioxid-api`, `ferrioxid-cli`, and the other workspace members. The Cargo version is `0.1.0`. This is not a stable public release, and `ferrioxid` is not an installable crate.

SQLite is the default database. PostgreSQL is optional. React, Vite, and Node.js are not required for the admin.

- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security: [SECURITY.md](SECURITY.md)
- Governance: [GOVERNANCE.md](GOVERNANCE.md)
- Conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- GitHub settings that are not stored in Git: [docs/github-maintenance.md](docs/github-maintenance.md)
- Product documentation, currently German: [docs/index.md](docs/index.md)

`LICENSE-MIT` and the workspace `Cargo.toml` currently identify the code as MIT. A dual `MIT OR Apache-2.0` license has not been adopted.

The following paragraph is the German milestone record.

Functional status v0.27.1 starts application jobs via `WorkerHost` and `src/bin/worker.rs`. `ferrioxid jobs inspect` hides the payload until `--show-payload` is set. Description: `docs/workers.md` and `docs/job-security.md`. Functional status v0.27 complements persistent background jobs without Redis. Description: `docs/jobs.md` and `docs/workers.md`. Functional status v0.26 complements reusable model fields and checks installed apps before launch. Description: `docs/models.md`, `docs/model-components.md`, `docs/model-timestamps.md` and `docs/app-lifecycle.md`. Soft Delete is not included. Functional status v0.25 supplements typed apps via `App::install`. Description: `docs/apps.md`, `docs/app-registry.md`, `docs/app-migrations.md`, `docs/reusable-apps.md` and `docs/create-app.md`. An existing `main.rs` does not have to be rewritten for this. `ferrum.toml` does not load an app crate. Functional status v0.24 complements typed settings, named Axum routes and view helpers. The description is given in `docs/settings.md`, `docs/urls.md` and `docs/views.md`. Existing `main.rs` files do not have to be rewritten for this. Function status v0.23 remains valid. Email and password reset describes `docs/mail.md` and `docs/password-reset.md`. The public reset is under `/auth/password-reset` and does not require a staff. Without an active mail backend, no e-mail is sent. Function status v0.22 remains valid. Own admin pages, actions, filters, widgets and templates register `AdminRegistry` in `ferrioxid::admin`. The description is in `docs/admin-extension-api.md`. `ferrioxid migrate status`, `check`, `doctor` and `dev` show pending authentic and app migrations without applying them. `ferrioxid upgrade` creates at most one missing legacy manifesto and does not overwrite a project file. Details: `docs/migration-diagnostics.md` and `docs/project-upgrade.md`. v0.20 remains valid. The cargo version remains `0.1.0`. Directory changes and their audit are a transaction. A `admin_inline` with `|form` stores parent objects and children together. Details: `docs/transactions.md`. The admin is server-side HTML with Axum and Tera. React, Vite and Node.js are not necessary for this. The description is in `docs/admin-native.md`. The area follows `DESIGN.md` and `docs/admin-design-system.md`. Forms, static files, local upload storage and request middleware remain out of v0.12. The admin speaks German and English. `GET /admin/api/config` provides title, color, languages and currency from `ferrum.toml`. Error responses carry a stable `code`. Settings are in `ferrum.toml`, described in `docs/settings.md`. The developer documentation is German. The admin is under `/admin`. `FERRUM_CSRF_SECRET` is in production 64 hex characters.

A new project outside of this workspace:

```powershell
cargo run -p ferrioxid-cli -- new demo --template fullstack --ferrioxid-path .
cd demo
cargo run --manifest-path ..\Cargo.toml -p ferrioxid-cli -- migrate up
cargo run --manifest-path ..\Cargo.toml -p ferrioxid-cli -- admin create-user --email admin@example.com --staff --superuser
cargo run --manifest-path ..\Cargo.toml -p ferrioxid-cli -- dev
```

After `cargo install --path crates/ferrioxid-cli`, the same steps are called `ferrioxid migrate up`, `ferrioxid admin create-user` and `ferrioxid dev`. The password is requested. `FERRUM_PASSWORD` is for automation only and has no default value.

Authentication, sessions and roles describes `docs/auth.md`. Protected myshop resource is `/api/v1/internal_notes`. Products and categories remain expressly public demo routes.

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
cargo run -p ferrioxid-cli -- migrate make initial
cargo run -p ferrioxid-cli -- migrate up
cargo run -p ferrioxid-cli -- migrate status
cargo run -p ferrioxid-cli -- migrate rename-column --table products --from name --to title
cargo run -p ferrioxid-cli -- migrate baseline --confirm
cargo run -p ferrioxid-cli -- admin create-user --email admin@example.com --superuser
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

`myshop --bootstrap` does not create or migrate a schema. Use `ferrioxid migrate up` before serving.

### Project files

- `examples/myshop/src/models/product.rs` — developer-facing model
- `crates/ferrioxid-macros/src/lib.rs` — Rust derive macro
- `crates/ferrioxid-core/src/lib.rs` — descriptors, errors and core trait
- `crates/ferrioxid-db/src/lib.rs` — repository contract and database connection
- `crates/ferrioxid-db/src/sql.rs` — SeaQuery DDL and CRUD for SQLite and PostgreSQL
- `crates/ferrioxid-api/src/lib.rs` — route registration, handler adapters and bootstrap
- `crates/ferrioxid/src/lib.rs` — public framework API
- `crates/ferrioxid-cli/src/main.rs` — experimental informational `ferrioxid doctor` CLI
- `crates/ferrioxid-admin/` — Admin API, HTML interface, templates and CSS

## Model example

```rust
use ferrioxid::prelude::*;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(table = "products", api = "/api/v1/products", allow_anonymous = true)]
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
with Ferrioxid problem JSON. `DELETE /{id}` returns `204` with an empty body when
a row was removed, and `404` when it was already gone. `{id}` must be a UUID;
Axum rejects other path values with its own response. Routes inherit
`allow_anonymous` and return `403` when it is false.

`PATCH` with `{}` returns the current row unchanged. A missing JSON key keeps
the stored value. JSON `null` on a non-null column is `422`. JSON `null` on an
`Option<T>` column stores SQL `NULL`. String rules use the trimmed length and
store the original text.

## Important security notice

Ferrioxid `0.1.0` is pre-release. Do not expose a demo to the public internet. Models deny access unless `allow_anonymous = true`, and that flag is only appropriate for a local demo. Authentication, sessions, CSRF for cookie-authenticated writes, and the server-rendered admin exist and are described in `docs/auth.md` and `docs/admin.md`. The example binds to `127.0.0.1` by default. The sample PostgreSQL password is for local development only. Report vulnerabilities through [SECURITY.md](SECURITY.md), not through a public issue.

## Scope and limitations

**Included in source:** workspace; macros for `Uuid`, `String`, `i64`, `bool` and `Option` of those types except a nullable `id`; typed create and update DTOs; `PatchField`; trimmed string validation; Boolean defaults rendered per dialect; generated list, read-by-UUID, create, patch and delete; single-column and composite unique constraints; secondary indexes; single-column foreign keys with `restrict`, `cascade` and `set_null`; lazy belongs-to and has-many methods; SeaQuery SQL for SQLite and PostgreSQL; versioned migrations for those objects, explicit renames, checked SQLite rebuilds, explicit destructive drops, and a confirm-only SQLite baseline; SQLite migration, constraint and HTTP tests in `cargo test --workspace`; optional Docker Compose PostgreSQL; the myshop category/product example and CLI.

**Not included:** many-to-many, eager or batch loading, changing a foreign key on an existing column, heuristic renames, `migrate down`, PostgreSQL baseline, admin UI, auth, RBAC, OpenAPI, pagination beyond 100 rows, optimistic version columns, `DateTime` or JSON columns, MySQL, and a Dockerfile for the API. The ignored PostgreSQL tests are not part of the default run. Do not assume otherwise.

## Development checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Contributor workflow, branch names, and migration rules are in [CONTRIBUTING.md](CONTRIBUTING.md). A warm local cache may add `--offline`. GitHub Actions must be allowed to download dependencies.

## Detailed developer documentation (German)

**[Documentation portal: docs/index.md](docs/index.md)** - Architecture, module boundaries, data flows, code generation, database, API, CLI, configuration, validation, security, testing and example application.] Constraints are in [docs/constraints.md](docs/constraints.md), relationships in [docs/relations.md](docs/relations.md), migrations in [docs/migrations.md](docs/migrations.md)]. Authentication, admin, OpenAPI, plugins and deployment remain drafts, as far as the respective page indicates this.

Each page separates **IST** from **SOLL (planned only)**. The functional status of these constraints and relationships is v0.6. The cargo version remains `0.1.0`. Changes to the framework must also be added to the appropriate module documentation. The [Implementation Guide ](docs/implementation-guide.md)] defines a binding change and test process.

See also [Architecture](docs/architecture.md), [Roadmap](docs/roadmap.md) and [Tests](docs/testing.md)].

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
| SQLite memory | `sqlite::memory:` | tests; Ferrioxid uses one pooled connection |
| PostgreSQL | `DATABASE_URL=postgres://...` | supported server backend |

Optional PostgreSQL, after a server is actually running:

```powershell
docker compose up -d db
$env:DATABASE_URL="postgres://ferrioxid:ferrioxid_dev_only@127.0.0.1:5432/ferrioxid"
cargo test -p myshop --test postgres_api -- --ignored --nocapture
```

The ignored test **requires `DATABASE_URL`** and drops its fixed test tables. Use only a disposable database. `cargo test --workspace` already runs the same HTTP flow against SQLite memory and does not need this step.

`ferrioxid doctor` prints setup hints only. `.env.example` is not loaded automatically.

`FerrioxidModel::schema_sql` now takes `DatabaseKind` and returns `String`. Callers of the old `schema_sql() -> &'static str` must pass the backend.
