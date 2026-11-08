# Draft forms

[For overview](admin.md) · Security: [admin-security.md](admin-security.md)]

## Zweck

The foreign key selection is its own GET page. The draft keeps the remaining, not yet saved fields on the server, bound to the session. The URL contains the draft ID and the selected key, not the form content.

`save=rows` writes the same draft and no model line. Keys `inline_*`, `extra_*` and `basis` are retained. Passwords, CSRF and keys with `password` are not stored. Limit remains 8 KiB, 30 minutes and 20 drafts per session.

## Speicher

Tabelle `_ferrum_admin_drafts` aus `crates/ferrioxid-auth/migrations/0002_admin_journal.json`.

| Spalte | Inhalt |
|---|---|
| `id` |Random UUID|
| `session_id` |Text UUID of the session, foreign key on `_ferrum_sessions.id` with `ON DELETE CASCADE`|
| `user_id` |Text UUID of the user|
| `model_key` |Registered model key|
| `record_id` |Empty when created, otherwise the record ID|
| `purpose` |`form` or `bulk`|
| `payload` |JSON, not more than 8192 bytes|
| `expires_at` |30 minutes after putting on|

A maximum of 20 drafts per meeting. Expired and then the oldest are deleted. Each call creates a new ID. Two tabs do not overwrite each other.

## What is not stored

Password fields, names with `password`, `token`, `secret`, `csrf` or endings `_hash` and `_secret`, the CSRF field, `next`, `save` and unknown keys. A reading access checks session, user, model, purpose and flow. A foreign user does not receive a draft.

## Ablauf

1. `POST /admin/models/{model}/draft` or `POST /admin/models/{model}/{id}/draft` stores the allowed fields and responds 303 to the selection page with `?draft=`.
2. The selection links back to `draft` and the selected foreign key.
3. The form sets `_use_draft`, fills in missing fields from the draft, and allows Query selections to win.
4. A validation error renders the sent values and retains the draft ID. He does not set `_use_draft`.
5. A successful saving deletes the design. Canceling is GET and waiting for the expiration.

The final POST checks fields, relations and policies again. The draft is not a write release.

Code: `crates/ferrioxid-admin/src/drafts.rs`, handler in `html.rs`. Test: `admin_v018_drafts_inlines_bulk_and_audit`.
