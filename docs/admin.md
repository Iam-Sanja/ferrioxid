# Ferrioxid Admin

[Overview](index.md) · Backend: [admin-backend.md](admin-backend.md) · Interface: [admin-frontend.md](admin-frontend.md) · Security: [admin-security.md](admin-security.md)

**Milestone:** v0.22. The Cargo version remains `0.1.0`. Extension: [admin-extension-api.md](admin-extension-api.md). Transactions: [transactions.md](transactions.md). Date hierarchy: [admin-date-hierarchy.md](admin-date-hierarchy.md). Types: [date-time-types.md](date-time-types.md). Admin metadata does not change the database schema. The `0002_admin_journal` auth migration creates drafts and audits. Create uses the same `can_create` rule as REST. Filter lists in SQL. The directory is reserved for staff superusers. Branding comes from `ferrioxid.toml`. Details are available in [admin-native.md](admin-native.md). Lists: [admin-list-configuration.md](admin-list-configuration.md). Sections: [admin-fieldsets.md](admin-fieldsets.md). Selection: [admin-relational-widgets.md](admin-relational-widgets.md). Drafts: [admin-form-drafts.md](admin-form-drafts.md). Inlines: [admin-inlines.md](admin-inlines.md). Bulk actions: [admin-bulk-actions.md](admin-bulk-actions.md). History: [admin-audit-log.md](admin-audit-log.md). Custom actions: [admin-custom-actions.md](admin-custom-actions.md).

A 403 page for an authenticated user includes a logout form. Lists without explicit `list_display` settings use metadata-based fallback columns. See [admin-native.md](admin-native.md) for current behavior and [roadmap.md](roadmap.md) for remaining work.

## Purpose

A model with `admin = true` provides navigation, table, form, detail and deletion. Tera renders this information. Authorization remains on the server.

## Responsible files

| Area | Files |
|---|---|
| Metadata | `crates/ferrioxid-core/src/lib.rs`, `crates/ferrioxid-macros/src/lib.rs` |
| Admin-API | `crates/ferrioxid-admin/src/` |
| List queries | `crates/ferrioxid-db/src/sql.rs` (`query_admin_page`) |
| Interface | `crates/ferrioxid-admin/templates/`, `crates/ferrioxid-admin/static/` |
|Example| `examples/myshop/src/models/`, `examples/myshop/src/main.rs` |
| Test | `examples/myshop/tests/admin_http.rs`, `examples/myshop/tests/admin_html.rs`, `examples/myshop/tests/admin_extensions.rs` |

## Data flow

Rust struct → `FerrioxidModel` generates `AdminModelMeta` alongside `ModelDescriptor` → `AdminRegistry::add` → HTML under `/admin/models/{modell}` and JSON under `/admin/api`. Writes pass session, staff, model-permission, CSRF and repository-validation checks.

## Configuration

`App::with_admin(registry)` attaches HTML and JSON. Templates and CSS are in the binary. Without configured authentication, `GET /admin` responds with 403. The sequence is in [admin-routing.md](admin-routing.md).

## Public use

```rust
let admin = ferrioxid::admin::AdminRegistry::new()
    .add::<Category>()
    .add::<Product>()
    .add::<InternalNote>();
App::new(db).with_auth(auth).with_admin(admin)
```

The surface is below `http://127.0.0.1:3000/admin`. The API is under `/admin/api`.

## Typed configuration

`AdminModelMeta` from `#[ferrioxid(admin = true)]` is the model admin declaration. `ModelAdmin` narrows that metadata. It does not start a second admin. List columns, search, filters, ordering, readonly, hidden and sensitive fields, fieldsets, inlines and the permission stem stay on `AdminModelMeta`. Compile-time checks reject a list link without list display, an unknown fieldset, duplicate fieldset names, a visible field without a fieldset when fieldsets exist, and a password, token, secret or sensitive field that is not hidden. Runtime paths still check the session, staff access, `{permission}.read|create|update|delete`, the object query scope and CSRF on writes.

Call `ModelAdmin::check` with the model metadata and columns, then `AdminRegistry::configure`, then `seal`. Until `seal`, the derive metadata stays in effect. A configuration can reduce list, search and filter fields, mark more fields readonly or hidden, replace fieldsets, set ordering and page size, and require an extra `resource.action` code through `holds`. It cannot reveal a sensitive or hidden field. Foreign keys of a configured model must point at a table that is also in the registry. Inlines stay on the model derive. Has-many editing remains `save_with_children`. Many-to-many membership uses `replace_links` inside one transaction. The HTML picker pages past 100 rows; the select shows the first 100 and links to that search. One-to-one batch loading is `load_one_to_one`. Unique foreign keys can still use the existing picker.

```rust
let notes = ModelAdmin::new("notes")
    .list_display(&["title"])
    .search_fields(&["title"])
    .readonly(&["body"])
    .action_permission("update", "notes.publish")
    .check(Note::admin_meta().unwrap(), Note::columns())?;
let registry = AdminRegistry::new().add::<Note>().configure(notes)?.seal()?;
```

See [admin-list-configuration.md](admin-list-configuration.md), [admin-fieldsets.md](admin-fieldsets.md), [admin-inlines.md](admin-inlines.md), [admin-relational-widgets.md](admin-relational-widgets.md) and [admin-devex.md](admin-devex.md).

## Security

Admin model and directory requests require a valid session and `is_staff = true`; public configuration and login routes are exceptions. `allow_anonymous` never grants admin access. Superusers still need staff access. Models with no admin permission are available only to superusers. Bound object policies filter lists through SQL query scopes and check individual records.

## Errors

| Situation | Response |
|---|---|
| No session, HTML | 303 redirect to login |
| No session, JSON | 401 |
|No staff or lack of model permissions| 403 |
|Unknown model, wrong sorting, blocked field|404 or 422|
|Object rule rejects a record| 404 |

## Tests

`examples/myshop/tests/admin_http.rs` covers the JSON API. `examples/myshop/tests/admin_html.rs` covers login, lists, search, filters, sorting, pagination, CRUD forms, user management, confirmation, relation labels, CSRF and HTML escaping.

## Limitations

General plugin hooks, OAuth, MFA and multi-tenancy are not implemented. The admin supports German and English. It cannot grant superuser access or physically delete users. Directory management requires a staff superuser and uses dedicated create, edit and password routes rather than generic model CRUD. Deactivation is the supported alternative to deletion.

## Later

SQL object scopes, embedded assets and custom widgets are already implemented. Remaining work includes finer directory permissions and the limitations described in [admin-django-parity.md](admin-django-parity.md).
