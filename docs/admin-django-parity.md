# Django admin and Ferrioxid

[Overview](index.md) · Roadmap: [roadmap.md](roadmap.md) · UX: [admin-ux-backlog.md](admin-ux-backlog.md)

**Milestone:** v0.22. The Cargo version remains `0.1.0`. The table below evaluates the native admin. Functional equality is the goal. Python-`ModelAdmin`, Django templates and Django URLs are not replicated. The date, time and UTC time are model types. The date hierarchy filters according to the UTC calendar. Directory changes and opt-in parent-child forms are each a transaction.

Status values:

- **Implemented:** usable in the native admin and covered by tests or enforced directly in code.
- **Partial:** implemented with narrower behavior than the corresponding Django capability.
- **Missing:** not implemented; any listed phase is a roadmap item.
- **Not applicable:** intentionally excluded because it would require Python compatibility or mandatory JavaScript.

## Lists

| Django | Ferrioxid | Status | Phase | Files |
|---|---|---|---|---|
| `list_display` |`#[admin(list_display)]` has priority. Without further marking: first visible text field, otherwise another visible field, plus `id`| Implemented | — | `ferrioxid-macros`, `html.rs` `listed_fields`, `templates/models/list.html` |
| `list_display_links` |`#[admin(list_link)]`. Without marking the first listed column except `id`| Implemented | — | `list.html`, [admin-list-configuration.md](admin-list-configuration.md) |
| `list_filter` |`filterable` and registered `AdminFilter`| Implemented |Bool, foreign key, number and fixed selections. Date ranges remain open| [admin-custom-filters.md](admin-custom-filters.md) |
| `search_fields` |`searchable` on string columns, parameters `q`| Implemented | — | Macro, `html.rs` |
| `ordering` |`admin_ordering`, otherwise `id`. A query `sort` has priority and is checked against the metadata| Implemented | — | `html.rs` `resolved_sort` |
| `list_per_page` |`admin_per_page`, 1 to 100, HTML specification 25. JSON remains at 100| Implemented | — | `html.rs` |
| `list_max_show_all` | No “show all” mode; page size is capped. | Implemented | — | `html.rs` |
| `list_select_related` |No join. Display names come from one query per destination table and page| Partial | — | `ferrioxid-db` `query_labels` |
| `date_hierarchy` |`admin_date_hierarchy` on `NaiveDate` or `DateTime<Utc>`| Implemented |UTC calendar, maximum 100 groups, Query scope remains| [admin-date-hierarchy.md](admin-date-hierarchy.md) |
|Actions on several lines|Checkbox per line, `scope=page` for the current page, registered action, confirmation page| Implemented |Delete, activate, deactivate and `AdminAction`. All or nothing in a transaction| [admin-bulk-actions.md](admin-bulk-actions.md) |
| Extra list rows | No additional rows beneath a record | Missing | Open | Template |

## Forms

| Django | Ferrioxid | Status | Phase | Files |
|---|---|---|---|---|
| `fields` |Visible fields come from `AdminFieldMeta`, minus hidden and sensitive| Implemented | — | `visible_fields` |
| `exclude` |`hidden` hides and `reject_locked` rejects POST| Partial | — | Macro, `html.rs` |
| `fieldsets` |`admin_fieldset` and `#[admin(fieldset)]`. Without declaration an unnamed group| Implemented | — | [admin-fieldsets.md](admin-fieldsets.md) |
| `readonly_fields` |`readonly` and `id` are text, not enterable| Implemented | — | `field.html`, `list.html` |
| `autocomplete_fields` |Select with a maximum of 100 readable destinations plus selection page with search and pages| Implemented |Without script| [admin-relational-widgets.md](admin-relational-widgets.md) |
| `raw_id_fields` |No free raw ID input. An unreadable UUID is rejected when stored| Partial | — | `ensure_relations` |
| `filter_horizontal` / `filter_vertical` |No multiple selection with two lists|Not suitable as a JavaScript widget. Multiple selection without script can later be a checkbox group|v0.18, only if Many-to-Many exists| — |
| `formfield_overrides` |`AdminWidget` for text, text area, email, URL, number and read-only| Partial |Foreign keys remain the built-in select| [admin-custom-widgets.md](admin-custom-widgets.md) |
| Initial values |No default from the admin configuration. Database defaults remain in the migration| Missing | v0.17 | Form handler |
| Validation |Field rules and `can_create` run on the server side. Errors are in the field| Implemented | — | `ferrioxid-forms`, `html.rs` |
| Client-side validation | No JavaScript validation before POST | Not applicable as a requirement; server-side validation is authoritative | — | — |

## Relations

|Capability| Status | Phase |Note:|
|---|---|---|---|
| Belongs-To | Implemented | — |Select to 100 plus selection page. Unreadable IDs are not stored. List and detail show the metadata name|
| List filters | Implemented | — | Boolean and foreign-key selects, numeric `type="number"` inputs and text search; all processed on the server. |
| Has-Many | Implemented | — | `admin_inline` shows readable child records with pagination and a separate creation form. |
| Inline forms | Partial | — | Without `form`, creation remains separate. With `form`, a parent and one level of has-many children save together. No many-to-many or nesting; at most 50 children. |
|Autocomplete| Implemented | — |GET selection page, no ID, no mandatory script|
| Authorized selection | Implemented | — |Without Reading Rights, no Options|
|Relational change| Partial | v0.18 |A foreign key can be set. Cascades remain those of migration|

## Actions and life cycle

|Capability| Status | Phase |
|---|---|---|
|Save and redirect to the detail| Implemented | — |
| Save and continue editing | Implemented | `save=edit`, only after a successful write |
|Save and re-create| Implemented |`save=new`. Another value is 422 and does not write|
|Delete with confirmation page| Implemented |GET does not delete|
|Custom admin actions| Implemented |`Delete`, `SetBool` and `ActionHandler`. The HTTP name only performs registered actions|
|Change history| Partial |Superuser sees all events. Other staff only own. Summary without sensitive fields|
| Audit-Log | Partial |Model writing and directory changes share the transaction with their audit. `operation_id` groups parent and child lines. The history page shows eight characters. CLI-Auth remains without this audit|
| Object-level permissions | Implemented | `ObjectPolicy`; inaccessible records return 404 like missing records |
| Custom admin hooks | Partial |`ViewHandler` and `ActionHandler`. No paths from the request|

## Extensibility

|Capability| Status | Phase | Scope |
|---|---|---|---|
| Custom admin views | Implemented | v0.22 | `/admin/pages/{key}`, staff access, permissions and CSRF; no request-selected paths |
| Custom templates | Partial | v0.22 |Registered names in addition to those embedded. No path from the request|
|Model settings| Implemented | — |Additionally `admin_inline`. No free Python class|
| Custom widgets | Partial | v0.22 |Text, text area, email, URL, number, read only. Without script|
| Custom filters | Implemented | v0.22 |Fixed column, bound value, Query scope remains|
| Custom actions | Implemented | v0.22 |`try_action` and `ActionHandler`. POST, CSRF, Right, Confirmation, no execution via GET|
|Reusable building blocks| Partial | v0.15 |Today Tera blocks. No React components|

## Users and permissions

|Capability| Status |Note:|
|---|---|---|
| `is_staff` as the admin access gate | Implemented | Required for both model and directory access |
|`is_superuser` skips model permissions| Implemented |Does not replace Staff|
| `is_active` | Implemented | Inactive sessions are rejected on the next check. |
|Create users, change email, set password| Implemented |Dedicated HTML routes. Superuser remains CLI. No hard-delete|
| Roles and permissions | Implemented | Create, grant, revoke and assign. Password changes and deactivation revoke sessions. |
| Grant superuser access | Not supported in the admin |Remains CLI. The admin responds 403 to superuser mutation|
| Self-lockout | Implemented |Your own deactivation and own staff withdrawal are blocked|

## What Ferrioxid does not adopt

- Same class and method signature as `django.contrib.admin`.
- Django templates, Django forms or the Django design.
- An admin that works only with JavaScript without server HTML.
- Reload `AdminSite` from user files at runtime.
- An allowlist that generally accepts `Origin: null`. The exception `Sec-Fetch-Site: same-origin` from v0.13.1 remains the only one.
