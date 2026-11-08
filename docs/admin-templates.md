# Admin-Vorlagen

[For overview](admin-native.md) · Design: [admin-design-system.md](admin-design-system.md)]

## Aufgabe

Tera renders the admin from fixed templates in the Crate. A request cannot choose a file path and a template name. Unknown names fail on the server side. The answer is then a short text `Request rejected` with status 500. The error text from Tera goes into the log, not into the browser.

## Dateien

| Vorlage | Seite |
|---|---|
| `templates/base.html` |Document, sidebar, language, theme, unsubscribe|
| `templates/login.html` | Anmelden |
| `templates/dashboard.html` |Summary|
| `templates/models/list.html` |List, search, filter, sorting, sheets, collection action|
| `templates/models/bulk.html` |Confirmation of a collective action registered as `bulk.html`|
| `templates/audit.html` |Change history|
| `templates/models/form.html` |Create and process, sections, three storage buttons|
| `templates/models/pick.html` |Foreign key selection, registered as `pick.html`|
| `templates/partials/field.html` | Feldgruppe, registriert als `field.html` |
| `templates/models/detail.html` | Detail |
| `templates/models/delete.html` |Deletion confirmation|
| `templates/directory/users.html` | Benutzerliste |
| `templates/directory/user_form.html` |Create and edit users, password|
| `templates/directory/user_detail.html` | Benutzerdetail |
| `templates/directory/roles.html` | Rollen |
| `templates/directory/permissions.html` | Berechtigungen |
| `templates/errors/error.html` |403, 404, 422 and 429|

`rendering.rs` registers these files under fixed names. `base.html` is the parent template. Subpages fill the block `content`.

## Kontext

Jede Seite erhält `locale`, `theme`, `title`, Branding, den Katalog `i18n`, `signed_in`, `superuser`, `user_email`, `csrf_token`, `current_path`, `notice` und `alert`. Zusätzliche Felder kommen aus dem Handler, zum Beispiel `sections`, `rows` oder `models`. Modellformulare gruppieren Felder in `sections`. Benutzerformulare liefern `email`, `email_error`, `password_error` und `role_checks`. Passwortfelder bekommen keinen Wert.

Messages after a POST do not appear as free text in the URL. The codes `created`, `saved`, `deleted`, `updated` and `changed` are permitted. `changed` additionally requires a number `changed` from 0 to 100. The trader translates the code.

## Escaping

`.html` templates are automatically escaped at Tera. Values from the database, error messages and the catalog run through this escaping. `primary_color` is previously limited to Hex. There is no `|safe` output for record fields.

Boolean fields first send a hidden `false` and then the checkbox `true`. The parser retains the last value. A check mark thus remains `true`, a missing `false`.

## Formularfelder

The trader builds fields from `AdminFieldMeta` and `ColumnSpec`. Hidden and sensitive do not appear. Read only and `id` appear as text and are not sent. Foreign keys become `select` if the target model is registered, the list is not blocked and the user is allowed to read. The current ID is `selected`. Mandatory foreign keys do not have an empty option. A validation error sets `aria-invalid` and a text under the field.

The role assignment sends `email` and `role` from selection lists. The trader assigns only one role that `list_roles` already knows.

## Known gaps

`errors/error.html` shows a POST unsubscription form during a session. Without a session, she links to the registration. `directory/users.html` assigns roles via a `<select>`. The toolbar separates search and filter from “New”. Field groups are in `partials/field.html`. Built-in names remain fixed. A request may not choose a template path. A project can register additional names described in [admin-extension-api.md](admin-extension-api.md)]. These templates extend `base.html`, do not overwrite them and may not use `|safe`.
