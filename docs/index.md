# Ferrioxid documentation portal

Ferrioxid is an experimental Rust framework with a model-first data layer, versioned migrations, authentication, a native admin, typed apps and background jobs. The current feature milestone is v0.28; the Cargo package version remains `0.1.0`. SQLite is the local default, and PostgreSQL is the optional server backend. The documentation is in English; the admin supports German and English.

## How to read this manual

**Implemented** describes behavior present in the source. **Planned** describes a proposal or limit to be addressed later, not a completed feature. Code in planned sections is illustrative unless the page explicitly identifies an implemented API. Feature milestone numbers are separate from package versions.

The main flow is: Rust model → `FerrioxidModel` metadata and repository methods → explicit registration → HTTP routes. Schema changes use the migration engine. `myshop --bootstrap` is a legacy compatibility option and does not create a schema.

## Django capability comparison

The [Django 6.1 capability matrix](django-capability-matrix.md) compares Ferrioxid's documented behavior with Django's major developer benefits, tracks missing/partial capabilities and their issues, and defines the **documentation-and-test gate** required before a feature can be called complete. Broad topic labels in the table below indicate existing implementation, **not** full Django parity; consult the matrix for the sub-capability status.

## Documentation by topic

| Topic | Status | Pages | Responsibility |
|---|---|---|---|
| Architecture | Implemented and planned | [architecture.md](architecture.md) | Module boundaries, dependencies and runtime data flow |
| Public facade | Implemented | [facade.md](facade.md) | Re-exports, prelude and stability limits |
| Core | Implemented and planned | [core.md](core.md) | Traits, metadata and shared errors |
| Macros | Implemented and planned | [macros.md](macros.md) | Parsing, validation and code generation |
| Database | Implemented and planned | [database.md](database.md) | Typed repositories, decimal and choice columns, composable queries and SQL |
| Django capability matrix | Inventory, not a completion claim | [django-capability-matrix.md](django-capability-matrix.md) | Evidence status for Django-shaped capabilities |
| Framework quality review | Local development review, not a release | [framework-quality-review.md](framework-quality-review.md) | What the current local branch verified and which gaps remain |
| CLI | Implemented | [cli.md](cli.md), [fixtures.md](fixtures.md), [management-commands.md](management-commands.md), [migration-diagnostics.md](migration-diagnostics.md), [project-upgrade.md](project-upgrade.md), [project-generator.md](project-generator.md), [development-server.md](development-server.md), [cli-doctor.md](cli-doctor.md), [project-templates.md](project-templates.md) | Generation, diagnostics, fixtures, project commands, upgrades, builds and development server |
| Example | Implemented | [example-myshop.md](example-myshop.md) | The myshop integration example |
| Settings | Implemented, v0.24 | [settings.md](settings.md), [custom-settings.md](custom-settings.md), [configuration.md](configuration.md) | `ferrioxid.toml`, environment overrides and custom sections |
| URLs and views | Implemented, v0.24 | [urls.md](urls.md), [routing.md](routing.md), [views.md](views.md), [templates.md](templates.md), [project-structure.md](project-structure.md) | Named Axum routes, responses, templates and project layout |
| Jobs | Implemented, v0.27.1 | [jobs.md](jobs.md), [workers.md](workers.md), [job-retries.md](job-retries.md), [job-security.md](job-security.md), [job-mail-integration.md](job-mail-integration.md) | Persistent queue, application workers, hidden payloads and mail without reset tokens |
| Apps | Implemented, v0.26 | [apps.md](apps.md), [app-registry.md](app-registry.md), [app-migrations.md](app-migrations.md), [app-lifecycle.md](app-lifecycle.md), [reusable-apps.md](reusable-apps.md), [create-app.md](create-app.md) | Typed registration, startup checks and app migrations |
| Models | Implemented, v0.26 | [models.md](models.md), [model-components.md](model-components.md), [model-timestamps.md](model-timestamps.md), [model-soft-delete.md](model-soft-delete.md) | Model definitions, reusable fields, managed timestamps and explicit limits |
| Security | Implemented and planned | [security.md](security.md) | Access defaults, threats, safeguards and open risks |
| Validation | Implemented and planned | [validation.md](validation.md) | Field rules and error semantics |
| Tests and CI | Implemented and planned | [testing.md](testing.md) | Workspace checks, SQLite tests and separate PostgreSQL tests |
| Migrations | Implemented with stated limits | [migrations.md](migrations.md) | Snapshots, diffs, explicit renames, checked rebuilds and baseline |
| Constraints | Implemented | [constraints.md](constraints.md) | Unique constraints, indexes, foreign keys and error codes |
| Relations | Implemented, lazy and explicit batch loading | [relations.md](relations.md) | Belongs-to, has-many, batch loads and N+1 limits |
| Auth and permissions | Implemented | [auth.md](auth.md), [password-reset.md](password-reset.md), [sessions.md](sessions.md), [permissions.md](permissions.md), [policies.md](policies.md), [csrf.md](csrf.md), [query-scopes.md](query-scopes.md) | Users, sessions, reset, role permissions, object policies and query scopes |
| Email | Implemented, v0.23 | [mail.md](mail.md), [mail-smtp.md](mail-smtp.md), [notifications.md](notifications.md), [mail-security.md](mail-security.md) | Transports, templates, typed notices and delivery limits |
| Forms | Implemented, v0.12 | [forms.md](forms.md), [forms-validation.md](forms-validation.md) | Typed fields, Tera HTML and shared validation rules |
| Static files | Implemented, v0.12 | [staticfiles.md](staticfiles.md) | Application CSS and JavaScript, separate from admin assets |
| Storage and uploads | Local storage implemented; S3 planned | [storage.md](storage.md), [uploads.md](uploads.md) | Local files, metadata and private downloads |
| Middleware | Implemented, v0.12 | [middleware.md](middleware.md), [request-lifecycle.md](request-lifecycle.md) | Request IDs, logging, limits and security headers |
| Admin | Implemented through v0.28 | [admin.md](admin.md), [admin-native.md](admin-native.md), [admin-navigation.md](admin-navigation.md), [admin-components.md](admin-components.md), [admin-extension-api.md](admin-extension-api.md), [transactions.md](transactions.md), [admin-directory-audit.md](admin-directory-audit.md), [admin-transactional-inlines.md](admin-transactional-inlines.md), [admin-form-drafts.md](admin-form-drafts.md), [admin-inlines.md](admin-inlines.md), [admin-bulk-actions.md](admin-bulk-actions.md), [admin-audit-log.md](admin-audit-log.md), [admin-custom-actions.md](admin-custom-actions.md), [admin-django-parity.md](admin-django-parity.md) | Native HTML, JSON API, permissions, navigation, forms, transactions, actions, history and extensions |
| OpenAPI | Planned only | [openapi.md](openapi.md) | API documentation generated from metadata and DTOs |
| General plugin hooks | Planned only | [extensions.md](extensions.md) | Extension points beyond the implemented admin API |
| Deployment | Local SQLite and optional Docker PostgreSQL implemented; app container planned | [deployment.md](deployment.md) | Local startup, proxy boundaries and production requirements |
| Implementation process | Implemented and planned | [implementation-guide.md](implementation-guide.md) | Change scope, acceptance checks and architecture decisions |
| Roadmap | Planning | [roadmap.md](roadmap.md) | Milestones and remaining work |

## Terms

- **Model:** a serializable Rust struct using `#[derive(FerrioxidModel)]`.
- **Descriptor:** `ModelDescriptor` describes the model name, table, API path, `allow_anonymous`, `permission` and `requires_object_policy`; `ColumnSpec` describes columns.
- **Repository:** generated typed `list`, `get`, `create`, `update` and `delete` methods. `update` accepts `{Model}Update` with `PatchField<T>`.
- **Migration:** a versioned file under `migrations/`, applied with `ferrioxid migrate up`.
- **Bootstrap:** the legacy command makes no schema changes. `App::bootstrap_schema` is a test helper for empty databases without migration history.
- **Registration:** explicit typed registration such as `App::new(db).register::<Product>()`.
- **Planned:** work that must not be described as an available feature.

## Verification sources

Implementation evidence comes from `crates/*/src/*`, `examples/myshop/src/*`, `examples/myshop/tests/postgres_api.rs`, `Cargo.toml`, `.github/workflows/ci.yml` and `compose.yaml`. The workspace test run includes SQLite HTTP tests. Ignored PostgreSQL tests count as passed only when actually run against a disposable server.

## Maintainer records

- [Breaking configuration and schema naming change](ferrum-legacy-removal.md)

- [GitHub maintenance](github-maintenance.md)
- [Historical rename plan and compatibility policy](ferrioxid-rename-plan.md)
- [Completed translation review](translation-review.md)
