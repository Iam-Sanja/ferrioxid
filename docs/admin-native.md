# Nativer Admin

[For overview](index.md) · Routes: [admin-routing.md](admin-routing.md)] Templates: [admin-templates.md](admin-templates.md)] Design: [admin-design-system.md](admin-design-system.md)]

**Stand:** v0.22. The cargo version remains `0.1.0`. Own pages are under `/admin/pages/{key}` and use the same layout. The interface is server-side HTML. React, Vite and Node.js are not needed for this. The login originals follow the bind address from `ferrum.toml` or `BIND_ADDRESS`. Form drafts, has-many inlines, the common parent-child form, collection actions, history and date hierarchy run without JavaScript. `admin.js` is `defer` and is not required for submission. Date fields use `type="date"`, `type="time"` and `type="datetime-local"` with UTC hint.

## Aufgabe

The admin shows registered models, forms and the user directory. The browser sends normal GET and POST forms. JavaScript in `admin.js` only focuses on the first invalid field. Lists, creation, modification, deletion, search, filtering, sorting, sheeting, login and logout work without a script, as far as the page shows them at all.

A non-Staff sees a 403 page without a sidebar and with an unsubscription form. The product decision remains: the standard admin will not be a React build again. Other Django functions are functional targets, no code compatibility: [admin-django-parity.md](admin-django-parity.md)].

## Listenspalten

`listed_fields` in `crates/ferrioxid-admin/src/html.rs` selects the columns:

1. If there is at least one field with `#[admin(list_display)]` except `id`, all `list_display` fields in the struct order apply. `id` is included because the macro always sets it as a list column.
2. Otherwise, the first visible text field, then `id`.
3. Otherwise, the first other visible field, then `id`.
4. Otherwise only `id`.

`hidden` and sensitive names (`password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`) are deleted. A model does not have to have a field `name`. Write protected fields remain visible. Except for `id`, they are marked “Read Only”. Sort links are only available for `sortable` and for `id`.

## Aufbau

```text
Browser
  → Axum-Route unter /admin
  → Sitzung, Staff, CSRF, Modellrecht, Query-Scope
  → AdminRegistry und ManagedModel
  → Tera
  → HTML
```

The JSON API remains under `/admin/api`. It uses the same models, rights and scopes. HTML routes do not replace or overwrite them.

## Responsible files

| Bereich |File|
|---|---|
|Routes and forms| `crates/ferrioxid-admin/src/html.rs` |
|JSON API and joint audits| `crates/ferrioxid-admin/src/http.rs` |
| Vorlagen | `crates/ferrioxid-admin/src/rendering.rs`, `crates/ferrioxid-admin/templates/` |
| Texte | `crates/ferrioxid-admin/src/localization.rs` |
|CSS and Script| `crates/ferrioxid-admin/static/css/admin.css`, `crates/ferrioxid-admin/static/js/admin.js` |
| Auslieferung | `crates/ferrioxid-admin/src/assets.rs` |
| Modelle | `crates/ferrioxid-admin/src/registry.rs` |

`build.rs` does not call npm. CSS, script and templates are in the binary via `include_str!`. The cargo feature `admin-ui` remains as an empty compatibility flag. It doesn’t require a frontend build anymore.

## Branding

`AdminBranding` reads `ferrum.toml`. Title, header and application name must not contain HTML. `primary_color` is `#rgb` or `#rrggbb`. Languages are `de` and `en`. The currency is a code of three capital letters. `--brand` sets the HTML area only if white font on it reaches at least 4.5:1. Otherwise, `#0f172a` remains. The dark door does not use `--brand`. Tera escaped the value. The `safe` filter is not used for database content. The JSON API passes on the configured color.

## Language and presentation

`ferrioxid_locale` and `ferrioxid_theme` are HttpOnly cookies. They are created via `POST /admin/preferences`. Allowed languages are `de` and `en`. Allowed themes are `light`, `dark` and `system`. `system` follows `prefers-color-scheme`. Without a cookie, the project language and `system` apply. `next` must begin with `/admin` and must not contain a schema or host.

## Grenzen

There are no invented key figures on the overview. Foreign key selections load a maximum of 100 visible lines. The HTML list shows 25 lines. Password hashes and sensitive field names are not issued. A record outside the object rule is 404, not 403.

## Foreign keys and filters

`display_column` selects the display text of a target model: the first visible text field with `list_display`, otherwise a searchable text field, otherwise the first other visible text field. `id`, hidden and sensitive columns are eliminated. A model does not need a field `name`.

Lists and details resolve the IDs of the current lines with `query_labels`. For each destination table and page, this is a query of 100 IDs, with bound parameters and the query scope of the destination. If the target model, the reading permission or a text field is missing, no name appears. An empty reference is an indent. A superuser sees an unknown ID “Not present”. Everyone else sees “Not visible” without a second query checking if the line exists.

Bool filters are `Alle`, `Ja` and `Nein`. Foreign key filters are the same limited, authorized selection. Number filters are `type="number"`. Other types remain a text field. The filter parser remains server-side. Sorting is preserved as a hidden field. A new filtering sets the page back.
