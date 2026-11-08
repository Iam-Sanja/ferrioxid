# Admin navigation

The sidebar is rendered by `crates/ferrum-admin/src/nav.rs` from the sealed admin registry and the signed-in user.

## Sections

1. Dashboard links to `/admin`.
2. Applications lists every registered app that has at least one visible model or custom admin page. Models registered with `AdminRegistry::add` and no app are grouped under `branding.app_name`.
3. Users and access is present only for an active staff superuser. It links to users and roles. Role permissions stay on `/admin/directory/permissions` and highlight Roles.
4. System links to the audit log. There is no settings entry, because the admin has no settings page. Language and theme stay in the header.

An empty application section says that no models are available. Shop, Notes, and other example names are not hardcoded.

## How an app is attached

`Registration::admin_model` and `Registration::admin_view` call `AdminRegistry::with_app` with the app name and `AppConfig::display_label` for **each** registration. An app scope applies only to the next `add`, `add_policy`, or `view` operation. To register another model or page in the same app, call `with_app` again. Registrations without an explicit scope appear under `branding.app_name`, and invalid scopes do not retain a previous app.

A model link points to `/admin/models/{key}`. A custom page points to `/admin/pages/{key}` and appears only when `navigation(true)` is set and `AuthService::holds` allows its permission. Models are included only when list access is allowed and the model is not blocked for a missing object policy.

The current item matches the path exactly or as a prefix with a slash. Editing a record keeps the model entry current.

## Audit

`GET /admin/audit` is the global log. Staff may open it. A superuser sees every row. Other staff see only their own rows. That rule is unchanged.

`?model=` and `?record=` filter rows that already store `model_key` and `record_id`. The values are limited to 80 ASCII letters, digits, `_`, `-`, and `.`. Invalid filters are ignored. Record pages and user pages link here when that correlation exists.

Directory permission changes are stored with model key `_ferrum_users` and the role name as `record_id`. Role history uses that existing pair. It does not invent events.

## Time

`created_at` stays a Unix second in `_ferrum_admin_audit`. The admin renders it as `YYYY-MM-DD HH:MM:SS UTC` inside `<time datetime="...Z">`. The zone is UTC, not the browser zone. A value that cannot be represented is labeled unavailable. The first paint does not depend on JavaScript.

## Unsaved changes

Existing record forms and the header language/theme form carry `data-track-dirty`. `admin.js` disables their save or apply buttons until the submitted field set differs from the initial snapshot. Confirmation checkboxes and the CSRF token do not count. A validation error inside the record form keeps save available. Reverting every counted field disables save again. Leaving the page with a dirty form raises the browser warning. A valid native form submission suppresses that warning for the submitted form; a failed validity check or an unrelated unsaved form does not. Server-side validation failures render their errors and keep the save action available.

New records, password forms, and inline "add row" stay available immediately. Without JavaScript, save and apply stay enabled, so those core actions still work.

## User deletion

The admin can deactivate a user. It does not delete the user row, sessions, or audit history. A physical delete needs a separate persistence design and is not part of v0.28.

## Validation

The Admin 2.0 change is tested with `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace` on both Linux and Windows in GitHub Actions. The HTML integration tests are part of the workspace test run. A separate PostgreSQL CI job runs the explicitly ignored integration tests against a disposable PostgreSQL instance. Do not interpret a workflow setup error or a skipped step as a passing or failing Rust test. The security audit is a separate workflow and is tracked independently.
