# Admin-Bausteine

[For overview](admin.md) · Templates: [admin-templates.md](admin-templates.md) · Design: [admin-design-system.md](admin-design-system.md)]

The building blocks are Tera blocks, not React components. Date fields are native inputs. The date hierarchy is a link list with the class `hierarchy`. A transactional inline is a `fieldset.form-section` in the existing form card, not a second card per child. The line is `.inline-row`.

| Baustein | Ort |
|---|---|
|Frame, sidebar, head| `templates/base.html` |
| Feldgruppe | `templates/partials/field.html`, eingebunden als `field.html` |
|Table, search, sheets| `templates/models/list.html` |
|Notice and warning|`base.html`, roles `status` and `alert`|
| Fehlerseite | `templates/errors/error.html` |
|Users, roles, rights| `templates/directory/` |

Fields are created in `html.rs` from the model metadata. Unknown, hidden and readable fields are not output as writeable input. The joint examination `reject_locked` rejects them even if they appear in the POST. A Boolean field with no value in the form takes over `default_bool`. An existing data record value has priority.

## Planned, not implemented

The open lines of this table have not yet been implemented. Reacted lines call their phase.

| Baustein | Phase |Behavior without script|
|---|---|---|
|Deregistration form on error pages| umgesetzt, v0.14 |POST with CSRF, even without staff|
|Status columns Staff, Superuser, Active| umgesetzt, v0.14 | Drei eigene Spalten |
|List column with identifier| umgesetzt, v0.14 |Metadata, not a fixed field `name`|
| Formulargruppe | umgesetzt, v0.15 |Label, note, control, errors among themselves|
| Button primär, ruhig, Gefahr | umgesetzt, v0.15 |Same classes everywhere|
| Rollenauswahl | umgesetzt, v0.15 |`<select>` of existing roles, no creation via assignment|
| Benutzerformular | umgesetzt, v0.16 |Create, edit and password. No hash, no superuser field|
| Relationale Anzeige | umgesetzt, v0.16 |Name from the model metadata, UUID as `title`|
| Filterauswahl | umgesetzt, v0.16 |Bool and foreign key as `select`, number as `number`|
|Confirmation| umgesetzt, v0.16 |Checkbox `confirm=1`, checked on the server side|
| Listenkonfiguration | umgesetzt, v0.17 |Link, sorting, page size from the metadata|
| Formularabschnitte | umgesetzt, v0.17 |`fieldset` or `details`, without script|
| Speicherziele | umgesetzt, v0.17 |Three Submit buttons, server-side address|
| Auswahlseite | umgesetzt, v0.17 |GET, Search, Pages, Reading Rights|
| Formularentwurf | umgesetzt, v0.18 |Hidden `_draft`, session, no password|
| Inline-Liste | umgesetzt, v0.18 |Child lines and own creation form|
| Sammelaktionsleiste | umgesetzt, v0.18 |Checkboxes, action selection, confirmation page|
| Historie | umgesetzt, v0.18 |Table, Pages, HTML-escaped|

Details and tests: [admin-ux-backlog.md](admin-ux-backlog.md)].
