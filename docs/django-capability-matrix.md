# Django 6.1 vs Ferrioxid — capability and evidence matrix

[Documentation portal](index.md) · [Roadmap](roadmap.md) · [Admin comparison](admin-django-parity.md) · [Django 6.1 documentation](https://docs.djangoproject.com/en/6.1/) · [Django 6.1 release notes](https://docs.djangoproject.com/en/6.1/releases/6.1/)

**Baseline:** 2026-11-09. Ferrioxid baseline is `main` after merged PR #56 (commit `45f260c`). The descriptions below reflect the project's documentation and the TestShop integration report; they are **not** an exhaustive source-code or runtime audit. The matrix must be rechecked against implementation and tests before any status is promoted to Complete. Django is a **capability reference**, not a requirement to copy Python interfaces.

## Status contract

- **Complete**: The named, narrowly scoped capability exists in `main`, has passing automated success/failure/security tests relevant to that claim, a runnable user example, and documentation specifying API, supported backends and limits. A successful PR or an existing documentation heading alone is insufficient.
- **Partial**: Useful supported behavior exists, but at least one named capability or required verification is missing. Describe what works; never show the overall category as complete.
- **In PR**: Implementation is not merged. A green CI job is not a completed release; do not mark Complete until merge, test evidence and documentation are checked.
- **Planned**: Not implemented, but scoped in an issue or roadmap.
- **Not supported**: Explicitly outside the currently supported design; do not imply a future commitment.
- **Unverified**: Repository evidence was not checked enough to make a defensible claim.

Use **per-capability status**, not a single binary label for large categories. Each Issue/PR should change affected rows and exact docs in the same change. Backend assertions must distinguish SQLite, PostgreSQL SQL generation, and PostgreSQL runtime tests.

## Core parity matrix

| Area | Django 6.1 capability / user expectation | Ferrioxid baseline in `main` | Status | Follow-up / verification |
|---|---|---|---|---|
| Project and app structure | Project-level settings, modular installed apps and app registry | `new project`, `new app`, typed `App::install`, explicit TOML+Rust registration | Partial | [project-generator](project-generator.md), [apps](apps.md); app lifecycle, install/removal and reusable packaging require deeper parity audit |
| Project generation | Working development project, reusable templates | Fullstack/API generators, `ferrioxid dev`, CLI diagnostics | Partial | [project-generator](project-generator.md); validate upgrades and generated production layout |
| Model declarations | Typed fields, options, constraints, defaults, relationships | `FerrioxidModel`, descriptors, UUID/string/i64/bool/date/time/datetime, indexes and unique constraints | Partial | [models](models.md), [constraints](constraints.md); decimal, JSON, enums/choices and additional field types [#49](https://github.com/Iam-Sanja/ferrioxid/issues/49) |
| Basic CRUD | Create/read/update/delete through model data access | Typed generated repositories and REST CRUD; `PatchField` and `Default` for update DTOs (PR #55) | Partial | [database](database.md); missing query composition and advanced ORM behavior |
| Filtering and sorting | Chained QuerySets, Q predicates, lookups, expressions, ordering | `list`, `get`, `find_by_column`, limited equality matching; admin has separate query/pagination features | Partial | [#44](https://github.com/Iam-Sanja/ferrioxid/issues/44) |
| Pagination | Database-backed count/page APIs, predictable ordering | Default generated REST list capped at 100; admin paginates separately | Partial | [#44](https://github.com/Iam-Sanja/ferrioxid/issues/44) |
| Aggregation and annotations | Count, sum, grouping, SQL expressions and annotations | No supported QuerySet-equivalent identified in project docs | Planned | New issue after query-builder design; do not equate custom SQL with framework support |
| Relations and N+1 control | FK, one-to-one, many-to-many, select/prefetch related, configurable fetch modes | Belongs-to/has-many lazy methods; cross-app FK merged in PR #56; no general batch/eager loading or many-to-many | Partial; cross-app FK **merged** | [relations](relations.md), [#42](https://github.com/Iam-Sanja/ferrioxid/issues/42), [#45](https://github.com/Iam-Sanja/ferrioxid/issues/45); Django 6.1 adds configurable field fetch modes |
| Transactions | Atomic, savepoints, transactional reads/writes | Database and admin transactional operations; some repository reads not transaction-bound | Partial | [transactions](transactions.md), [#45](https://github.com/Iam-Sanja/ferrioxid/issues/45) |
| Schema migrations | Dependency graphs, reversible operations, data migrations, safe evolution | Versioned migrations, diffs, explicit renames, limited rebuilds; cross-app dependency graph and explicit required-column backfill merged in PR #56 | Partial; selected fixes **merged** | [migrations](migrations.md), [#42](https://github.com/Iam-Sanja/ferrioxid/issues/42), [#43](https://github.com/Iam-Sanja/ferrioxid/issues/43); no general `migrate down`/data migration framework |
| Database backends | SQLite, PostgreSQL, MySQL/MariaDB, Oracle (varying capabilities) | SQLite and PostgreSQL; PostgreSQL CI runtime tests when infrastructure available | Partial | [database](database.md), [testing](testing.md); MySQL/Oracle not supported |
| Forms | Field validation, bound forms, widgets, ModelForm, formsets | Hand-written `FerrioxidForm`; text, number, email, password, checkbox, select and textarea widgets; explicit model allow-list with create/replace versus patch. **In PR** for [#47](https://github.com/Iam-Sanja/ferrioxid/issues/47). Formsets are not implemented. | Partial | [forms](forms.md), [#47](https://github.com/Iam-Sanja/ferrioxid/issues/47) |
| Validation / i18n | Custom validators, per-field messages, locale-aware display | Stable codes, German and English catalog, per-field overrides and safe `{min}`/`{max}` substitution. **In PR** for [#48](https://github.com/Iam-Sanja/ferrioxid/issues/48). JSON `detail` stays the stored text. | Partial | [validation](validation.md), [i18n](i18n.md), [#48](https://github.com/Iam-Sanja/ferrioxid/issues/48) |
| Templates | Inheritance, context, filters/tags, reverse URLs, custom error views | Tera templates and app template support; project-level `mount_pages` URL reversing and `views::fail` integration incomplete | Partial | [templates](templates.md), [urls](urls.md), [#50](https://github.com/Iam-Sanja/ferrioxid/issues/50) |
| Named routing and views | Namespaced reverse, request/response helpers, generic views | Named Axum routes, view helpers and static routes; missing uniform reverse in all contexts | Partial | [urls](urls.md), [views](views.md), [#50](https://github.com/Iam-Sanja/ferrioxid/issues/50); generic class-based views are not a necessary Rust-shaped API |
| Static files / uploads | Static collection, storage abstractions, model file fields | Static assets and local upload/storage mechanisms | Partial | [staticfiles](staticfiles.md), [storage](storage.md), [uploads](uploads.md); model-file integration, production collection and remote storage to audit |
| Authentication | Users, passwords, sessions, permissions, password resets, pluggable backends | Argon2id, login, staff/superuser, roles, object policies, password reset | Partial | [auth](auth.md); no MFA, OAuth or third-party auth parity |
| Anonymous session and CSRF | Secure anonymous session state and CSRF on cookie-based writes | Authenticated session CSRF implemented; public anonymous forms/cart have no equivalent built-in guard | Partial / security gap | [#46](https://github.com/Iam-Sanja/ferrioxid/issues/46), [csrf](csrf.md), [security](security.md) |
| Admin model CRUD | List, search, filters, fieldsets, detail/edit, inlines, actions, audit | Native HTML admin, role editors, unified record editor, extension actions, logs | Partial | [admin](admin.md), [admin-django-parity](admin-django-parity.md); cross-app relation labels merged in PR #56 [#53](https://github.com/Iam-Sanja/ferrioxid/issues/53); audit sidebar [#32](https://github.com/Iam-Sanja/ferrioxid/issues/32) |
| Admin extensions / permissions | Custom pages/actions/widgets, authenticated and object-scoped actions | Registered pages/actions/filters/widgets, object scope and auditing | Partial | [admin-extension-api](admin-extension-api.md), [admin-custom-actions](admin-custom-actions.md); feature-by-feature parity test required |
| Fixtures / serialization | `dumpdata`, `loaddata`, natural keys and ordered fixtures | TestShop data currently seeded through an ignored Rust test; no general `loaddata` | Planned | [#51](https://github.com/Iam-Sanja/ferrioxid/issues/51) |
| Management commands / checks | Custom app commands, `check`, custom system checks | `ferrioxid check` and `doctor`; project-specific command registry missing | Partial | [cli](cli.md), [#52](https://github.com/Iam-Sanja/ferrioxid/issues/52) |
| Async/background tasks | Register, enqueue and process application tasks | Durable jobs, worker host, retry support, limited handler topology | Partial | [jobs](jobs.md), [workers](workers.md); cron, schedules, worker orchestration, idempotency |
| Email | Multiple transports, templates, attachments and delivery options | Email interfaces, disabled/console/memory and SMTP limits documented | Partial | [mail](mail.md), [mail-smtp](mail-smtp.md); verify production transport behavior |
| Middleware / request lifecycle | Security, authentication hooks, middleware ordering, request processing | Middleware, request IDs, headers and limits | Partial | [middleware](middleware.md); extension ordering and request lifecycle audit |
| Testing | Test client, fixtures, database isolation, framework assertions | Cargo unit/integration/HTTP tests, CI and SQLite/PostgreSQL paths | Partial | [testing](testing.md); test-client ergonomics, app test fixtures, isolation and ignored-test coverage |
| Caching | Cache API, per-view/site/template caching, backends | No first-class cache framework documented | Planned / unscoped | Define explicit requirements and issue before claiming parity |
| Observability | Structured logging, error reporting, DB query instrumentation | Request logging and diagnostics exist; comprehensive observability unverified | Unverified | Audit logging, tracing, query counts, metrics and hooks |
| Signals / lifecycle events | Model and request events, transactional hooks | App lifecycle and admin audit exist; general signal/event contract unverified | Unverified | Audit [app-lifecycle](app-lifecycle.md); define typed events rather than copy Python signals |
| Security operations | CSRF, clickjacking, session protection, host validation, CSP, signing | Several security headers/CSRF mechanisms; anonymous cookie gap; production status experimental | Partial | [security](security.md), [#46](https://github.com/Iam-Sanja/ferrioxid/issues/46); check CSP, trusted proxies, signing, key rotation, deployment |
| HTTP optimization | Conditional responses, ETag, cache control, streaming | Not verified comprehensively | Unverified | New audit issue; no blanket parity claim |
| Advanced contrib | Content types, generic relations, sites, sitemaps, feeds, messages | No general equivalent documented | Planned / unscoped | Prioritize according to real app use cases; not all require core crates |
| Production readiness | Deployment checks, monitoring, stable API and migration contracts | `0.1.0` experimental; local runtime and optional PostgreSQL | Partial | [deployment](deployment.md), [security](security.md); production acceptance gate required |

## Recommended gap-driven delivery order

1. **Data integrity + security**: [#42](https://github.com/Iam-Sanja/ferrioxid/issues/42), [#43](https://github.com/Iam-Sanja/ferrioxid/issues/43), [#46](https://github.com/Iam-Sanja/ferrioxid/issues/46). #42 and #43 merged via PR #56; #46 remains open.
2. **Common application coding**: [#44](https://github.com/Iam-Sanja/ferrioxid/issues/44) and [#45](https://github.com/Iam-Sanja/ferrioxid/issues/45); avoid designing two incompatible query interfaces.
3. **Forms and URLs**: [#47](https://github.com/Iam-Sanja/ferrioxid/issues/47), [#48](https://github.com/Iam-Sanja/ferrioxid/issues/48), [#50](https://github.com/Iam-Sanja/ferrioxid/issues/50).
4. **Admin experience**: [#53](https://github.com/Iam-Sanja/ferrioxid/issues/53) merged via PR #56, [#32](https://github.com/Iam-Sanja/ferrioxid/issues/32).
5. **Fixtures, custom commands, field types**: [#49](https://github.com/Iam-Sanja/ferrioxid/issues/49), [#51](https://github.com/Iam-Sanja/ferrioxid/issues/51), [#52](https://github.com/Iam-Sanja/ferrioxid/issues/52).
6. **Django benefits not yet scoped**: caching, comprehensive aggregations, richer model relations, generic reusable views, messaging, signals, structured observability, deployment readiness, advanced contrib features. Create narrowly scoped issues after architectural review and example-user requirements.

## Documentation and completion gate (mandatory for future features)

Each feature PR must include all applicable parts in one reviewable change:

1. **Contract**: public Rust API, CLI command or HTTP endpoint, behavior, error states, compatibility and security model; decide which crate owns it.
2. **Implementation evidence**: exact source paths and tests, including negative cases. For DB features: SQLite and PostgreSQL runtime tests, not just SQL rendering, if both are claimed as supported.
3. **Developer tutorial**: a compilable or executable example in the topic documentation or generated sample app, with setup and expected output.
4. **Limits**: unsupported field types, backend differences, migration hazards, concurrency, pagination/resource limits and rollback behavior.
5. **Docs update**: topic doc, this matrix and, for milestone changes, [roadmap](roadmap.md) plus relevant [index](index.md) status wording. Update the README only if its developer-facing feature/limit statement changed.
6. **Issue linkage**: `Closes #N` only when the issue's acceptance criteria are fully met; otherwise `Refs #N` and name the remaining gap.
7. **Status promotion**: `In PR` during review; `Complete` only after merge, green required CI, tested behaviors and documented limitations. A broad topic such as "Auth" or "Forms" remains Partial while any essential stated sub-capabilities remain missing.
8. **Regression**: Existing generated projects and databases must remain valid or have documented upgrade instructions; especially never silently alter applied migration checksums.

## Update record

- **2026-11-09**: First inventory from Ferrioxid README/docs, TestShop vs Django observations and official Django 6.1 documentation. This is a starting inventory, **not** a declaration of full implementation coverage. PR #56 merged 2026-11-09 with required CI green; its delivered sub-capabilities are marked merged, while broad feature areas remain Partial.
