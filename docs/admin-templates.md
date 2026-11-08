# Admin templates

[Overview](admin-native.md) · Design: [admin-design-system.md](admin-design-system.md)

## Purpose

Tera renders the admin from fixed templates embedded in the crate. Requests cannot choose a template name or file path. Unknown templates fail on the server and return status 500 with the short text `Request rejected`. Detailed Tera errors go to the log, not the browser.

## Files

| Template | Page |
|---|---|
| `templates/base.html` | Document layout, sidebar, language, theme and logout |
| `templates/login.html` | Login |
| `templates/dashboard.html` |Summary|
| `templates/models/list.html` | List, search, filters, sorting, pagination and bulk actions |
| `templates/models/bulk.html` | Bulk-action confirmation, registered as `bulk.html` |
| `templates/audit.html` |Change history|
| `templates/models/form.html` | Create/edit form, fieldsets and three save buttons |
| `templates/models/pick.html` |Foreign key selection, registered as `pick.html`|
| `templates/partials/field.html` | Field group, registered as `field.html` |
| `templates/models/detail.html` | Detail |
| `templates/models/delete.html` |Deletion confirmation|
| `templates/directory/users.html` | User list |
| `templates/directory/user_form.html` |Create and edit users, password|
| `templates/directory/user_detail.html` | User detail |
| `templates/directory/roles.html` | Roles |
| `templates/directory/permissions.html` | Permissions |
| `templates/errors/error.html` |403, 404, 422 and 429|

`rendering.rs` registers these files under fixed names. `base.html` is the parent template. Subpages fill the block `content`.

## Context

Every page receives `locale`, `theme`, `title`, branding, the `i18n` catalog, `signed_in`, `superuser`, `user_email`, `csrf_token`, `current_path`, `notice` and `alert`. Handlers add page-specific fields such as `sections`, `rows` or `models`. Model forms group fields in `sections`. User forms provide `email`, `email_error`, `password_error` and `role_checks`. Password fields are never prefilled.

Messages after a POST do not appear as free text in the URL. The codes `created`, `saved`, `deleted`, `updated` and `changed` are permitted. `changed` additionally requires a number `changed` from 0 to 100. The handler translates the code.

## Escaping

Tera automatically escapes `.html` templates. Database values, error messages and catalog text use this escaping. `primary_color` is validated as a hex color before rendering. Record fields never use `|safe`.

Boolean controls submit a hidden `false` followed by the checkbox value `true` when checked. The parser retains the last value, so checked controls produce `true` and unchecked controls produce `false`.

## Form fields

The handler builds fields from `AdminFieldMeta` and `ColumnSpec`. Hidden and sensitive do not appear. Read only and `id` appear as text and are not sent. Foreign keys become `select` if the target model is registered, the list is not blocked and the user is allowed to read. The current ID is `selected`. Mandatory foreign keys do not have an empty option. A validation error sets `aria-invalid` and a text under the field.

The role assignment sends `email` and `role` from selection lists. The handler assigns only one role that `list_roles` already knows.

## Known gaps

`errors/error.html` shows a POST logout form when a session exists and links to login otherwise. `directory/users.html` assigns roles through a `<select>`. The toolbar separates search and filters from “New”. Field groups live in `partials/field.html`. Built-in template names remain fixed, and requests cannot choose paths. Projects can register additional templates as described in [admin-extension-api.md](admin-extension-api.md). They extend `base.html`, cannot replace it and must not use `|safe`.
