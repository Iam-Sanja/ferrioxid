# Ferrioxid Admin

[For overview](index.md) · Backend: [admin-baking.md](admin-backend.md) · Surface: [admin-frontend.md](admin-frontend.md)] Security: [admin-security.md](admin-security.md)]

**Stand:** v0.22. The cargo version remains `0.1.0`. Extension: [admin-extension-api.md](admin-extension-api.md). Transactions: [transactions.md](transactions.md)]. Date hierarchy: [admin-date-hierarchy.md](admin-date-hierarchy.md). Types: [date-time-types.md](date-time-types.md).] Admin metadata does not change the database schema. The `0002_admin_journal` authentic migration creates drafts and audits. Create uses the same `can_create` rule as REST. Filter lists in SQL. The directory is reserved for staff superusers. Branding comes from `ferrum.toml`. Details are available in [admin-native.md](admin-native.md). Lists: [admin-list-configuration.md](admin-list-configuration.md).] Sections: [admin-fieldsets.md](admin-fieldsets.md)]. Selection: [admin-relational-widgets.md](admin-relational-widgets.md). Drafts: [admin-form-drafts.md](admin-form-drafts.md). Inlines: [admin-inlines.md](admin-inlines.md). Collective actions: [admin-bulk-actions.md](admin-bulk-actions.md). History: [admin-audit-log.md](admin-audit-log.md).] Own actions: [admin-custom-actions.md](admin-custom-actions.md)].

A registered 403 page contains a deregistration form. Lists without further ado `list_display` select a visible field and the ID. The area and the role selection are implemented in v0.15. The phases v0.16 to v0.18 are in [roadmap.md](roadmap.md)].

## Zweck

A model with `admin = true` provides navigation, table, form, detail and deletion. Tera renders this information. Authorization remains on the server.

## Responsible files

| Bereich | Dateien |
|---|---|
| Metadaten | `crates/ferrioxid-core/src/lib.rs`, `crates/ferrioxid-macros/src/lib.rs` |
| Admin-API | `crates/ferrioxid-admin/src/` |
| Listenabfrage | `crates/ferrioxid-db/src/sql.rs` (`query_admin_page`) |
|Surface surface| `crates/ferrioxid-admin/templates/`, `crates/ferrioxid-admin/static/` |
|Example| `examples/myshop/src/models/`, `examples/myshop/src/main.rs` |
| Test | `examples/myshop/tests/admin_http.rs`, `examples/myshop/tests/admin_html.rs`, `examples/myshop/tests/admin_extensions.rs` |

## Datenfluss

Rust-Struct → `FerrioxidModel` generates `AdminModelMeta` alongside the existing `ModelDescriptor` → `AdminRegistry::add` → HTML under `/admin/models/{modell}` and JSON under `/admin/api`. Writing calls run through session, staff exam, model law, CSRF and existing repository validation.

## Konfiguration

`App::with_admin(registry)` attaches HTML and JSON. Templates and CSS are in the binary. Without configured authentication, `GET /admin` responds with 403. The sequence is in [admin-routing.md](admin-routing.md)].

## Public use

```rust
let admin = ferrioxid::admin::AdminRegistry::new()
    .add::<Category>()
    .add::<Product>()
    .add::<InternalNote>();
App::new(db).with_auth(auth).with_admin(admin)
```

The surface is below `http://127.0.0.1:3000/admin`. The API is under `/admin/api`.

## Sicherheit

Each admin request needs a valid session and `is_staff = true`. `allow_anonymous` does not open the admin. A superuser without a staff flag does not come in. Model rights remain necessary, unless the admin authorization is empty: then only one superuser may operate the model. Object policy lists remain blocked.

## Fehler

| Lage | Antwort |
|---|---|
| Keine Sitzung, HTML | 303 zur Anmeldung |
| Keine Sitzung, JSON | 401 |
|No staff or lack of model rights| 403 |
|Unknown model, wrong sorting, blocked field|404 or 422|
|Object rule rejects a record| 404 |

## Tests

`examples/myshop/tests/admin_http.rs` checks the JSON API. `examples/myshop/tests/admin_html.rs` checks login, list, search, filter, sort, scroll, create, modify, delete, directory, user care, confirmation, relational name, CSRF and escaping via HTML forms.

## Grenzen

No plugin system, no OAuth, no MFA, no client system. Languages are German and English. Superuser flags cannot be set in the admin. User management is reserved for superusers and not a generic model CRUD. Creating, editing and password are their own directory routes. Deleting a user is deactivating.

## Later

SQL scopes for object rules, embedded assets, own field widgets and finer controllable directory rights.
