# Fexu

**Build model-driven web applications in Rust.**

A Django-inspired, experimental web framework built on **Axum**, **SeaORM**, and **Tera**. Define typed models, manage migrations, use SQLite or PostgreSQL, and work with a server-rendered admin — without requiring React, Vite, or Node.js for the admin.

[![Rust CI](https://github.com/Iam-Sanja/fexu-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Iam-Sanja/fexu-rs/actions/workflows/ci.yml)
[![Security audit](https://github.com/Iam-Sanja/fexu-rs/actions/workflows/security.yml/badge.svg)](https://github.com/Iam-Sanja/fexu-rs/actions/workflows/security.yml)
![Rust](https://img.shields.io/badge/language-Rust-CE422B?logo=rust&logoColor=white)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)
![Status: experimental](https://img.shields.io/badge/status-experimental-orange)

[**Quick start**](#quick-start) · [**Features**](#what-can-fexu-do) · [**Docs**](docs/index.md) · [**Contribute**](#help-build-fexu) · [**Roadmap**](docs/roadmap.md)

> [!IMPORTANT]
> **Fexu is under active development, not a stable release.** APIs, configuration and migrations may change. The workspace currently uses Cargo version `0.1.0`; its crates are **not published on crates.io**. Install the CLI from a source checkout. Do not expose a demo application directly to the public internet.

## Why Fexu?

Rust brings strong typing, performance and explicit control. Fexu aims to bring familiar framework ergonomics to those strengths: one place for models and validation, consistent database access, built-in application features, and a native admin.

| | Feature | What is available |
|---|---|---|
| 🧱 | **Typed models** | `#[derive(FexuModel)]`, validation and generated CRUD endpoints |
| 🗃️ | **Database layer** | SQLite by default, PostgreSQL, composable model queries and batched relation reads |
| 🔄 | **Migrations** | Versioned schema changes, dependency checks and guarded SQLite table rebuilds |
| 🔐 | **Authentication** | User accounts, permissions, server-side sessions and cookie-CSRF protection |
| 🛠️ | **Native admin** | Server-rendered admin for models, with app grouping, actions and filters |
| 🧩 | **App building blocks** | Forms, templates, named routes, fixtures, custom commands, mail and persistent jobs |

These features have **documented limitations**. In particular, public model routes must be configured deliberately, and CSRF protection depends on the request and route policy. See [security](docs/security.md), [capability matrix](docs/django-capability-matrix.md), and [current limitations](#current-status).

## Quick start

**Requirements:** stable Rust and Cargo. For the default SQLite setup, you do not need to install a database server.

Clone the repository and try the included MyShop example:

```bash
git clone https://github.com/Iam-Sanja/fexu-rs.git
cd fexu-rs
cargo run -p fexu-cli -- migrate up
cargo run -p myshop
```

Then, in another terminal:

```bash
curl http://127.0.0.1:3000/healthz
curl http://127.0.0.1:3000/api/v1/products
```

SQLite defaults to `sqlite://db.sqlite3?mode=rwc` in the current working directory when `DATABASE_URL` is unset. The example uses the migration directory configured in `fexu.toml`.

### Create your own application

From the Fexu checkout, install the **local** CLI and generate a standalone project alongside the repository:

```powershell
cargo install --path crates/fexu-cli
$fexuRepo = (Get-Location).Path
Set-Location ..
fexu new project demo --template fullstack --fexu-path $fexuRepo
Set-Location demo
fexu migrate up
fexu admin create-user --email admin@example.com --staff --superuser
fexu dev
```

The CLI prompts for the admin password; there is **no default password**. New projects do not include shop-specific business models. See [project generation](docs/project-generator.md) and [CLI reference](docs/cli.md).

> [!NOTE]
> Independent projects do not add a `[patch.crates-io]` entry. SQLite is rusqlite inside Fexu. Published `sqlx-sqlite` cannot be added beside this graph, because both bindings set `links = "sqlite3"`. See [SQLite consumer instructions](docs/security/sqlite-rustsec-2026-0459.md).

## A model in Rust

```rust
use fexu::prelude::*;

#[derive(FexuModel, fexu::serde::Serialize, fexu::serde::Deserialize)]
#[fexu(table = "products", api = "/api/v1/products", allow_anonymous = true)]
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

Register the model explicitly with `App::new(db).register::<Product>()`. Fexu then mounts list, detail, create, patch and delete routes for its API path. **`allow_anonymous = true` is only for intentionally public routes**; protected business data should use authentication, permissions and object policies.

Learn more: [models](docs/models.md) · [database](docs/database.md) · [relations](docs/relations.md) · [migrations](docs/migrations.md) · [admin](docs/admin.md).

## Native admin

Fexu includes a **server-rendered admin**, built with Axum and Tera. No separate JavaScript frontend build is needed for basic administration.

Admin screenshots and a visual walkthrough will be added as the project matures. Until then, the [admin documentation](docs/admin.md) describes the supported workflows and restrictions.

## Current status

Fexu is a **work in progress**, not a drop-in Django replacement.

`main` includes model-derived OpenAPI, keyset pagination, many-to-many loading, `migrate down`, and the SQLite/PostgreSQL driver without a vendor patch. The [Django capability matrix](docs/django-capability-matrix.md) and [roadmap](docs/roadmap.md) remain the map of what is still missing. The experimental framework has **not** been declared production-ready, and version `0.1.0` has not been uploaded to crates.io.

## Help build Fexu

**You don't need to know the whole framework to contribute.** Useful contributions include reproducible bug reports, smaller documentation fixes, tests, examples and focused feature improvements.

- 🐛 [Report a bug](https://github.com/Iam-Sanja/fexu-rs/issues/new/choose) — include a minimal reproduction and your Rust/OS/database versions.
- 💡 [Suggest an improvement](https://github.com/Iam-Sanja/fexu-rs/issues/new/choose) — explain the problem and the developer experience you want.
- 💬 [Ask a question or share an idea](https://github.com/Iam-Sanja/fexu-rs/discussions) — discussions are welcome before you write code.
- 🌱 [Find a first contribution](https://github.com/Iam-Sanja/fexu-rs/labels/good%20first%20issue) — or read the [first contribution guide](docs/first-contribution.md).
- 🔧 [Open a pull request](CONTRIBUTING.md) — small, well-tested changes are welcome.

Please use [private security reporting guidance](SECURITY.md) for vulnerabilities rather than opening a public issue. All participation follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Documentation and development

Start with the [documentation index](docs/index.md). Other useful entry points: [architecture](docs/architecture.md), [configuration](docs/configuration.md), [security](docs/security.md), [testing](docs/testing.md) and [implementation guide](docs/implementation-guide.md).

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

PostgreSQL runtime tests require a **disposable** PostgreSQL database and an explicit `DATABASE_URL`; the default workspace test command does not run ignored PostgreSQL tests. Never run destructive test migrations against real application data.

Fexu is currently licensed under [MIT](LICENSE-MIT). See [governance](GOVERNANCE.md) and [contribution guidelines](CONTRIBUTING.md).

---

**Interested in Rust web frameworks?** Try the example, tell us what was confusing, and open an issue or discussion. Early feedback is valuable — even if you haven't written a line of Fexu code.
