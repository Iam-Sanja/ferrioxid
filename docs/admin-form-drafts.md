# Form drafts

[Overview](admin.md) · Security: [admin-security.md](admin-security.md)

## Purpose

The foreign key selection is its own GET page. The draft keeps the remaining, not yet saved fields on the server, bound to the session. The URL contains the draft ID and the selected key, not the form content.

`save=rows` saves a draft without writing a model row. Keys `inline_*`, `extra_*` and `basis` are retained; passwords, CSRF values and keys containing `password` are excluded. Limits are 8 KiB, 30 minutes and 20 drafts per session.

## Storage

The table is `_ferrum_admin_drafts`, created by `crates/ferrioxid-auth/migrations/0002_admin_journal.json`.

| Column | Contents |
|---|---|
| `id` |Random UUID|
| `session_id` |Text UUID of the session, foreign key on `_ferrum_sessions.id` with `ON DELETE CASCADE`|
| `user_id` |Text UUID of the user|
| `model_key` |Registered model key|
| `record_id` |Empty when created, otherwise the record ID|
| `purpose` |`form` or `bulk`|
| `payload` |JSON, not more than 8192 bytes|
| `expires_at` | 30 minutes after creation |

A session may have at most 20 drafts. Cleanup removes expired drafts first, then the oldest. Every save creates a new ID, so two tabs never overwrite each other.

## What is not stored

Excluded values are password fields, names containing `password`, `token`, `secret` or `csrf`, names ending in `_hash` or `_secret`, the CSRF field, `next`, `save` and unknown keys. Reading a draft checks session, user, model, purpose and flow. Another user cannot read it.

## Workflow

1. `POST /admin/models/{model}/draft` or `POST /admin/models/{model}/{id}/draft` stores the allowed fields and responds 303 to the selection page with `?draft=`.
2. The selection links back to `draft` and the selected foreign key.
3. The form sets `_use_draft`, fills missing fields from the draft and gives explicit query selections precedence.
4. A validation error renders the submitted values and keeps the draft ID without setting `_use_draft`.
5. A successful save deletes the draft. Cancel is a GET; the draft remains until expiration.

The final POST rechecks fields, relations and policies. A draft never grants permission to write.

Code: `crates/ferrioxid-admin/src/drafts.rs`, handler in `html.rs`. Test: `admin_v018_drafts_inlines_bulk_and_audit`.
