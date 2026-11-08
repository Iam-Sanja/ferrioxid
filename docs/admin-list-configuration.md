# Listenkonfiguration

[For overview](admin.md) · Comparison: [admin-django-parity.md](admin-django-parity.md)]

**Stand:** v0.17. The cargo version remains `0.1.0`.

The list reads the same `AdminFieldMeta` and `AdminModelMeta` values as the form and JSON API. There is no second configuration.

| Django-Begriff | Ferrioxid-Attribut | Wirkung |
|---|---|---|
| `list_display` | `#[admin(list_display)]` |columns in struct order. `id` always remains a column. Without marking: first visible text field, otherwise another visible field, plus `id`|
| `list_display_links` | `#[admin(list_link)]` |These columns link to the detail. `list_link` requires `list_display`, except for `id`. Without marking, the first listed column that is not `id` links, otherwise `id`|
| `ordering` |`admin_ordering = "name"` or `"-name"`|Standard if the request does not send a `sort`. The minus sorts descending. The column must be `sortable` or `id`. A sent `sort` replaces the default. An unknown name is 422|
| `list_per_page` | `admin_per_page = 25` |HTML page size from 1 to 100. Specification 25. The trader caps again at 100. There is no parameter `limit` and no “show all”|
| `list_filter` | `#[admin(filterable)]` | GET-Feld `filter.{spalte}` |
| `search_fields` | `#[admin(searchable)]` |`q` parameters, string columns only|

`hidden` and sensitive names fall from display, search, sorting and filtering. Sorting and filtering do not build SQL fragments from the request. The JSON list remains at a maximum of 100 lines and does not use `admin_per_page`.

Valid search and filters remain in the following links. A new filter resets the page because `page` is not sent.
