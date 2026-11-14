# Implementation guide for coding agents

[Overview](index.md) · **Scope:** working conventions and testable acceptance criteria. Relevant modules include [jobs](jobs.md), [workers](workers.md), [model components](model-components.md), [apps](apps.md), [settings](settings.md), [named routes](urls.md), [views](views.md), [migration diagnostics](migration-diagnostics.md), [project upgrade](project-upgrade.md), [date/time types](date-time-types.md) and [transactions](transactions.md). Reset tokens never belong in job payloads. Soft delete is planned. Framework changes must not silently rewrite existing `main.rs` files or applied migrations.

## 1. First understand the existing code

Read `Cargo.toml`, [architecture.md](architecture.md), [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [auth.md](auth.md), [csrf.md](csrf.md), [query-scopes.md](query-scopes.md), [i18n.md](i18n.md), [error-codes.md](error-codes.md) and [testing.md](testing.md), in that order, before changing the relevant crate. **Do not claim a feature is complete based on a planned-design document.** Auth tables use `apply_directory_with_history` and `_fexu_auth_migrations`. Do not edit `0001_initial.json`. v0.12 adds forms, static files, local storage and middleware. Upload metadata remains in sidecar JSON to preserve migration checksums. Do not describe S3 or virus scanning as implemented.

## 2. Respect for module boundaries

- Core: only joint contracts, no ORM/Axum dependency.
- Macros: syn-parsing, typecheck, codegen; no DB connection.
- DB: connections, type-safe repositories and database errors; no HTTP status codes.
- API: Axum router, DTO extractor, HTTP response, no new model syntax.
- CLI: Commands and orchestration, not double ORM logic.
- Example: only public Fexu facade, no special internal code.

## 3. Required workflow for each change

1. Name specific target and affected paths.
2. Check public API/data model for breaking changes.
3. Choose the smallest correct implementation.
4. Supplement tests for positive, negative and safety cases.
5. `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `cargo build --workspace`. The SQLite HTTP test is part of `cargo test`. Check PostgreSQL only with running server and `--ignored`.
6. Document each executed command with result; Name missing tools or DB openly.
7. Update affected documentation pages in Implemented/Planned and check cross-links.

## 4. Modification template (for issue/PR)

```text
Titel:
Ziel / Motivation:
Betroffene Dateien (relative Pfade):
IST-Verhalten:
SOLL-Verhalten:
Öffentliche API / Breaking Changes:
Sicherheit / DB-Auswirkungen:
Tests inkl. echter Ausführung:
Dokumentationsänderungen:
Bekannte Einschränkungen:
```

## 5. Technical Definition of Done

- No untreated compiler errors, no mute schema drift behavior.
- No unjustified `allow`-Lint or dead pseudocode.
- API/repo contracts are typed; `serde_json::Value` remains **internal** when required.
- Exposed endpoints are not public without explicit approval.
- Negative tests confirm necessary rejection.
- README and partial documentation describe the **actually implemented**.

## 6. Architecture decision records (ADRs)

Create short ADRs under `docs/adr/` for landmark decisions:

```text
ADR-0001: Titel
Status: proposed | accepted | superseded
Kontext:
Entscheidung:
Alternativen:
Folgen:
Migrationsaufwand:
```

Proposed ADR topics: macro/ORM strategy, schema format, access policy, the admin schema API, and migration and rollback semantics. These ADRs are suggestions, not existing requirements.

## 7. Next admin phases

Read [roadmap.md](roadmap.md) before extending the admin. Historical feature milestones through v0.28 are implemented in source, not Cargo releases. Consult the current roadmap and keep proposals separate from available features.

Impeccable, a Cloudflare audit skill, sub-mode and ponytail are not stored in the work tree as a skill. Don't install them on the side. Read a skill only after express consent and only for the phase that calls it. No skill or hook generates commits or pushes.

An admin order that only extends the roadmap does not change handlers and templates other than documentation.

## 8. Current focus

SQLite is the default local database; PostgreSQL remains supported through `DATABASE_URL`. Add column mappings in `crates/fexu-db/src/sql.rs` and migration rules in `crates/fexu-migrations`. A SQLite rebuild must validate affected values before conversion and roll back the entire transaction on error. Existing-column foreign keys are not rebuilt. Authentication exists; the admin was added in v0.8 and has used server-rendered HTML since v0.13, as described in [admin-native.md](admin-native.md). v0.9 adds `can_create`, the `admin-ui` feature and `new`, `dev`, `doctor`, `check`, `generate model` and `build`. v0.10 adds settings and Tera pages. The Cargo version remains `0.1.0`. Implement many-to-many or eager loading only when explicitly requested; the roadmap is not authorization. See [constraints.md](constraints.md) and [relations.md](relations.md).
