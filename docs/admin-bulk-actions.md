# Bulk actions

[Overview](admin.md) · Custom actions: [admin-custom-actions.md](admin-custom-actions.md)

## User interface

The model list shows if at least one permitted action exists:

- One `ids` checkbox per row
- a checkbox `scope=page` for the current page
- a `select` with the action name
- A submit button

`scope=page` is a server-side selection. The server reloads the current page with the write scope and ignores submitted IDs. Page size remains between 1 and 100; the action never selects the entire table.

## Workflow

1. `POST /admin/models/{model}/bulk` checks CSRF, the registered action name, UUID syntax, the selection limit, and read access plus object-policy checks for each ID. Failure neither saves a draft nor changes a row.
2. The permitted selection is stored in a draft with purpose `bulk`. The response redirects with 303 to `GET /admin/models/{model}/bulk/{draft}`. This GET does not write.
3. `POST` of the same address without `confirm=1` shows the confirmation again and does not write.
4. With `confirm=1`, the server checks permissions again. A permission revoked since selection returns 403 without changing data.
5. A transaction reloads every selected ID, applies the action and writes an audit entry per row. The draft is deleted after commit.

## Atomicity

`Delete` and `SetBool` are atomic: either every selected row changes or none does. Database errors, including `ON DELETE RESTRICT`, roll back the transaction. The error page is rendered after the transaction ends so SQLite's single connection is available.

Duplicate IDs count once. An unknown action, invalid UUID or exceeded selection limit returns 422. Missing CSRF returns 403. A missing or inaccessible ID rejects the entire selection.

`App::bootstrap_schema` does not create foreign keys. The rollback test therefore rebuilds its product table with `ON DELETE RESTRICT`. In applications, foreign keys come from migrations.

Code: `crates/ferrioxid-admin/src/actions.rs` and the `stage_bulk`, `show_bulk`, `apply_bulk` functions in `html.rs`. Template: `templates/models/bulk.html`.
