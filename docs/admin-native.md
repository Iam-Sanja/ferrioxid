# Native admin

[Overview](index.md) · Routes: [admin-routing.md](admin-routing.md) Templates: [admin-templates.md](admin-templates.md) Design: [admin-design-system.md](admin-design-system.md)

**Milestone:** v0.22. The Cargo version remains `0.1.0`. Custom pages under `/admin/pages/{key}` share the native server-rendered layout. React, Vite and Node.js are not required. Allowed login origins follow the bind address from `ferrum.toml` or `BIND_ADDRESS`. Drafts, has-many inlines, opt-in parent-child forms, bulk actions, history and date hierarchy work without JavaScript. `admin.js` uses `defer` and is not required for submission. Date fields use `type="date"`, `type="time"` and `type="datetime-local"` with a UTC hint.

## Purpose

The admin displays registered models, record forms and the user directory through normal GET and POST requests. Optional `admin.js` focuses the first invalid field. Lists, record creation, editing, deletion, search, filtering, sorting, pagination, login and logout work without JavaScript.

An authenticated user without staff status receives a 403 page with a logout form and no sidebar. The default admin uses server-rendered HTML. Django parity describes functional goals rather than code compatibility; see [admin-django-parity.md](admin-django-parity.md).

## List columns

`listed_fields` in `crates/ferrioxid-admin/src/html.rs` selects the columns:

1. If there is at least one field with `#[admin(list_display)]` except `id`, all `list_display` fields in the struct order apply. `id` is included because the macro always sets it as a list column.
2. Otherwise, the first visible text field, then `id`.
3. Otherwise, the first other visible field, then `id`.
4. Otherwise only `id`.

`hidden` and sensitive names (`password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`) are deleted. A model does not have to have a field `name`. Write protected fields remain visible. Except for `id`, they are marked “Read Only”. Sort links are only available for `sortable` and for `id`.

## Structure

```text
Browser
  → Axum-Route unter /admin
  → Sitzung, Staff, CSRF, Modellrecht, Query-Scope
  → AdminRegistry und ManagedModel
  → Tera
  → HTML
```

The JSON API remains under `/admin/api`. It uses the same models, permissions and scopes. HTML routes do not replace or overwrite them.

## Responsible files

| Area |File|
|---|---|
|Routes and forms| `crates/ferrioxid-admin/src/html.rs` |
|JSON API and joint audits| `crates/ferrioxid-admin/src/http.rs` |
| Templates | `crates/ferrioxid-admin/src/rendering.rs`, `crates/ferrioxid-admin/templates/` |
| Text and localization | `crates/ferrioxid-admin/src/localization.rs` |
|CSS and Script| `crates/ferrioxid-admin/static/css/admin.css`, `crates/ferrioxid-admin/static/js/admin.js` |
| Asset delivery | `crates/ferrioxid-admin/src/assets.rs` |
| Models | `crates/ferrioxid-admin/src/registry.rs` |

`build.rs` does not call npm. CSS, script and templates are in the binary via `include_str!`. The cargo feature `admin-ui` remains as an empty compatibility flag. It doesn’t require a frontend build anymore.

## Branding

`AdminBranding` reads `ferrum.toml`. Titles, headers and application names must not contain HTML. `primary_color` accepts `#rgb` or `#rrggbb`; languages are `de` and `en`, and currency is a three-uppercase-letter code. `--brand` colors the light sidebar only if white text reaches at least 4.5:1 contrast; otherwise it remains `#0f172a`. The dark sidebar does not use `--brand`. Tera escapes values, and database content never uses the `safe` filter. The JSON API returns the configured color.

## Language and presentation

`ferrioxid_locale` and `ferrioxid_theme` are HttpOnly cookies set by `POST /admin/preferences`. Languages are `de` and `en`; themes are `light`, `dark` and `system`. `system` follows `prefers-color-scheme`. With no cookie, the project language and `system` apply. `next` must begin with `/admin` and contain neither a URL scheme nor a host.

## Limitations

There are no invented key figures on the overview. Foreign key selections load a maximum of 100 visible lines. The HTML list shows 25 lines. Password hashes and sensitive field names are not issued. A record outside the object rule is 404, not 403.

## Foreign keys and filters

`display_column` selects the display text of a target model: the first visible text field with `list_display`, otherwise a searchable text field, otherwise the first other visible text field. `id`, hidden and sensitive columns are eliminated. A model does not need a field `name`.

Lists and detail pages resolve record labels with `query_labels`, using one parameterized query per target table and page for up to 100 IDs under the target’s query scope. Missing target registration, read permission or a text display field prevents label resolution. Empty references display a dash. Superusers see missing targets as “Not present”; other users see “Not visible” without an existence-check query.

Bool filters are `Alle`, `Ja` and `Nein`. Foreign key filters are the same limited, authorized selection. Number filters are `type="number"`. Other types remain a text field. The filter parser remains server-side. Sorting is preserved as a hidden field. A new filtering sets the page back.
