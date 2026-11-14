# Fexu Roadmap

[Documentation](index.md) · [Django capability matrix](django-capability-matrix.md) · [Admin comparison](admin-django-parity.md) · [GitHub issues](https://github.com/Iam-Sanja/fexu-rs/issues)

**Baseline:** Fexu source at `a39c952` (2026-11-14). **Cargo package version: `0.1.0`**, experimental. Individual workspace crates are being published to crates.io in dependency order; a partially published set is not yet an installable standalone framework. Check [release readiness](release-readiness.md) and the [crates.io package](https://crates.io/crates/fexu) for current availability. Publication does not imply production readiness.

## Release versions versus historical milestones

Older documents and the [changelog](../CHANGELOG.md) use internal feature milestones such as `v0.13` and `v0.28`. **Those labels are not crates.io releases or Cargo semver versions.** They record a development sequence. The first planned crates.io package version is `0.1.0`. This page tracks the actual current design and remaining work instead of presenting a second version numbering system.

## Available in the source baseline

| Area | Present | Boundaries |
|---|---|---|
| Models and queries | `FexuModel`, typed model methods, explicit registration, filters, keyset pages, aggregate annotations, batch relations including explicit many-to-many loading | Not a general replacement for Django QuerySets or arbitrary SQL |
| Persistence | SeaQuery-generated SQL; bundled rusqlite 0.42 for SQLite, sqlx-postgres/sqlx-core 0.8.6 for PostgreSQL | SQLite and PostgreSQL only; do not mix `sqlx-sqlite` into the dependency graph |
| Migrations | Versioned app/framework migrations, dependency checks, `migrate up`, guarded `migrate down`, explicit backfills and SQLite rebuild protections | Never rewrite recorded migration checksums or migrate a production DB without backup |
| API and routing | Axum HTTP, generated model CRUD, model-derived OpenAPI 3.1, cursor-based lists, named routes, in-process TestClient | Custom routes require explicit documentation; API access requires policy configuration |
| Auth and security | Argon2id, sessions, permissions, cookie CSRF, MFA support in framework, password reset, deployment checks | No claim of universal production hardening; test application flows separately |
| Native admin | Axum + Tera, server-rendered CRUD, directory, filters, fieldsets, actions, audit, navigation and extension points | No React/Vite/Node dependency for basic admin; no promise of full Django parity |
| Services | Forms, templates, i18n, local storage, email, cache, jobs and worker host | Remote storage and mail delivery require external integration and deployment review |
| Developer tooling | CLI, project/app generators, migration diagnostics, SQLite tests and PostgreSQL CI | CLI can only load code compiled into its own process; a separate project binary hosts app handlers |

Details and limits are in the [capability matrix](django-capability-matrix.md), [database driver](database-driver.md), [API documentation](api.md), [auth](auth.md) and [admin guide](admin.md). This is a feature inventory, **not** a blanket assurance that every capability is complete.

## Next priorities

1. **Finish and independently validate the first registry release.** Release all 16 crates in the documented dependency order. After publishing, test `cargo add fexu` and `cargo install fexu-cli` in a clean external project with no path dependency. Do not report these commands as tested before their packages are indexed.
2. **Resolve parallel PostgreSQL test fixture contention** ([#117](https://github.com/Iam-Sanja/fexu-rs/issues/117)). Isolated database creation can collide via `template1` when tests run concurrently. Serial fixture execution works; concurrency needs a regression test on PostgreSQL 17 and 18.
3. **Keep the public documentation aligned with the shipped code** ([#119](https://github.com/Iam-Sanja/fexu-rs/issues/119)). Verify install instructions, the current DB driver, CLI command behavior and compatibility notes.
4. **Extend deployment and integration coverage.** The TestShop passed 24 SQLite tests, 24 serial PostgreSQL tests and a separate PostgreSQL 17 Docker HTTP acceptance against `a39c952` ([#118](https://github.com/Iam-Sanja/fexu-rs/issues/118)). That TestShop does not cover MFA or `Prefer` TLS negotiation against an SSL-enabled PostgreSQL server.
5. **Improve framework ergonomics and capability parity** according to individually scoped issues and the [Django capability matrix](django-capability-matrix.md). Do not promise unimplemented features or dates.

## Long-term direction

Fexu should offer model-first Rust development with an integrated, accessible, server-rendered admin. Core admin tasks must work without JavaScript, while optional client scripting adds convenience. Focus on correctness, documented limits, security tests, PostgreSQL portability and maintainable public APIs before declaring a stable 1.0 release.

Further possible work includes deeper Django-admin workflows, remote object-storage integrations, broader automated browser coverage, operational observability and scheduling improvements. These are **proposals**, not current release guarantees.

## Historical development records

- [CHANGELOG](../CHANGELOG.md) preserves the original internal implementation milestones.
- [Admin UX backlog](admin-ux-backlog.md) captures detailed UX decisions and historical implementation notes.
- [Framework quality review](framework-quality-review.md) documents an older branch snapshot, not the current release state.
- [Database driver](database-driver.md) explains the PR #115 execution-layer migration.
- [Release readiness](release-readiness.md) records publication ordering and pre-release checks.

Historical branch names, screenshots, test outputs and issue links should remain dated rather than silently rewritten as if they described today's `main`.
