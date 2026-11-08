# Implementation guide for executing coding AIs

[For an overview](index.md) · **Status:** Working conventions and testable target criteria. Function status v0.27.1 complements the application worker in [workers.md](workers.md)]. Functional status v0.27 complements [jobs.md](jobs.md), [workers.md](workers.md) and [job-mail-integration.md](job-mail-integration.md)]. Reset tokens are not placed in job payloads. Functional status v0.26 complements [models.md](models.md), [model-components.md](model-components.md), [model-timestamps.md](model-timestamps.md) and [app-lifecycle.md](app-lifecycle.md)]. Soft Delete cannot be described as finished. Functional status v0.25 complements [apps.md](apps.md), [app-registry.md](app-registry.md) and [app-migrations.md](app-migrations.md)]. An existing `main.rs` and applied migrations are not rewritten for this. Functional status v0.24 complements [settings.md](settings.md), [urls.md](urls.md) and [views.md](views.md)]. An existing `main.rs` is not rewritten for this. Function status v0.23 remains valid. Email and reset: [mail.md](mail.md), [password-reset.md](password-reset.md), [notifications.md](notifications.md). Functional status v0.22 remains valid]. Admin extensions: [admin-extension-api.md](admin-extension-api.md). Diagnosis and upgrade: [migration-diagnostics.md](migration-diagnostics.md)., [project-upgrade.md](project-upgrade.md). v0.20 remains valid.] The date and time are described in [date-time-types.md](date-time-types.md)]. Transactions: [transactions.md](transactions.md)]. Existing applied migrations are not rewritten.

## 1. First understand the existing code

Lies in dieser Reihenfolge `Cargo.toml`, [architecture.md](architecture.md), [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [auth.md](auth.md), [csrf.md](csrf.md), [query-scopes.md](query-scopes.md), [i18n.md](i18n.md), [error-codes.md](error-codes.md), [testing.md](testing.md), bevor du die betroffene Crate änderst. **Behaupte keine Feature-Vollständigkeit auf Basis einer SOLL-Dokumentation.** Auth-Tabellen laufen über `apply_directory_with_history` und `_ferrum_auth_migrations`. `0001_initial.json` wird nicht geändert. v0.12 ergänzt Formulare, Staticfiles, lokalen Speicher und Middleware. Upload-Metadaten bleiben Sidecar-JSON, damit die Migrationsprüfsummen gelten. S3 und ein Virenscanner sind nicht als fertig zu beschreiben.

## 2. Respect for module boundaries

- Core: only joint contracts, no ORM/Axum dependency.
- Macros: syn-parsing, typecheck, codegen; no DB connection.
- DB: Connection, typsicheres Repository, DB-Fehler; keine HTTP-Statuscodes.
- API: Axum router, DTO extractor, HTTP response, no new model syntax.
- CLI: Commands and orchestration, not double ORM logic.
- Example: only public ferrioxide facade, no special internal code.

## 3. Mandatory expiry per change

1. Name specific target and affected paths.
2. Check public API/data model for breaking changes.
3. Choose the smallest correct implementation.
4. Supplement tests for positive, negative and safety cases.
5. `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `cargo build --workspace`. The SQLite HTTP test is part of `cargo test`. Check PostgreSQL only with running server and `--ignored`.
6. Document each executed command with result; Name missing tools or DB openly.
7. Update affected documentation pages in IST/SOLL and check cross-links.

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

## 6. Decision Papers (ADR)

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

Proposed ADR topics: Macro ↔ ORM strategy; schema format; access policy; Admin Scheme API; Migration and Rollback Semantics. SOLL means **suggested** here – do not require existing ADRs.

## 7. Next admin phases

Read [roadmap.md](roadmap.md)] before expanding the admin. v0.14 to v0.18 are implemented. The date and time are deliberately not supplemented.

Impeccable, a Cloudflare audit skill, sub-mode and ponytail are not stored in the work tree as a skill. Don't install them on the side. Read a skill only after express consent and only for the phase that calls it. No skill or hook generates commits or pushes.

An admin order that only extends the roadmap does not change traders and templates other than documentation.

## 8. Current focus

SQLite ist die lokale Standarddatenbank. PostgreSQL bleibt über `DATABASE_URL` unterstützt. Neue Spaltenabbildungen gehören nach `crates/ferrioxid-db/src/sql.rs`. Neue Migrationsregeln gehören nach `crates/ferrioxid-migrations`. Ein SQLite-Umbau darf nur laufen, wenn die geänderte Spalte vorher geprüft ist und die Transaktion bei Fehler alles zurücknimmt. Foreign Keys auf bestehenden Spalten werden nicht umgebaut. Authentifizierung ist vorhanden. Der Admin ist in v0.8 vorhanden und seit v0.13 serverseitiges HTML, beschrieben in [admin-native.md](admin-native.md). v0.9 ergänzt `can_create`, das Feature `admin-ui` und die Befehle `new`, `dev`, `doctor`, `check`, `generate model` und `build`. v0.10 ergänzt Settings und Tera-Seiten. Die Cargo-Version bleibt `0.1.0`. Many-to-Many und Eager Loading nur nach ausdrücklicher Beauftragung. Die Roadmap ist keine Freigabe. Constraints und Beziehungen sind in [constraints.md](constraints.md) und [relations.md](relations.md) beschrieben.
