# Fexu, Runique, Cot and Loco

Checked on 2026-11-12. This is a documentation matrix. It is not a timed implementation of the same application, and it is not a benchmark. No latency, throughput, memory or startup numbers were measured on shared hardware for this note. Do not quote any of those figures from this page.

## Versions and sources

| Framework | Version used for this note | Source checked |
|---|---|---|
| Fexu | Cargo `0.1.0`; initial component crates published 2026-11-14 | This repository, `main` at `a39c952`; [release readiness](release-readiness.md) |
| Runique | crates.io lists `3.0.3` updated 2026-10-10. The project site and docs.rs describe `4.0.0`. | [crates.io/crates/runique](https://crates.io/crates/runique), [runique.io installation](https://runique.io/installation), [docs.rs/crate/runique/latest](https://docs.rs/crate/runique/latest/builds) |
| Cot | `0.8.0`, published 2026-11-09 | [crates.io/crates/cot](https://crates.io/crates/cot), [github.com/cot-rs/cot](https://github.com/cot-rs/cot) |
| Loco | `loco-rs` `1.3.1` and generator `loco` `1.3.1`, published 2026-11-06 | [crates.io/crates/loco-rs](https://crates.io/crates/loco-rs), [github.com/loco-rs/loco/releases](https://github.com/loco-rs/loco/releases) |

The Runique crates.io page and the Runique website did not show the same version on the day of this note. The matrix below uses the website's `4.0.0` feature descriptions and says so. It does not pick a winner from that mismatch.

## What was not done

A four-framework application with the same models, files and admin was not built. Setup-step counts, lines of code and migration timings were not recorded. Performance cells say "not measured".

## Matrix

| Topic | Fexu 0.1.0 | Runique 4.0.0 (site) / 3.0.3 (crates.io) | Cot 0.8.0 | Loco 1.3.1 |
|---|---|---|---|---|
| Developer effort | Model derive, explicit `register`, CLI project generator. Still experimental and path-installed. | `runique new`, one model declaration for table, migration, form and admin. Admin described as beta or preview depending on the page. | Django-like batteries on Axum. `cargo add` from crates.io. MSRV listed as 1.94. | `cargo install loco` and a Rails-like generator. MSRV listed as 1.94. First stable line is 1.0.0. |
| ORM | SeaQuery + rusqlite SQLite + direct SQLx PostgreSQL; typed `Select`, keyset pages, scopes. | SeaORM. Site pins SeaORM `=2.0.4` for 4.0.0. One database feature per build. | SeaQuery and SQLx. SQLite, PostgreSQL and MySQL features. | SeaORM. SQLite and PostgreSQL features. 1.0 moved to SeaORM 2. |
| Migrations | Versioned files, `migrate up` and `migrate down` on main. Not run at process start. | `runique makemigrations` and `runique migration up/down/status`. | Framework migrations through its database feature. | SeaORM migrations. 1.3.1 notes a SQLite pool timeout fix. |
| Admin | Native HTML admin in the binary, session login, tested in this repo. | Generated admin, described by the project as beta or preview. | Not claimed as a Django admin in the crates.io summary. | Not a built-in Django-style admin in the crates.io feature list. |
| Forms | `FexuForm`, model binding, uploads through storage. | `#[form]` in the project README. | Form stack is part of the framework docs, not re-tested here. | Controllers and validation on Axum. Not re-tested here. |
| Auth | Sessions, roles, CSRF, anonymous cookie, Argon2. | Argon2, CSRF and database sessions in the project README. | Auth is not the default feature set on the crates.io feature list checked here. | `auth` feature with session cookies. |
| Tests | Workspace tests on Windows and Linux. Ignored PostgreSQL tests were run locally on PostgreSQL 18.6 against disposable databases. Playwright login is an Ubuntu CI job and was not launched on Windows. | `runique test` in the installation page. CI shape was not re-run. | `test` feature. CI shape was not re-run. | `testing` feature and axum-test. CI shape was not re-run. |
| API | Generated CRUD, OpenAPI 3.1, cursor list, problem JSON. | Not described as a generated OpenAPI CRUD in the pages checked. | `openapi` and `swagger-ui` features. | Controllers on Axum. OpenAPI is not a default feature in the crates.io list checked here. |
| Docker | Root Compose starts PostgreSQL on localhost. `deploy/` adds an app image, Caddy and an unpublished database. That stack was not booted here. | Not part of the pages checked. | Not part of the pages checked. | Not part of the pages checked. |
| Documentation | English manual in `docs/`, including limits. Full `fexu` crate documentation depends on its registry release and docs.rs processing. | English and French on runique.io. | docs.rs and the project changelog. | loco.rs and docs.rs. |
| Performance | Not measured. | Not measured. | Not measured. | Not measured. The 1.3.0 release notes discuss compiler work for a generated app. Those numbers are the project's own, on their hardware, and are not a runtime benchmark against Fexu. |

## Fexu limits that this comparison does not hide

- Fexu remains experimental `0.1.0`; during the staged rollout, component crates may be available before the top-level package.
- Public list filters exist only for admin-filterable fields that the read serializer still returns.
- `FEXU_TRUST_PROXY=1` requires explicit `FEXU_TRUSTED_PROXIES` peer addresses; an empty or wildcard-only list trusts no peer. The trusted-peer safeguard was integrated into `main` (PR #97). See [deployment](deployment.md).
- Playwright covers the login page with a fresh profile. It does not submit a password or walk an authenticated form. That spec was not launched on this Windows machine.
- Docker was not started. Docker is not installed here.
- A local PostgreSQL 18.6 run of the ignored tests, including `pg_dump` and `pg_restore`, passed on disposable `fexu_test_*` databases. That is not a performance measurement.
