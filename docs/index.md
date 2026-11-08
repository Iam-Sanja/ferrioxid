# Ferrioxid – Dokumentationsportal

**Project status:** Function status v0.28 groups the native admin by apps: [admin-navigation.md](admin-navigation.md)]. Functional status v0.27.1 complements application workers and hidden job payloads: [workers.md](workers.md), [job-security.md](job-security.md)]. Functional status v0.27 adds background jobs: [jobs.md](jobs.md), [workers.md](workers.md), [job-retries.md](job-retries.md),] [job-security.md](job-security.md), [job-mail-integration.md](job-mail-integration.md)]. Functional status v0.26 complements model fields: [models.md](models.md), [model-components.md](model-components.md), [model-timestamps.md](model-timestamps.md), [model-soft-delete.md](model-soft-delete.md), [app-lifecycle.md](app-lifecycle.md)]. Functional status v0.25 complements apps: [apps.md](apps.md), [app-registry.md](app-registry.md), [app-migrations.md](app-migrations.md), [reusable-apps.md](reusable-apps.md), [create-app.md](create-app.md)]. Functional status v0.24 adds settings, named routes and views: [settings.md](settings.md), [custom-settings.md](custom-settings.md), [urls.md](urls.md), [routing.md](routing.md), [views.md](views.md), [project-structure.md](project-structure.md). Functional status v0.23 remains valid.] Email: [mail.md](mail.md), [mail-smtp.md](mail-smtp.md), [mail-security.md](mail-security.md)]. Password reset: [password-reset.md](password-reset.md). Notes: [notifications.md](notifications.md). Functional status v0.22 remains valid.] Extension: [admin-extension-api.md](admin-extension-api.md), [admin-custom-views.md](admin-custom-views.md), [admin-custom-filters.md](admin-custom-filters.md), [admin-custom-widgets.md](admin-custom-widgets.md). Diagnosis: [migration-diagnostics.md](migration-diagnostics.md). Upgrade: [project-upgrade.md](project-upgrade.md)]. Manifest: [project-manifest.md](project-manifest.md). Legacy: [legacy-projects.md](legacy-projects.md). v0.20] remains valid. The cargo version remains `0.1.0`. The admin is server-side HTML: [admin-native.md](admin-native.md). Transactions: [transactions.md](transactions.md).] Directory audit: [admin-directory-audit.md](admin-directory-audit.md)]. Parent-child forms: [admin-transactional-inlines.md](admin-transactional-inlines.md)]. Drafts, inlines, Collective Actions and History: [admin-form-drafts.md](admin-form-drafts.md), [admin-inlines.md](admin-inlines.md), [admin-bulk-actions.md](admin-bulk-actions.md), [admin-audit-log.md](admin-audit-log.md), [admin-custom-actions.md](admin-custom-actions.md). Date and time: [date-time-types.md](date-time-types.md)]. SQLite is the local default database, PostgreSQL is the optional backend. **Language of documentation:** German

## How to Read This Manual

Each document separates **IST** from **SOLL (Draft)**. SOLL refers to an architecture decision or proposal, **not an implemented function**. Sample code and CLI calls in DESIRED sections are not executable unless the function is implemented. Authentication is v0.7. The automatic admin is v0.8. Query scopes, stable CSRF and embedded admin assets are v0.8.1. Settings, Tera pages and prepared bilingualism are v0.10: [settings.md](settings.md), [templates.md](templates.md), [i18n.md](i18n.md)]. Full admin translation, error codes and branding are v0.11: [localization.md](localization.md), [error-codes.md](error-codes.md), [admin-directory.md](admin-directory.md)]. Create policies, the admin feature and the developer CLI are v0.9: [project-generator.md](project-generator.md), [project-templates.md](project-templates.md), [development-server.md](development-server.md), [cli-doctor.md](cli-doctor.md)]. Forms, static files, uploads and middleware are v0.12: [forms.md](forms.md), [forms-validation.md](forms-validation.md),] [staticfiles.md](staticfiles.md), [storage.md](storage.md), [uploads.md](uploads.md), [middleware.md](middleware.md), [request-lifecycle.md](request-lifecycle.md)]. The cargo version remains `0.1.0`.

**Central rule:** Rust-Struct → `FerrioxidModel` macro → typed repository implementation and model descriptor → explicit registration → HTTP routes. Schema changes run through the migration engine. `myshop --bootstrap` is legacy.

## Parts and their documents

| Modul | Zustand | Dokument | Kernverantwortung |
|---|---|---|---|
| Gesamtarchitektur |ACTual + REference| [architecture.md](architecture.md) |Boundaries, dependencies and data flow|
|Public API (`ferrioxid`)| IST | [facade.md](facade.md) |Re-Exports, Prelude, Stability Limit|
| `ferrioxid-core` |ACTual + REference| [core.md](core.md) |Traits, metadata, errors|
| `ferrioxid-macros` |ACTual + REference| [macros.md](macros.md) |Parsing, validation, code generation|
| `ferrioxid-db` |ACTual + REference| [database.md](database.md) | Typisierte Repositories, Connection, SQL |
| `ferrioxid-api` |ACTual + REference| [api.md](api.md) |Axum routing, DTOs, error handling|
| `ferrioxid-cli` | IST | [cli.md](cli.md), [migration-diagnostics.md](migration-diagnostics.md), [project-upgrade.md](project-upgrade.md), [project-generator.md](project-generator.md), [development-server.md](development-server.md), [cli-doctor.md](cli-doctor.md), [project-templates.md](project-templates.md) |Generator, Diagnosis, Upgrade, Development Server|
| `myshop` | IST | [example-myshop.md](example-myshop.md) |Integration example and start|
| Konfiguration |ACTual v0.24| [settings.md](settings.md), [custom-settings.md](custom-settings.md), [configuration.md](configuration.md) |`ferrum.toml`, environment, own sections|
|URLs and views|ACTual v0.24| [urls.md](urls.md), [routing.md](routing.md), [views.md](views.md), [project-structure.md](project-structure.md) |Named Axum routes, answers, project layout|
| Jobs |ACTual v0.27.1| [jobs.md](jobs.md), [workers.md](workers.md), [job-retries.md](job-retries.md), [job-security.md](job-security.md), [job-mail-integration.md](job-mail-integration.md) |Queue, Application Worker, Covert Payload, Mail Without Reset Tokens|
| Apps |ACTual v0.26| [apps.md](apps.md), [app-registry.md](app-registry.md), [app-migrations.md](app-migrations.md), [app-lifecycle.md](app-lifecycle.md), [reusable-apps.md](reusable-apps.md), [create-app.md](create-app.md) |Typical registration, verified start, app migrations|
| Modelle |ACTual v0.26| [models.md](models.md), [model-components.md](model-components.md), [model-timestamps.md](model-timestamps.md), [model-soft-delete.md](model-soft-delete.md) |A model definition, flat field groups, managed timestamps|
| Sicherheit |ACTual + REference| [security.md](security.md) |Access Defaults, Threats, Checks|
| Datenvalidierung |ACTual + REference| [validation.md](validation.md) |Rules and Error Semantics|
| Tests & CI |ACTual + REference| [testing.md](testing.md) | SQLite-HTTP im Workspace-Test, PostgreSQL optional |
|Migrations|IST, with designated boundaries| [migrations.md](migrations.md) |Snapshots, constraints, explicit renaming, verified SQLite conversion, baseline|
|Constraints and Indices| IST | [constraints.md](constraints.md) |Unique, index, foreign key, SQL and error codes|
| Modellbeziehungen |IST, Lazy Loading| [relations.md](relations.md) | Belongs-To, Has-Many, N+1 |
| Auth & Rechte | IST | [auth.md](auth.md), [password-reset.md](password-reset.md), [sessions.md](sessions.md), [permissions.md](permissions.md), [policies.md](policies.md), [csrf.md](csrf.md), [query-scopes.md](query-scopes.md) |Users, sessions, reset, RBAC, object rules, scopes|
| E-Mail |ACTual v0.23| [mail.md](mail.md), [mail-smtp.md](mail-smtp.md), [notifications.md](notifications.md), [mail-security.md](mail-security.md) |Shipping, templates, instructions, no delivery guarantee|
| Formulare |ACTual v0.12| [forms.md](forms.md), [forms-validation.md](forms-validation.md) |Typical fields, Tera-HTML, Common Rule Core|
| Staticfiles |ACTual v0.12| [staticfiles.md](staticfiles.md) |Project CSS and JavaScript, separate from the admin|
|Storage and Uploads|IST v0.12, S3 planned| [storage.md](storage.md), [uploads.md](uploads.md) |Local storage, private files|
| Middleware |ACTual v0.12| [middleware.md](middleware.md), [request-lifecycle.md](request-lifecycle.md) | Request-ID, Log, Limits, Header |
| Admin |ACTual v0.22| [admin.md](admin.md), [admin-native.md](admin-native.md), [admin-extension-api.md](admin-extension-api.md), [transactions.md](transactions.md), [admin-directory-audit.md](admin-directory-audit.md), [admin-transactional-inlines.md](admin-transactional-inlines.md), [admin-form-drafts.md](admin-form-drafts.md), [admin-inlines.md](admin-inlines.md), [admin-bulk-actions.md](admin-bulk-actions.md), [admin-audit-log.md](admin-audit-log.md), [admin-custom-actions.md](admin-custom-actions.md), [admin-django-parity.md](admin-django-parity.md) |HTML interface, JSON API, rights, drafts, inlines, transactions, collection actions, history, registered extensions|
| OpenAPI |**reference only ***| [openapi.md](openapi.md) |Documentation from DTOs|
| Erweiterungen |**reference only ***| [extensions.md](extensions.md) |Hooks, Plugin Boundaries|
| Deployment |IST SQLite, optional Docker PostgreSQL, SOLL app| [deployment.md](deployment.md) | Lokaler Start, Proxy, Produktion |
| Umsetzungsprozess |ACTual + REference| [implementation-guide.md](implementation-guide.md) |Work orders, acceptance, ADR|
| Roadmap | Planung | [roadmap.md](roadmap.md) |Milestones and order|

## Mandatory terms

- **Model:** Developer-side, serializable Rust Struct with `#[derive(FerrioxidModel)]`.
- **Descriptor:** `ModelDescriptor` with name, table, API path, `allow_anonymous`, `permission` and `requires_object_policy`. Columns are additionally in `ColumnSpec`.
- **Repository:** Typed methods generated `list`, `get`, `create`, `update` and `delete`. `update` takes `{Model}Update` with `PatchField<T>`.
- **Migration:** versioned file under `migrations/`, applied via `ferrioxid migrate up`.
- **Bootstrap:** Legacy command without schema change. `App::bootstrap_schema` remains a test assistant for empty databases without history.
- **Registration:** `App::new(db).register::<Product>()`, explicit and typed.
- **reference:** Scheduled function; is not communicated as a finished feature.

## Verifikationsbasis

Files: `crates/*/src/*`, `examples/myshop/src/*`, `examples/myshop/tests/postgres_api.rs`, `Cargo.toml`, `.github/workflows/ci.yml`, `compose.yaml`. The SQLite HTTP test belongs to `cargo test --workspace`. The PostgreSQL integration test remains ignored and is not considered to be passed without a running server.
