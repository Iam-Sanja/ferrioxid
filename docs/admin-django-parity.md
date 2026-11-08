# Django admin and ferrioxide

[Zur Übersicht](index.md) · Roadmap: [roadmap.md](roadmap.md) · UX: [admin-ux-backlog.md](admin-ux-backlog.md)

**Stand:** v0.22. The cargo version remains `0.1.0`. The table below evaluates the native admin. Functional equality is the goal. Python-`ModelAdmin`, Django templates and Django URLs are not replicated. The date, time and UTC time are model types. The date hierarchy filters according to the UTC calendar. Directory changes and opt-in parent-child forms are each a transaction.

Statuswerte:

- **Is:** usable in native admin and tested or hardwired in code.
- **Part:** present, but narrower or incomplete compared to the term Django mentioned.
- ** Missing:* not present. It will appear in the roadmap as soon as a phase is mentioned.
- **Inappropriate:** not intended for ferrioxide because it would be code compatibility or a JavaScript obligation.

## Listen

| Django | Ferrioxid | Status | Phase | Dateien |
|---|---|---|---|---|
| `list_display` |`#[admin(list_display)]` has priority. Without further marking: first visible text field, otherwise another visible field, plus `id`| Ist | — | `ferrioxid-macros`, `html.rs` `listed_fields`, `templates/models/list.html` |
| `list_display_links` |`#[admin(list_link)]`. Without marking the first listed column except `id`| Ist | — | `list.html`, [admin-list-configuration.md](admin-list-configuration.md) |
| `list_filter` |`filterable` and registered `AdminFilter`| Ist |Bool, foreign key, number and fixed selections. Date ranges remain open| [admin-custom-filters.md](admin-custom-filters.md) |
| `search_fields` |`searchable` on string columns, parameters `q`| Ist | — | Makro, `html.rs` |
| `ordering` |`admin_ordering`, otherwise `id`. A query `sort` has priority and is checked against the metadata| Ist | — | `html.rs` `resolved_sort` |
| `list_per_page` |`admin_per_page`, 1 to 100, HTML specification 25. JSON remains at 100| Ist | — | `html.rs` |
| `list_max_show_all` |There is no “show all”. The side is capped| Ist | — | `html.rs` |
| `list_select_related` |No join. Display names come from one query per destination table and page| Teil | — | `ferrioxid-db` `query_labels` |
| `date_hierarchy` |`admin_date_hierarchy` on `NaiveDate` or `DateTime<Utc>`| Ist |UTC calendar, maximum 100 groups, Query scope remains| [admin-date-hierarchy.md](admin-date-hierarchy.md) |
|Actions on several lines|Checkbox per line, `scope=page` for the current page, registered action, confirmation page| Ist |Delete, activate, deactivate and `AdminAction`. All or nothing in a transaction| [admin-bulk-actions.md](admin-bulk-actions.md) |
|Extended list lines|No added lines under the list line| Fehlt | offen | Template |

## Formulare

| Django | Ferrioxid | Status | Phase | Dateien |
|---|---|---|---|---|
| `fields` |Visible fields come from `AdminFieldMeta`, minus hidden and sensitive| Ist | — | `visible_fields` |
| `exclude` |`hidden` hides and `reject_locked` rejects POST| Teil | — | Makro, `html.rs` |
| `fieldsets` |`admin_fieldset` and `#[admin(fieldset)]`. Without declaration an unnamed group| Ist | — | [admin-fieldsets.md](admin-fieldsets.md) |
| `readonly_fields` |`readonly` and `id` are text, not enterable| Ist | — | `field.html`, `list.html` |
| `autocomplete_fields` |Select with a maximum of 100 readable destinations plus selection page with search and pages| Ist |Without script| [admin-relational-widgets.md](admin-relational-widgets.md) |
| `raw_id_fields` |No free raw ID input. An unreadable UUID is rejected when stored| Teil | — | `ensure_relations` |
| `filter_horizontal` / `filter_vertical` |No multiple selection with two lists|Not suitable as a JavaScript widget. Multiple selection without script can later be a checkbox group|v0.18, only if Many-to-Many exists| — |
| `formfield_overrides` |`AdminWidget` for text, text area, email, URL, number and read-only| Teil |Foreign keys remain the built-in select| [admin-custom-widgets.md](admin-custom-widgets.md) |
| Vorbelegung |No default from the admin configuration. Database defaults remain in the migration| Fehlt | v0.17 | Formular-Handler |
| Validierung |Field rules and `can_create` run on the server side. Errors are in the field| Ist | — | `ferrioxid-forms`, `html.rs` |
|Validation during input|No script checks before POST|Inappropriate as a duty. Server-side errors remain the decrease| — | — |

## Relationen

|Capability| Status | Phase |Note:|
|---|---|---|---|
| Belongs-To | Ist | — |Select to 100 plus selection page. Unreadable IDs are not stored. List and detail show the metadata name|
| Listenfilter | Ist | — |Bool and foreign key are selections, numbers are `type="number"`, text remains search. All servers|
| Has-Many | Ist | — |`admin_inline` shows the child list with reading rights and pages. Creating is your own form|
| Inline-Formulare | Teil | — |Without `form` the own creation form remains. With `form`, parent objects and a has-many level are stored together. No many-to-many, no nesting, maximum 50 children|
|Autocomplete| Ist | — |GET selection page, no ID, no mandatory script|
| Berechtigte Auswahl | Ist | — |Without Reading Rights, no Options|
|Relational change| Teil | v0.18 |A foreign key can be set. Cascades remain those of migration|

## Actions and life cycle

|Capability| Status | Phase |
|---|---|---|
|Save and redirect to the detail| Ist | — |
|Storage and further processing| Ist |`save=edit`, only after successful writing|
|Save and re-create| Ist |`save=new`. Another value is 422 and does not write|
|Delete with confirmation page| Ist |GET does not delete|
|Custom admin actions| Ist |`Delete`, `SetBool` and `ActionHandler`. The HTTP name only performs registered actions|
|Change history| Teil |Superuser sees all events. Other staff only own. Summary without sensitive fields|
| Audit-Log | Teil |Model writing and directory changes share the transaction with their audit. `operation_id` groups parent and child lines. The history page shows eight characters. CLI-Auth remains without this audit|
|Objective Eligibility| Ist |`ObjectPolicy`, unreadable sentences are 404 like missing sentences|
| Eigene Admin-Hooks | Teil |`ViewHandler` and `ActionHandler`. No paths from the request|

## Erweiterbarkeit

|Capability| Status | Phase | Abgrenzung |
|---|---|---|---|
| Eigene Admin-Views | Ist | v0.22 |`/admin/pages/{key}`, Staff, Legal, CSRF. No open path|
| Eigene Templates | Teil | v0.22 |Registered names in addition to those embedded. No path from the request|
|Model settings| Ist | — |Additionally `admin_inline`. No free Python class|
| Eigene Widgets | Teil | v0.22 |Text, text area, email, URL, number, read only. Without script|
| Eigene Filter | Ist | v0.22 |Fixed column, bound value, Query scope remains|
| Eigene Aktionen | Ist | v0.22 |`try_action` and `ActionHandler`. POST, CSRF, Right, Confirmation, no execution via GET|
|Reusable building blocks| Teil | v0.15 |Today Tera blocks. No React components|

## Users and rights

|Capability| Status |Note:|
|---|---|---|
|`is_staff` as admin door| Ist |Without Staff no model and no directory|
|`is_superuser` skips model rights| Ist |Does not replace Staff|
| `is_active` | Ist |Inactive sessions end at next exam|
|Create users, change email, set password| Ist |Own HTML routes. Superuser remains CLI. No hard-delete|
|Roles and rights| Ist |Create, give, take, assign. Password and deactivation revoked sessions|
| Superuser vergeben | Unpassend im Admin |Remains CLI. The admin responds 403 to superuser mutation|
| Selbstsperre | Ist |Your own deactivation and own staff withdrawal are blocked|

## What ferrioxide does not take

- Same class and method signature as `django.contrib.admin`.
- Django templates, Django forms or the Django design.
- An admin that works only with JavaScript without server HTML.
- Reload `AdminSite` from user files at runtime.
- An allowlist that generally accepts `Origin: null`. The exception `Sec-Fetch-Site: same-origin` from v0.13.1 remains the only one.
