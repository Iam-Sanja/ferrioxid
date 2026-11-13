# List configuration

[Overview](admin.md) · Comparison: [admin-django-parity.md](admin-django-parity.md)

**Milestone:** v0.17. The Cargo version remains `0.1.0`.

The list reads the same `AdminFieldMeta` and `AdminModelMeta` values as the form and JSON API. There is no second configuration.

| Django concept | Fexu attribute | Effect |
|---|---|---|
| `list_display` | `#[admin(list_display)]` |columns in struct order. `id` always remains a column. Without marking: first visible text field, otherwise another visible field, plus `id`|
| `list_display_links` | `#[admin(list_link)]` |These columns link to the detail. `list_link` requires `list_display`, except for `id`. Without marking, the first listed column that is not `id` links, otherwise `id`|
| `ordering` |`admin_ordering = "name"` or `"-name"`|Standard if the request does not send a `sort`. The minus sorts descending. The column must be `sortable` or `id`. A sent `sort` replaces the default. An unknown name is 422|
| `list_per_page` | `admin_per_page = 25` | HTML page size from 1 to 100, default 25. The handler enforces the upper bound. There is no `limit` parameter or “show all” mode. |
| `list_filter` | `#[admin(filterable)]` | GET-Field `filter.{spalte}` |
| `search_fields` | `#[admin(searchable)]` |`q` parameters, string columns only|

Hidden and sensitive fields are excluded from display, search, sorting and filtering. Request values are bound as parameters rather than SQL fragments. JSON lists retain their maximum of 100 rows and do not use `admin_per_page`.

Pagination links preserve valid search and filter values. Submitting a new filter omits `page`, returning to the first page.
