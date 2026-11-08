# Change history

[For overview](admin.md) · Security: [admin-security.md](admin-security.md)]

## Tabelle

`_ferrum_admin_audit` from `crates/ferrioxid-auth/migrations/0002_admin_journal.json`. SQLite saves the UUID columns as text. The same migration runs through the existing engine. PostgreSQL compatibility follows from the dialect of the engine. The ignored PostgreSQL tests were not run for this.

| Spalte | Inhalt |
|---|---|
| `id` | Ereignis-UUID |
| `created_at` | Unix-Sekunden |
| `actor_id` |User UUID|
| `model_key` |Model key|
| `record_id` |Record ID, empty for a rejected aggregate action|
| `action` | siehe unten |
| `outcome` |`ok` or `denied`|
| `summary` |Shortened text summary|
| `operation_id` |Nullable UUID from `0003_audit_operation`. Older lines remain empty. History shows eight characters|

Aktionen: `create`, `update`, `delete`, `bulk_update`, `bulk_delete`, `user_create`, `user_update`, `password_reset`, `role_assign`, `role_revoke`, `permission_grant`, `permission_revoke`.

## What is not in the summary

Felder mit `sensitive`, verborgene Felder, Namen `password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash` und Endungen `_hash` oder `_secret`. Höchstens 12 Zeilen, insgesamt 1500 Zeichen, ein Wert höchstens 80 Zeichen, Zeilenumbrüche ersetzt. Die Vorlage escaped die Ausgabe.

A normal change uses the field label: `Name: "Apfel" → "Grüner Apfel"`. A password reset has an empty summary. Create users only saves the e-mail. Roles store the role name.

## Transaktion

`audited_create`, `audited_update` and `audited_delete` write model line and audit in one transaction. The public `ModelRepository::create` does not do this. The JSON Admin API goes via `ManagedModel` and writes along.

Directory events and a `|form` inline write audit in the same transaction as the data. If the audit fails, the previous state remains. Details: [admin-directory-audit.md](admin-directory-audit.md) and [admin-transactional-inlines.md](admin-transactional-inlines.md)]. CLI-Auth does not write this audit.

## Seite

`GET /admin/audit` requires staff. Superusers see all lines, descending by time and ID, 25 per page. Other staff sees only `actor_id` equal to their own ID. As a result, unreadable data sets and foreign field values are not visible over the history. This is narrower than “every readable data set”.

The page is in the sidebar under system, not as a model. `?model=` and `?record=` restrict the existing columns. Invalid filters are discarded. Detailed pages link to these filters when the model key and record ID are stored. Directory rights are further under `_ferrum_users` with the role name as `record_id`.

`created_at` remains Unix seconds. The HTML page shows `YYYY-MM-DD HH:MM:SS UTC` in `<time>`. The time zone of the display is UTC. An unrepresentable value is called unreadable. The stored value is not converted and is not overwritten.

Code: `crates/ferrioxid-admin/src/audit.rs`. Test: `summary_skips_sensitive_values` and `admin_v018_drafts_inlines_bulk_and_audit`.
