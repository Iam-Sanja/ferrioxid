# Sammelaktionen

[Zur Übersicht](admin.md) · Eigene Aktionen: [admin-custom-actions.md](admin-custom-actions.md)

## Surface surface

The model list shows if at least one permitted action exists:

- one checkbox `ids` per line
- a checkbox `scope=page` for the current page
- a `select` with the action name
- A Submit

`scope=page` is not a script selection. The server reads the page with the write scope again and ignores sent IDs. The page size remains 1 to 100. The whole table is not chosen.

## Ablauf

1. `POST /admin/models/{model}/bulk` checks CSRF, the registered name, UUID IDs, the upper limit and for each ID read access plus object rule. A failure does not save a draft and does not change a line.
2. The permitted selection is stored as a draft with purpose `bulk`. The answer is 303 to `GET /admin/models/{model}/bulk/{draft}`. This GET does not write.
3. `POST` of the same address without `confirm=1` shows the confirmation again and does not write.
4. With `confirm=1`, the server checks the right again. A now deprived right is 403 and does not change anything.
5. A transaction reads each ID again, performs the operation, writes an audit per line and commits. Then the draft is deleted.

## Nuclearity

`Delete` and `SetBool` are atomic: each selected line is changed or none. A database error, including `ON DELETE RESTRICT`, rolls back the transaction. The error page arises only after the end of the transaction, so that the only SQLite connection is not blocked.

Double IDs count once. An unknown action, a non-UUID or a crossed border is 422. Missing CSRF is 403. A foreign or missing ID refuses the whole selection.

`App::bootstrap_schema` does not generate foreign keys. The rollback test therefore rebuilds the product table with `ON DELETE RESTRICT`. Productively, the foreign keys of the migration apply.

Code: `crates/ferrioxid-admin/src/actions.rs`, `html.rs` `stage_bulk`, `show_bulk`, `apply_bulk`. Vorlage: `templates/models/bulk.html`.
