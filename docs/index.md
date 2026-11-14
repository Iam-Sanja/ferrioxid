# Fexu documentation portal

Fexu is an experimental Rust framework with a model-first data layer, versioned migrations, authentication, a native admin, typed apps and background jobs. The Cargo package version is `0.1.0` (experimental); historical v0.x feature milestones are not registry releases. SQLite is the local default, and PostgreSQL is the optional server backend. The documentation is in English; the admin supports German and English.

## How to read this manual

**Implemented** describes behavior present in the source. **Planned** describes a proposal or limit to be addressed later, not a completed feature. Code in planned sections is illustrative unless the page explicitly identifies an implemented API. Feature milestone numbers are separate from package versions.

The main flow is: Rust model → `FexuModel` metadata and repository methods → explicit registration → HTTP routes. Schema changes use the migration engine. `myshop --bootstrap` is a legacy compatibility option and does not create a schema.

## Django capability comparison

The [Django 6.1 capability matrix](django-capability-matrix.md) compares Fexu's documented behavior with Django's major developer benefits. The [framework comparison](framework-comparison.md) records Fexu, Runique, Cot and Loco from published pages, without runtime numbers. [Release readiness](release-readiness.md) records the staged first crates.io publication; check individual crate pages for current availability.

## Documentation by topic

**Status labels below describe source capabilities, not published versions.** The numbered `v0.x` labels in older per-feature pages refer to historical development milestones. Fexu's Cargo version is `0.1.0` and publishing proceeds by crate dependency order. Verify any registry-based install commands against the live registry.

| Topic | Status | Pages | Responsibility |
|---|---|---|---|
| Architecture | Implemented and planned | [architecture.md](architecture.md) | Module boundaries, dependencies and runtime data flow |
| Public facade | Implemented | [facade.md](facade.md) | Re-exports, prelude and stability limits |
| Core | Implemented and planned | [core.md](core.md) | Traits, metadata and shared errors |
| Macros | Implemented and planned | [macros.md](macros.md) | Parsing, validation and code generation |
| Database | Implemented and planned | [database.md](database.md), [database-driver.md](database-driver.md), [postgres-portability-v2.md](postgres-portability-v2.md), [orm-capability-matrix.md](orm-capability-matrix.md), [orm-write-security-audit.md](orm-write-security-audit.md) | Typed repositories, decimal and choice columns, composable queries and SQL. The portability note, capability matrix and write audit record the SQLite and PostgreSQL behavior from November 2026 |
| Django capability matrix | Inventory, not a completion claim | [django-capability-matrix.md](django-capability-matrix.md) | Evidence status for Django-shaped capabilities |
| Framework quality review | Historical snapshot | [framework-quality-review.md](framework-quality-review.md) | Dated development-branch review; do not use as current release evidence |
| CLI | Implemented | [cli.md](cli.md), [fixtures.md](fixtures.md), [management-commands.md](management-commands.md), [migration-diagnostics.md](migration-diagnostics.md), [project-upgrade.md](project-upgrade.md), [project-generator.md](project-generator.md), [development-server.md](development-server.md), [cli-doctor.md](cli-doctor.md), [project-templates.md](project-templates.md) | Generation, diagnostics, fixtures, project commands, upgrades, builds and development server |
| Example | Implemented | [example-myshop.md](example-myshop.md) | The myshop integration example |
| Settings | Implemented | [settings.md](settings.md), [custom-settings.md](custom-settings.md), [configuration.md](configuration.md) | `fexu.toml`, environment overrides and custom sections |
| URLs and views | Implemented | [urls.md](urls.md), [routing.md](routing.md), [views.md](views.md), [templates.md](templates.md), [project-structure.md](project-structure.md) | Named Axum routes, responses, templates and project layout |
| Jobs | Implemented | [jobs.md](jobs.md), [job-scheduling.md](job-scheduling.md), [workers.md](workers.md), [job-retries.md](job-retries.md), [job-security.md](job-security.md), [job-mail-integration.md](job-mail-integration.md) | Persistent queue, cron in UTC, Europe/Berlin and fixed offsets, application workers, hidden payloads and mail without reset tokens |
| Cache | Implemented, process-local | [cache.md](cache.md) | Optional typed cache. Not a distributed cache |
| Advanced services | Implemented with stated limits | [advanced-services.md](advanced-services.md) | How auth, jobs, cache and storage fit together, and what #97–#100 do not provide |
| Apps | Implemented | [apps.md](apps.md), [app-registry.md](app-registry.md), [app-migrations.md](app-migrations.md), [app-lifecycle.md](app-lifecycle.md), [reusable-apps.md](reusable-apps.md), [create-app.md](create-app.md) | Typed registration, startup checks and app migrations |
| Models | Implemented | [models.md](models.md), [model-components.md](model-components.md), [model-timestamps.md](model-timestamps.md), [model-soft-delete.md](model-soft-delete.md) | Model definitions, reusable fields, managed timestamps and explicit limits |
| Security | Implemented and planned | [security.md](security.md) | Access defaults, threats, safeguards and open risks |
| Validation | Implemented and planned | [validation.md](validation.md) | Field rules and error semantics |
| Tests and CI | Implemented and planned | [testing.md](testing.md) | Workspace checks, SQLite tests and PostgreSQL runtime CI and browser tests |
| Migrations | Implemented with stated limits | [migrations.md](migrations.md), [advanced-framework-progress.md](advanced-framework-progress.md) | Snapshots, diffs, explicit renames, checked rebuilds, baseline and read-only drift plan |
| Constraints | Implemented | [constraints.md](constraints.md) | Unique constraints, indexes, foreign keys and error codes |
| Relations | Implemented, lazy and explicit batch loading | [relations.md](relations.md) | Belongs-to, has-many, many-to-many, one-to-one and N+1 limits |
| Auth and permissions | Implemented | [auth.md](auth.md), [auth-providers.md](auth-providers.md), [auth-threat-model.md](auth-threat-model.md), [password-reset.md](password-reset.md), [sessions.md](sessions.md), [permissions.md](permissions.md), [policies.md](policies.md), [csrf.md](csrf.md), [query-scopes.md](query-scopes.md) | Users, sessions, TOTP, reset, role permissions, object policies and query scopes |
| Email | Implemented | [mail.md](mail.md), [mail-smtp.md](mail-smtp.md), [notifications.md](notifications.md), [mail-security.md](mail-security.md) | Transports, templates, typed notices and delivery limits |
| Forms | Implemented | [forms.md](forms.md), [forms-validation.md](forms-validation.md) | Typed fields, Tera HTML and shared validation rules |
| Static files | Implemented | [staticfiles.md](staticfiles.md) | Application CSS and JavaScript, separate from admin assets |
| Storage and uploads | Local storage implemented; remote object storage is configuration only | [storage.md](storage.md), [storage-remote.md](storage-remote.md), [uploads.md](uploads.md) | Local files, lifecycle, signed downloads and an S3 plan without a network client |
| Middleware | Implemented | [middleware.md](middleware.md), [request-lifecycle.md](request-lifecycle.md) | Request IDs, logging, limits and security headers |
| Admin | Implemented with documented limitations | [admin.md](admin.md), [admin-devex.md](admin-devex.md), [admin-native.md](admin-native.md), [admin-navigation.md](admin-navigation.md), [admin-components.md](admin-components.md), [admin-extension-api.md](admin-extension-api.md), [transactions.md](transactions.md), [admin-directory-audit.md](admin-directory-audit.md), [admin-transactional-inlines.md](admin-transactional-inlines.md), [admin-form-drafts.md](admin-form-drafts.md), [admin-inlines.md](admin-inlines.md), [admin-bulk-actions.md](admin-bulk-actions.md), [admin-audit-log.md](admin-audit-log.md), [admin-custom-actions.md](admin-custom-actions.md), [admin-django-parity.md](admin-django-parity.md) | Native HTML, JSON API, permissions, navigation, forms, transactions, actions, history and extensions |
| OpenAPI | Implemented in main | [openapi.md](openapi.md) | OpenAPI 3.1 from model metadata, plus an optional explorer |
| General plugin hooks | Planned only | [extensions.md](extensions.md) | Extension points beyond the implemented admin API |
| Deployment | Local SQLite, optional Docker PostgreSQL, and a disposable app image in `deploy/` | [deployment.md](deployment.md), [deploy-docker.md](deploy-docker.md) | Local startup, proxy boundaries, trusted proxies and embedded framework migrations |
| Implementation process | Implemented and planned | [implementation-guide.md](implementation-guide.md) | Change scope, acceptance checks and architecture decisions |
| Roadmap | Current priorities | [roadmap.md](roadmap.md) | Current capabilities, known limitations, open work and historical milestone distinction |

## Terms

- **Model:** a serializable Rust struct using `#[derive(FexuModel)]`.
- **Descriptor:** `ModelDescriptor` describes the model name, table, API path, `allow_anonymous`, `permission` and `requires_object_policy`; `ColumnSpec` describes columns.
- **Repository:** generated typed `list`, `get`, `create`, `update` and `delete` methods. `update` accepts `{Model}Update` with `PatchField<T>`.
- **Migration:** a versioned file under `migrations/`, applied with `fexu migrate up`.
- **Bootstrap:** the legacy command makes no schema changes. `App::bootstrap_schema` is a test helper for empty databases without migration history.
- **Registration:** explicit typed registration such as `App::new(db).register::<Product>()`.
- **Planned:** work that must not be described as an available feature.

## Verification sources

Implementation evidence comes from `crates/*/src/*`, `examples/myshop/src/*`, `examples/myshop/tests/postgres_api.rs`, `Cargo.toml`, `.github/workflows/ci.yml` and `compose.yaml`. The workspace test run includes SQLite HTTP tests. Ignored PostgreSQL tests count as passed only when actually run against a disposable server.

## Maintainer records

- [Integration of pull requests #97 to #100](advanced-framework-integration.md)
- [Fexu rename, no old-name compatibility](fexu-rename-plan.md)
- [Earlier name removal](ferrum-legacy-removal.md)
- [Historical compatibility policy](ferrioxid-rename-plan.md)
- [GitHub maintenance](github-maintenance.md)
- [Completed translation review](translation-review.md)
