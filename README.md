# Ferrioxid — Model-first Rust backend (experimental)

Ferrioxid is a pre-release Rust full-stack web framework with a Django-inspired shape: explicit models, migrations, authentication, a server-rendered admin, and application modules on Axum and SeaORM. The framework name is Ferrioxid. Crate names stay `ferrioxid`, `ferrioxid-api`, `ferrioxid-cli`, and the other workspace members. The Cargo version is `0.1.0`. This is not a stable public release, and `ferrioxid` is not an installable crate.

**Breaking configuration and schema change:** use `ferrioxid.toml`, `.ferrioxid/`, `FERRIOXID_*` and `_ferrioxid_*` tables with a fresh database. Old names are not aliases. See [the legacy-name removal record](docs/ferrum-legacy-removal.md) for the complete mapping and compatibility limits.

SQLite is the default database. PostgreSQL is optional. React, Vite, and Node.js are not required for the admin.

- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security: [SECURITY.md](SECURITY.md)
- Governance: [GOVERNANCE.md](GOVERNANCE.md)
- Conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- GitHub settings that are not stored in Git: [docs/github-maintenance.md](docs/github-maintenance.md)
- Developer documentation: [docs/index.md](docs/index.md)

`LICENSE-MIT` and the workspace `Cargo.toml` currently identify the code as MIT. A dual `MIT OR Apache-2.0` license has not been adopted.

The milestone numbers below describe implemented features; the Cargo package version remains `0.1.0`.

Milestone v0.28 groups the native admin by app. v0.27.1 runs application jobs through `WorkerHost` and `src/bin/worker.rs`; `ferrioxid jobs inspect` hides payloads unless `--show-payload` is supplied. v0.27 adds persistent jobs without Redis. v0.26 adds reusable model fields and validates installed apps before startup; soft delete is not implemented. v0.25 adds typed apps through `App::install` without loading Rust crates from `ferrioxid.toml`. v0.24 adds typed settings, named Axum routes and view helpers. v0.23 adds email and public password reset at `/auth/password-reset`; mail remains disabled by default. v0.22 adds custom admin pages, actions, filters, widgets and templates through `AdminRegistry` in `ferrioxid::admin`. Migration diagnostics report pending changes without applying them, and `ferrioxid upgrade` creates only a missing legacy manifest. Model and directory changes share transactions with their audits; `admin_inline` with `|form` saves parent and child records together. The admin uses Axum and Tera, supports German and English, and exposes branding through `GET /admin/api/config`. Forms, static files, local uploads and middleware remain available. Runtime settings stay in `ferrioxid.toml`; production `FERRIOXID_CSRF_SECRET` values contain 64 hex characters. See the [documentation portal](docs/index.md) for each feature and its limits.

Create a standalone project outside the framework workspace. Start in the Ferrioxid repository; the CLI is installed from this checkout because the crates are not published:

```powershell
cargo install --path crates/ferrioxid-cli
$ferrioxidRepo = (Get-Location).Path
Set-Location ..
ferrioxid new project demo --template fullstack --ferrioxid-path $ferrioxidRepo
Set-Location demo
ferrioxid migrate up
ferrioxid admin create-user --email admin@example.com --staff --superuser
ferrioxid dev
```

The generated project has no business models or product tables. Add an app with `ferrioxid new app produkte`. See the [project/app guide](docs/project-generator.md).

The generated project is a sibling of the framework repository, so Cargo does not treat it as an unlisted workspace member. The CLI prompts for a password. `FERRIOXID_PASSWORD` is an optional automation input with no default value.

[Authentication, sessions and roles](docs/auth.md) protect `/api/v1/internal_notes` in myshop. Products and categories are explicitly public demo routes.

> This is **not** a complete Django replacement. The current milestone covers a Rust derive macro, shared model metadata, one SeaORM/SeaQuery repository for SQLite and PostgreSQL, and generated list, detail, create, patch and delete routes.

SQLite is the default local database. Docker and PostgreSQL are optional.

## Quick start

Prerequisites: current stable Rust and Cargo. No database server is required.

```powershell
cargo run -p ferrioxid-cli -- migrate up
cargo run -p myshop
```

Unset `DATABASE_URL` uses `sqlite://db.sqlite3?mode=rwc` in the process working directory. The file is created on first connect and is gitignored.

The following commands are a migration-command reference, not a setup script. On a fresh checkout, use the quick start above: `migrate up` is sufficient. Create, rename or baseline migrations only when preparing that specific schema change. The user-creation command below demonstrates model privileges; add `--staff` when the account must also open the admin.

```powershell
cargo run -p ferrioxid-cli -- migrate make initial
cargo run -p ferrioxid-cli -- migrate up
cargo run -p ferrioxid-cli -- migrate status
cargo run -p ferrioxid-cli -- migrate rename-column --table products --from name --to title
cargo run -p ferrioxid-cli -- migrate baseline --confirm
cargo run -p ferrioxid-cli -- admin create-user --email admin@example.com --superuser
cargo run -p myshop
```

`myshop` already contains `examples/myshop/migrations/0001_initial.json`. On a new SQLite file, `migrate up` is enough. `ferrioxid.toml` points the CLI at the myshop model registry. The CLI does not parse Rust source. `--bootstrap` is legacy: it no longer creates tables, does not write migration history, and does not delete existing files.

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
- `crates/ferrioxid-cli/src/main.rs` — project generation, diagnostics, migrations and developer commands
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

Ferrioxid `0.1.0` is pre-release. Do not expose a demo to the public internet. Models are private by default; protected APIs require configured authentication and permissions. Use `allow_anonymous = true` only for intentional public routes such as local demos. Authentication, sessions, CSRF protection for cookie-authenticated writes and the server-rendered admin are described in [docs/auth.md](docs/auth.md) and [docs/admin.md](docs/admin.md). The example binds to `127.0.0.1` by default. Sample PostgreSQL credentials are for local development only. Report vulnerabilities through [SECURITY.md](SECURITY.md).

## Scope and limitations

**Included in source:** typed models and create/update DTOs; `Uuid`, `String`, `i64`, `bool`, `NaiveDate`, `NaiveTime` and `DateTime<Utc>` fields with supported optional forms; `PatchField`; validation; SQLite and PostgreSQL repositories; generated list, detail, create, patch and delete endpoints; unique constraints, indexes and single-column foreign keys; lazy belongs-to and has-many relations; versioned migrations, explicit renames, checked SQLite rebuilds, destructive-change confirmation and SQLite baseline; authentication, sessions, role permissions, object policies and password reset; the native admin; settings, named routes, views and templates; forms, static files, local storage, middleware, email, jobs, workers and typed apps. See individual documentation pages for limits and test coverage.

**Not included:** many-to-many, eager or batch loading, changing foreign keys on existing columns, inferred renames, `migrate down`, PostgreSQL baseline, OpenAPI generation, cursor/offset pagination for generated public REST lists, optimistic version columns, JSON model columns, MySQL, S3, virus scanning, soft delete or an API Dockerfile. The admin has its own paginated lists. PostgreSQL tests are ignored in the default local test run and require a disposable server when run explicitly.

## Development checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Contributor workflow, branch names, and migration rules are in [CONTRIBUTING.md](CONTRIBUTING.md). A warm local cache may add `--offline`. GitHub Actions must be allowed to download dependencies.

## Developer documentation

**[Documentation portal: docs/index.md](docs/index.md)** — Architecture, models, database, APIs, CLI, settings, validation, security, tests and examples. [Constraints](docs/constraints.md), [relations](docs/relations.md) and [migrations](docs/migrations.md) describe the implemented data layer. Authentication and the native admin are implemented. OpenAPI, general plugin hooks and a production app container remain planned where their pages say so.

Each page distinguishes **Implemented** behavior from **Planned** work. Feature milestones are separate from the Cargo version `0.1.0`. Update the relevant module documentation whenever framework behavior changes. The [implementation guide](docs/implementation-guide.md) defines the change and validation workflow.

See also [Architecture](docs/architecture.md), [Roadmap](docs/roadmap.md) and [Tests](docs/testing.md).

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

`ferrioxid doctor` checks tools, configuration, project files and migration status without applying migrations. `.env.example` is documentation and is not loaded automatically.

`FerrioxidModel::schema_sql` now takes `DatabaseKind` and returns `String`. Callers of the old `schema_sql() -> &'static str` must pass the backend.
