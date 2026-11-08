# Admin components

[Overview](admin.md) · Templates: [admin-templates.md](admin-templates.md) · Design: [admin-design-system.md](admin-design-system.md)

The building blocks are Tera blocks, not React components. Date fields are native inputs. The date hierarchy is a link list with the class `hierarchy`. A transactional inline is a `fieldset.form-section` in the existing form card, not a second card per child. The line is `.inline-row`.

| Component | Location |
|---|---|
| Layout, sidebar and header | `templates/base.html` |
| Field group | `templates/partials/field.html`, included as `field.html` |
| Table, search and pagination | `templates/models/list.html` |
|Notice and warning|`base.html`, roles `status` and `alert`|
| Error page | `templates/errors/error.html` |
|Users, roles, permissions| `templates/directory/` |

Fields are built in `html.rs` from model metadata. Unknown, hidden and read-only fields are not rendered as writable inputs. The shared `reject_locked` check rejects them even when they appear in a POST. A Boolean field with no submitted value uses `default_bool`; an existing record value takes precedence.

## Component status

Implemented components are listed with their delivery milestone. The remaining design work is tracked in [admin-ux-backlog.md](admin-ux-backlog.md).

| Component | Milestone | Behavior without JavaScript |
|---|---|---|
| Logout form on error pages | Implemented, v0.14 | POST with CSRF, including for non-staff users |
| Staff, Superuser and Active status columns | Implemented, v0.14 | Three separate columns |
| List column showing the record label | Implemented, v0.14 | Metadata-driven; no fixed `name` field |
| Field group | Implemented, v0.15 | Label, help text, control and errors in order |
| Primary, quiet and danger buttons | Implemented, v0.15 | Shared classes throughout |
| Role selection | Implemented, v0.15 | `<select>` of existing roles; assignment cannot create roles |
| User form | Implemented, v0.16 | Create, edit and password forms; no hash or superuser field |
| Relation display | Implemented, v0.16 | Label from model metadata; UUID in `title` |
| Filter controls | Implemented, v0.16 | Boolean and foreign-key `select` controls; numeric `number` input |
| Confirmation | Implemented, v0.16 | Server checks the `confirm=1` checkbox |
| List configuration | Implemented, v0.17 | Link, ordering and page size from metadata |
| Fieldsets | Implemented, v0.17 | `fieldset` or `details`, without JavaScript |
| Save destinations | Implemented, v0.17 | Three submit buttons with server-side redirects |
| Selection page | Implemented, v0.17 | GET, search, pagination and read permissions |
| Form draft | Implemented, v0.18 | Hidden `_draft`, session-bound, no passwords |
| Inline list | Implemented, v0.18 | Child rows and a separate creation form |
| Bulk-action bar | Implemented, v0.18 | Checkboxes, action selection and confirmation page |
| History | Implemented, v0.18 | Paginated table with HTML-escaped values |

Details and tests: [admin-ux-backlog.md](admin-ux-backlog.md).
