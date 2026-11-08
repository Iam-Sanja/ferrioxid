# Change history

[Overview](admin.md) · Security: [admin-security.md](admin-security.md)

## Table

`_ferrum_admin_audit` from `crates/ferrioxid-auth/migrations/0002_admin_journal.json`. SQLite saves the UUID columns as text. The same migration runs through the existing engine. PostgreSQL compatibility follows from the dialect of the engine. The ignored PostgreSQL tests were not run for this.

| Column | Contents |
|---|---|
| `id` | Event UUID |
| `created_at` | Unix seconds |
| `actor_id` |User UUID|
| `model_key` |Model key|
| `record_id` | Record ID; empty for a rejected bulk action |
| `action` | See below |
| `outcome` |`ok` or `denied`|
| `summary` |Shortened text summary|
| `operation_id` | Nullable UUID added by `0003_audit_operation`. Older rows have no value; the history page displays the first eight characters. |

Actions: `create`, `update`, `delete`, `bulk_update`, `bulk_delete`, `user_create`, `user_update`, `password_reset`, `role_assign`, `role_revoke`, `permission_grant`, `permission_revoke`.

## What is not in the summary

The summary excludes fields marked `sensitive`, hidden fields, names `password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`, and names ending in `_hash` or `_secret`. It is limited to 12 lines and 1500 characters in total, with at most 80 characters per value. Embedded line breaks are replaced. The template escapes the output.

A normal change uses the field label: `Name: "Apfel" → "Grüner Apfel"`. Password resets have an empty summary. User creation records only the email address; role changes record the role name.

## Transaction

`audited_create`, `audited_update` and `audited_delete` write the model row and audit entry in one transaction. Public `ModelRepository::create` calls do not add an audit entry. The JSON admin API uses `ManagedModel`, which includes auditing.

Directory changes and an inline configured with `|form` write the audit entry in the same transaction as the data. If auditing fails, the previous state is preserved. See [admin-directory-audit.md](admin-directory-audit.md) and [admin-transactional-inlines.md](admin-transactional-inlines.md). Auth CLI commands do not write this audit.

## Page

`GET /admin/audit` requires staff access. Superusers see all entries, ordered by descending timestamp and ID, with 25 entries per page. Other staff see only entries whose `actor_id` is their own ID. They cannot use history to read other users' changes or field values.

The page appears under System in the sidebar, not as a model. `?model=` and `?record=` filter the stored columns; invalid filters are discarded. Detail pages link to these filters when a model key and record ID are available. Directory permission events use `_ferrum_users` as the model key and the role name as `record_id`.

`created_at` remains Unix seconds. The HTML page shows `YYYY-MM-DD HH:MM:SS UTC` in `<time>`. The time zone of the display is UTC. An unrepresentable value is called unreadable. The stored value is not converted and is not overwritten.

Code: `crates/ferrioxid-admin/src/audit.rs`. Test: `summary_skips_sensitive_values` and `admin_v018_drafts_inlines_bulk_and_audit`.
