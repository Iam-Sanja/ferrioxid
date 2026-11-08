# Ferrioxide Roadmap – Status and Next Phases

[For overview](index.md) · Admin comparison: [admin-django-parity.md](admin-django-parity.md) · UX backlog: [admin-ux-backlog.md](admin-ux-backlog.md)]

** Status:** Function status v0.27.1 supplements Application Worker. Functional status v0.27 complements persistent jobs. Functional status v0.26 complements model fields and the tested app launch. Function status v0.25 remains valid. Function status v0.24 remains valid. Function status v0.23 remains valid. The cargo version remains `0.1.0`. This text separates implemented milestones from planned phases. A planned phase is not a release and not a finished feature.

## Produktentscheidung

The default admin remains server-side HTML with Rust, Axum and Tera. React, Vite and Node.js are not standard admins. Core actions, i.e. lists, forms, deletion, login, log-out and directory, must work without JavaScript. The optional `admin.js` must only offer comfort, for example focus.

In the long term, the admin should cover the relevant functions of the Django admin in a functionally equivalent manner. The internal API does not have to be django compatible. The surface should become a dense working environment, not a decorative redesign.

## Implemented milestones

| Meilenstein | Status | Ziel | Grenze |
|---|---|---|---|
| v0.1 |Implemented, SQLite tested in the workspace| Typed Model, Liste, GET, POST, PATCH, DELETE, sichere Migrationen | PostgreSQL-HTTP-Test bleibt ignoriert, solange kein Server läuft |
| v0.6 | Implementiert | Unique, Indizes, Foreign Keys, Belongs-To, Has-Many, Snapshot und Diff |Many-to-Many, Eager Loading, `down` and PostgreSQL baseline missing|
| v0.7 | Implementiert |Argon2id, sessions, cookies, CSRF, RBAC, model rights|OAuth, MFA and JWT are missing. Password reset follows in v0.23|
| v0.8 bis v0.12 | Implementiert | Admin-Metadaten, JSON-API, Query-Scopes, Generator, Settings, Formulare, Staticfiles, Uploads, Middleware |Historic React surface has been removed since v0.13|
| v0.13 | Implementiert |Native Admin under `/admin`, Tera, CSS in Binary, JSON API remains|Cargo version remains `0.1.0`|
| v0.13.1 | Implementiert |Login originals from the bind address, `Origin: null` only with `Sec-Fetch-Site: same-origin`|Host header is not read as Allowlist|
| v0.14 | Implementiert |Logging out on authenticated 403 pages, flag notices, standard columns|Users are still created v0.16|
| v0.15 | Implementiert |Design tokens, field groups, table rhythm, role selection, contrast gate|Cargo version remains `0.1.0`|
| v0.16 | Implementiert |User maintenance, server-side confirmation, readable foreign keys, filter selection, isolated generator databases|Superuser remains CLI. No user hard delete. JSON API without `confirm`|
| v0.17 | Implementiert |List configuration, fieldsets, three storage destinations, form state, foreign key selection page|No date type. Cargo version remains `0.1.0`|
| v0.18 | Implementiert |Draft forms, has-many inlines, collection actions, audit, registered actions|No common parent-child storage. Directory audit is not the same transaction|
| v0.19 | Abgeschlossen |`Date`, `Time`, `DateTime<Utc>`, Migration `0004_events`, admin fields, UTC date hierarchy|PostgreSQL runtime untested. No tacit text-to-date conversion|
| v0.20 | Abgeschlossen |Shared transaction for model, authentic and audit. Directory audit. Opt-in parent-child form|Not many-to-many. No framework-wide optimistic locking. PostgreSQL runtime untested|
| v0.21 | Abgeschlossen |Reading Auth and App Diagnosis. `check --strict`. Upgrading only creates a missing legacy manifesto|No automatic template replacement. No silent migrations. PostgreSQL runtime untested|
| v0.21.1 | Abgeschlossen |Generator Test limits nested release builds to two jobs. Unrestricted workspace test after green|rustc crash `0xc0000005` and `0xc0000409` is not solved causally|
| v0.22 | Abgeschlossen |Public admin extension: Pages, actions, filters, widgets, templates|No second admin engine. PostgreSQL runtime untested. The rustc crash remains unexplained|
| v0.23 | Abgeschlossen |Email service, self-service password reset, typed notes|No delivery guarantee. No admin notification center. PostgreSQL runtime untested|
| v0.24 | Abgeschlossen | Typisierte Settings, `[custom.*]`, `UrlSet`, View-Helfer, Generatorstruktur |`require_user` does not check CSRF. API templates retain HTML homepages. PostgreSQL runtime untested|
| v0.25 | Abgeschlossen |`App::install`, App Migrations, Tera-`url`, App Templates and Static Files, `ferrioxid generate app`|`merge` of a raw router does not see names. PostgreSQL runtime untested|
| v0.27 | Abgeschlossen |Persistent queue, lease, limited retries, worker, ordinary mail jobs|Cron is missing. Reset tokens remain outside the queue. PostgreSQL runtime untested|
| v0.27.1 | Abgeschlossen | `WorkerHost`, `ferrioxid generate worker`, App-Handler, verdeckte Job-Payload |CLI-Worker does not load application code. Exactly once is missing. PostgreSQL runtime untested|
| v0.26 | Abgeschlossen |Tested app launch, comparison of rust and TOML migrations, flat field groups, managed timestamps|Soft Delete is missing. PostgreSQL runtime untested|

## Offen nach v0.27.1

- Cron-Syntax.
- Soft Delete.
- PostgreSQL runtime against a running server.
- The CLI worker only executes the mail handler. Application handlers remain in the project binary.

## Offen nach v0.26

- Soft Delete, including queries, restores, audits and foreign keys.
- `merge` of a raw router does not see route names.
- The CLI does not execute app code. The adjustment is made by the process start.

## Offen nach v0.25

- `views::fail` uses its own HTML page and not `templates/errors/`.
- `require_user` continues to authenticate with GET. Mutations use `require_mutation`.
- Since v0.26, the process start compares both lists. The CLI remains with `ferrum.toml`.
- `ferrioxid new` does not yet generate `src/apps/`.

## Offen nach v0.24

- `views::fail` uses its own HTML page and not `templates/errors/`.
- `require_user` authenticates with GET and does not check CSRF.
- The API template continues to write `templates/index.html` because the start page remains active.

## Offen nach v0.23

- An assumed SMTP response does not prove the inbox. A process termination can devalue an already accepted token when cleaning up.
- `AccountDeactivated` and `SecurityNotice` do not depend on user care.
- The reset rate limit remains process-local.
- The JSON directory API does not require `confirm`.
- A non-superuser cannot distinguish a missing foreign key line from an invisible one.
- Filter selections and the form select show a maximum of 100 eligible targets. The selection page also scrolls.
- Cancel does not immediately delete a draft form. It expires after 30 minutes.
- The form fingerprint is not a version stable database value.
- Further child foreign keys are not resolved in the inline transaction.

## Geplante Phasen

The order is binding for the next work. A later phase does not begin until the previous P0 removal is fulfilled.

| Phase |Priority| Inhalt |Depends on|
|---|---|---|---|
| v0.15 | umgesetzt |Design tokens, distances, form groups, selection instead of free text for roles| v0.14 |
| v0.16 | umgesetzt |Create users, change email and password, revoke sessions, confirmation and readable relations| v0.15 |
| v0.17 | umgesetzt |List links, standard sorting, page size, fieldsets, storage destinations, form state, selection page| [admin-django-parity.md](admin-django-parity.md) |
| v0.18 | umgesetzt |Drafts, Has-Many-Inlines, Nuclear Collective Actions, Audit, `AdminAction`| [admin-bulk-actions.md](admin-bulk-actions.md) |
| v0.19 | abgeschlossen |Date, time, UTC time, date hierarchy| [date-time-types.md](date-time-types.md) |
| v0.20 | abgeschlossen |Transactions, directory audit, parent-child forms| [transactions.md](transactions.md) |
| v0.21 | abgeschlossen |Migration Diagnosis and Secure Project Manifesto| [migration-diagnostics.md](migration-diagnostics.md) |
| v0.22 | abgeschlossen |Registered admin pages, actions, filters, widgets and templates| [admin-extension-api.md](admin-extension-api.md) |
| v0.23 | abgeschlossen |Email, Password Reset and Typed Notes| [password-reset.md](password-reset.md) |
| v0.24 | abgeschlossen |Settings, named URLs, views and project structure| [settings.md](settings.md) |

Not in these phases: React Return, OAuth, MFA, JWT, S3, Virus Scanner, Many-to-Many, `migrate down`, PostgreSQL Baseline, OpenAPI, automatic superuser semantics.

## Boundaries that remain

- Cargo version `0.1.0`. Functional statuses are only in the changelog and in this roadmap.
- SQLite is the local standard. PostgreSQL tests remain ignored until a server is reachable.
- `0001_initial.json` and the auth migration `0001_auth_schema` are not rewritten.
- GET does not change data. Write accesses are POST with CSRF, except for the existing JSON API, which uses the header `x-csrf-token`.
- Password hashes, session tokens and CSRF tokens do not appear in HTML, URLs or logs.
- Skills are read before use. Impeccable has been read for v0.15, v0.16 and the form sections in v0.17. No skill, hook or agent commits or pushes on its own.

## Skills for later phases

Tested on 2026-11-07: Impeccable is below `.cursor/skills/impeccable/` and has been read for v0.15. Context, Critique (two separate assessments), audit, layout and the polishing of the existing system “The Night Shift” were carried out. Nothing was reinstalled and nothing committed.

| Skill |Used only after testing|Do not do|
|---|---|---|
| Impeccable |Design criticism, audit, layout and fine-tuning of the native admin in v0.15|External installation without consent, decorative alterations without functional gain|
| Cloudflare Security Audit |Targeted testing of CSRF, XSS, sessions and file access before v0.16 and v0.18|Blind adoption of findings without comparison with the ferrioxide code|
| ub-mode |Architectural boundaries and tests for directory and audit|New frameworks or a second CRUD engine|
| Ponytail |Complexity in `html.rs` and lower the metadata if a phase grows otherwise|Remove behaviour requiring removal|

## Umgesetzt in v0.16

1. Users create, display, edit and password via their own HTML routes and `AuthService`.
2. `confirm=1` is checked on the server side. Superusers and their own lock remain protected.
3. Foreign keys show a metadata name. Bool and foreign key filters are choices.
4. Generator child processes overwrite `DATABASE_URL` locally and clean up the test file.

## Umgesetzt in v0.17

1. `list_link`, `admin_ordering` and `admin_per_page` control link, standard sorting and HTML page size.
2. `admin_fieldset` groups form fields. Three memory buttons distinguish `save=detail`, `save=edit` and `save=new`.
3. Validation errors retain text, selection, and ticks. Passwords are not written back.
4. The foreign key selection page searches and scrolls in the reader right of the destination. An unreadable UUID is not stored.

## Umgesetzt in v0.18

1. A session draft holds text, number, ticks, and selection while the foreign key page is open.
2. `admin_inline` shows linked lines and creates a child record with a fixed parent key.
3. Collective actions are POST, confirmation and a transaction. A denied line does not change a line.
4. Model changes and their audit entry share a transaction. The history for Staff is limited to own events.

Acceptance and limits are in [admin-ux-backlog.md](admin-ux-backlog.md)].
