# Ferrioxid Roadmap – Status and Next Phases

[Overview](index.md) · [Django 6.1 capability matrix](django-capability-matrix.md) · Admin comparison: [admin-django-parity.md](admin-django-parity.md) · UX backlog: [admin-ux-backlog.md](admin-ux-backlog.md)

**Status:** milestones through v0.28 are implemented; the Cargo version remains `0.1.0`. This roadmap distinguishes completed work from proposals. A planned phase is neither a release nor an available feature.

## Product decision

The default admin remains server-rendered HTML with Rust, Axum and Tera. React, Vite and Node.js are not standard admins. Core actions, i.e. lists, forms, deletion, login, log-out and directory, must work without JavaScript. The optional `admin.js` must only offer comfort, for example focus.

In the long term, the admin should cover the relevant functions of the Django admin in a functionally equivalent manner. The internal API does not have to be django compatible. The surface should become a dense working environment, not a decorative redesign.

## Implemented milestones

| Milestone | Status | Goal | Limit |
|---|---|---|---|
| v0.1 | Implemented; SQLite tested in the workspace | Typed models, list, GET, POST, PATCH, DELETE and safe migrations | PostgreSQL HTTP tests remain ignored unless a server is available. |
| v0.6 | Implemented | Unique constraints, indexes, foreign keys, belongs-to, has-many, snapshots and diffs | Many-to-many, eager loading, `down` and PostgreSQL baseline are missing. |
| v0.7 | Implemented |Argon2id, sessions, cookies, CSRF, RBAC, model permissions|OAuth, MFA and JWT are missing. Password reset follows in v0.23|
| v0.8 to v0.12 | Implemented | Admin metadata, JSON API, query scopes, generator, settings, forms, static files, uploads and middleware | The React interface was removed in v0.13 |
| v0.13 | Implemented |Native Admin under `/admin`, Tera, CSS in Binary, JSON API remains|Cargo version remains `0.1.0`|
| v0.13.1 | Implemented | Login origins follow the bind address; `Origin: null` is allowed only with `Sec-Fetch-Site: same-origin`. | The Host header never supplies the allowlist. |
| v0.14 | Implemented |Logging out on authenticated 403 pages, flag notices, standard columns|Users are still created v0.16|
| v0.15 | Implemented |Design tokens, field groups, table rhythm, role selection, contrast gate|Cargo version remains `0.1.0`|
| v0.16 | Implemented |User maintenance, server-side confirmation, readable foreign keys, filter selection, isolated generator databases|Superuser remains CLI. No user hard delete. JSON API without `confirm`|
| v0.17 | Implemented |List configuration, fieldsets, three save destinations, form state, foreign key selection page|No date type. Cargo version remains `0.1.0`|
| v0.18 | Implemented |Draft forms, has-many inlines, bulk actions, audit, registered actions|No common parent-child storage. Directory audit is not the same transaction|
| v0.19 | Completed |`Date`, `Time`, `DateTime<Utc>`, Migration `0004_events`, admin fields, UTC date hierarchy|PostgreSQL runtime untested. No tacit text-to-date conversion|
| v0.20 | Completed | Shared transactions for model, auth and audit writes, directory auditing and opt-in parent-child forms | No many-to-many or framework-wide optimistic locking. PostgreSQL runtime tests were not run for this milestone. |
| v0.21 | Completed |Reading Auth and App Diagnosis. `check --strict`. Upgrading only creates a missing legacy manifest|No automatic template replacement. No silent migrations. PostgreSQL runtime untested|
| v0.21.1 | Completed | Generator tests limit nested release builds to two jobs; the subsequent unrestricted workspace run passed | The cause of rustc crashes `0xc0000005` and `0xc0000409` remains unresolved |
| v0.22 | Completed |Public admin extension: Pages, actions, filters, widgets, templates|No second admin engine. PostgreSQL runtime untested. The rustc crash remains unexplained|
| v0.23 | Completed |Email service, self-service password reset, typed notes|No delivery guarantee. No admin notification center. PostgreSQL runtime untested|
| v0.24 | Completed | Typed settings, `[custom.*]`, `UrlSet`, view helpers and project layout | `require_user` does not check CSRF; API projects retain HTML homepages |
| v0.25 | Completed |`App::install`, App Migrations, Tera-`url`, App Templates and Static Files, `ferrioxid generate app`|`merge` of a raw router does not see names. PostgreSQL runtime untested|
| v0.27 | Completed |Persistent queue, lease, limited retries, worker, ordinary mail jobs|Cron is missing. Reset tokens remain outside the queue. PostgreSQL runtime untested|
| v0.27.1 | Completed | `WorkerHost`, `ferrioxid generate worker`, app handlers and hidden job payloads | CLI workers do not load application code; no exactly-once guarantee |
| v0.26 | Completed | Validated app startup, Rust/TOML migration matching, flat field groups and managed timestamps | Soft delete remains planned |
| v0.28 | Completed | Admin navigation grouped by app, model links and permission-aware visibility | See [admin-navigation.md](admin-navigation.md) for limits |

## In review

- Forms ([#47](https://github.com/Iam-Sanja/ferrioxid/issues/47)): text, number, email, password, checkbox, select and textarea widgets, escaped labels and values, and model binding through an explicit column allow-list. Create and replace require every required field. Patch leaves missing keys unchanged. Formsets are still absent. Status in the capability matrix: **In PR**.
- Validation display ([#48](https://github.com/Iam-Sanja/ferrioxid/issues/48)): one German/English catalog, field overrides, and model errors stored as stable codes. Unknown locales use English. SQL and stack traces are not shown. Status: **In PR**.
- Error pages and named routes ([#50](https://github.com/Iam-Sanja/ferrioxid/issues/50)): status templates with a safe fallback, project `url()` across prefixes, and percent-encoded path parameters. Status: **In PR**.

## ORM queries

`Model::query()` in [database.md](database.md) filters, orders and pages without replacing `list`. `load_by_ids` and `load_children` in [relations.md](relations.md) batch foreign keys and has-many rows on the same connection, including an open transaction ([transactions.md](transactions.md)). This is not a new version milestone. PR #59 is merged; the capability matrix rows for filtering, pagination, relations and transactions stay **In PR** until a later status review, because PostgreSQL runtime tests were not a local pass. Local workspace tests cover SQLite. PostgreSQL SQL generation is asserted. Annotations, keyset pagination, many-to-many and automatic eager loading are not part of this API.

## Decimal fields and typed choices

`Decimal` and `FerrioxidChoice` are described in [models.md](models.md). myshop migration `0006_prices_and_status` adds a precise product price, `ProductStatus`, and optional `Order.state`. This is not a new version milestone. The capability matrix stays **In PR** until the change is merged. Local tests cover SQLite. `postgres_decimal_and_choice_roundtrip` stays ignored without a server.

## Fixtures

`dumpdata` and `loaddata` are described in [fixtures.md](fixtures.md). myshop `fixtures/catalog.json` loads one category and two priced products. A second plain load does not create duplicates, and a conflicting file rolls back. This is not a new version milestone. The capability matrix stays **In PR** until the change is merged. `postgres_fixture_roundtrip` stays ignored without a server. Natural keys are not implemented.

## Management commands

Project commands live in the compiled `ferrioxid-manage` binary. `ferrioxid manage` only checks the command name and starts that binary with separate arguments. The contract is in [management-commands.md](management-commands.md). myshop's `seed-shop` upserts `fixtures/catalog.json` and can be run again without adding rows. This is not a new version milestone. The capability matrix stays **In PR** until the change is merged. The sample command was exercised on SQLite. It was not run against PostgreSQL locally.

## Open after v0.27.1

- Cron-Syntax.
- Soft Delete.
- PostgreSQL runtime against a running server.
- The CLI worker only executes the mail handler. Application handlers remain in the project binary.

## Open after v0.26

- Soft Delete, including queries, restores, audits and foreign keys.
- `merge` of a raw router does not see route names.
- The CLI does not load app Rust into its own process. `ferrioxid manage` starts the project's compiled binary. See [management-commands.md](management-commands.md).

## Open after v0.25

- `require_user` continues to authenticate with GET. Mutations use `require_mutation`.
- Since v0.26, the process start compares both lists. The CLI remains with `ferrioxid.toml`.
- `ferrioxid new` does not yet generate `src/apps/`.

## Open after v0.24

- `require_user` authenticates with GET and does not check CSRF.
- The API template continues to write `templates/index.html` because the start page remains active.

## Open after v0.23

- An assumed SMTP response does not prove the inbox. A process termination can devalue an already accepted token when cleaning up.
- `AccountDeactivated` and `SecurityNotice` do not depend on user care.
- The reset rate limit remains process-local.
- The JSON directory API does not require `confirm`.
- A non-superuser cannot distinguish a missing foreign key line from an invisible one.
- Filter selections and the form select show a maximum of 100 eligible targets. The selection page also scrolls.
- Cancel does not immediately delete a draft form. It expires after 30 minutes.
- The form fingerprint is not a version stable database value.
- Further child foreign keys are not resolved in the inline transaction.

## Planned phases

The order is binding for the next work. A later phase does not begin until the previous P0 removal is fulfilled.

| Phase |Priority| Contents |Depends on|
|---|---|---|---|
| v0.15 | Implemented |Design tokens, distances, form groups, selection instead of free text for roles| v0.14 |
| v0.16 | Implemented |Create users, change email and password, revoke sessions, confirmation and readable relations| v0.15 |
| v0.17 | Implemented |List links, standard sorting, page size, fieldsets, save destinations, form state, selection page| [admin-django-parity.md](admin-django-parity.md) |
| v0.18 | Implemented |Drafts, Has-Many-Inlines, Atomic bulk actions, Audit, `AdminAction`| [admin-bulk-actions.md](admin-bulk-actions.md) |
| v0.19 | Completed |Date, time, UTC time, date hierarchy| [date-time-types.md](date-time-types.md) |
| v0.20 | Completed |Transactions, directory audit, parent-child forms| [transactions.md](transactions.md) |
| v0.21 | Completed |Migration Diagnosis and Secure Project Manifest| [migration-diagnostics.md](migration-diagnostics.md) |
| v0.22 | Completed |Registered admin pages, actions, filters, widgets and templates| [admin-extension-api.md](admin-extension-api.md) |
| v0.23 | Completed |Email, Password Reset and Typed Notes| [password-reset.md](password-reset.md) |
| v0.24 | Completed |Settings, named URLs, views and project structure| [settings.md](settings.md) |

Not in these phases: React Return, OAuth, MFA, JWT, S3, Virus Scanner, Many-to-Many, `migrate down`, PostgreSQL Baseline, OpenAPI, automatic superuser semantics.

## Boundaries that remain

- Cargo version `0.1.0`. milestones are only in the changelog and in this roadmap.
- SQLite is the local standard. PostgreSQL tests remain ignored until a server is reachable.
- `0001_initial.json` and the auth migration `0001_auth_schema` are not rewritten.
- GET does not change data. Write accesses are POST with CSRF, except for the existing JSON API, which uses the header `x-csrf-token`.
- Password hashes, session tokens and CSRF tokens do not appear in HTML, URLs or logs.
- Skills are read before use. Impeccable has been read for v0.15, v0.16 and the form sections in v0.17. No skill, hook or agent commits or pushes on its own.

## Features for later phases

Tested on 2026-11-07: Impeccable is below `.cursor/skills/impeccable/` and has been read for v0.15. Context, Critique (two separate assessments), audit, layout and the polishing of the existing system “The Night Shift” were carried out. Nothing was reinstalled and nothing committed.

| Skill |Used only after testing|Do not do|
|---|---|---|
| Impeccable |Design criticism, audit, layout and fine-tuning of the native admin in v0.15|External installation without consent, decorative alterations without functional gain|
| Cloudflare Security Audit |Targeted testing of CSRF, XSS, sessions and file access before v0.16 and v0.18|Blind adoption of findings without comparison with the Ferrioxid code|
| ub-mode |Architectural boundaries and tests for directory and audit|New frameworks or a second CRUD engine|
| Ponytail |Complexity in `html.rs` and lower the metadata if a phase grows otherwise|Remove behaviour requiring removal|

## Implemented in v0.16

1. Users create, display, edit and password via their dedicated HTML routes and `AuthService`.
2. `confirm=1` is checked on the server side. Superusers and their own lock remain protected.
3. Foreign keys show a metadata name. Bool and foreign key filters are choices.
4. Generator child processes overwrite `DATABASE_URL` locally and clean up the test file.

## Implemented in v0.17

1. `list_link`, `admin_ordering` and `admin_per_page` control link, standard sorting and HTML page size.
2. `admin_fieldset` groups form fields. Three save buttons distinguish `save=detail`, `save=edit` and `save=new`.
3. Validation errors retain text, selection, and ticks. Passwords are not written back.
4. The foreign key selection page searches and scrolls in the reader right of the destination. An unreadable UUID is not stored.

## Implemented in v0.18

1. A session draft holds text, number, ticks, and selection while the foreign key page is open.
2. `admin_inline` shows linked lines and creates a child record with a fixed parent key.
3. Bulk actions are POST, confirmation and a transaction. A denied line does not change a line.
4. Model changes and their audit entry share a transaction. The history for Staff is limited to own events.

Acceptance and limits are in [admin-ux-backlog.md](admin-ux-backlog.md).
