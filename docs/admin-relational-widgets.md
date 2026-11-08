# Foreign key selection

[For overview](admin.md) · Security: [admin-security.md](admin-security.md)]

**Stand:** v0.17. The cargo version remains `0.1.0`.

The form also shows a `<select>` with a maximum of 100 readable goals. If the current value is missing in this page and the user is allowed to read it, it is added at the front. Underneath it is the link “Continue searching”.

The selection page is GET:

- `/admin/models/{modell}/pick/{feld}`
- `/admin/models/{modell}/{id}/pick/{feld}`

It checks Staff, the write right of the initial model and the read right of the target. The search uses the visible `searchable` columns of the target. The page uses `admin_per_page` of the target, capped at 100, and the same query scope as the list. An unknown field name, a hidden or readable field and a destination without display text are 404. Without reading rights, the answer is 403.

A hit only links to the form of the initial model and sets the field name to the UUID from the database. Other form data are not retained. There is no `next` address from the request and no mandatory script.

When saving, `ensure_relations` rejects a UUID that the user cannot read at the registered destination. The answer remains the form with the error in the field. An unregistered destination is left to the database foreign key check.

After a validation error, a readable selection remains in the form because the transmitted field values are rendered again.
