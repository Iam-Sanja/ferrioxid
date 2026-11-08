# Directory audit

[Zur Übersicht](admin.md) · Auth: [auth.md](auth.md) · Transaktionen: [transactions.md](transactions.md) · Historie: [admin-audit-log.md](admin-audit-log.md)

**Stand:** v0.20. The cargo version remains `0.1.0`.

HTML and JSON of the directory call the same functions in `ferrioxid-admin`. Auth writing, session revocation and audit line share a transaction. If the audit fails, the previous Auth state remains.

## Ablauf

1. Check input. Argon2id hashes are created before `begin` so that the SQLite write lock is not held during hashing.
2. Transaktion starten.
3. Re-read the executing user. He must be active, staff and superuser. A superuser flag withdrawn in the meantime ends the process with `Forbidden`.
4. Check the target state: no superuser target for profile, password, active and staff. No self-deactivation. No withdrawal of your own staff flag. An authorization code containing `superuser` is rejected. Superuser allocation remains the CLI.
5. Letter. Password change and deactivation revoke the target sessions in the same transaction.
6. Write an audit line with `operation_id`.
7. Commit. Any error before it rolls back hash, flags, roles, rights, sessions, and audit.

## Ereignisse

| Aktion | `action` | Zusammenfassung |
|---|---|---|
|Creating Users| `user_create` |Email, including the role names set|
|Email, Active, Staff| `user_update` |new email, `active`/`inactive` or `staff`/`not-staff`|
| Passwort | `password_reset` | leer |
| Rolle | `role_assign` / `role_revoke` | Rollenname |
| Recht | `permission_grant` / `permission_revoke` | Berechtigungscode |

`record_id` is the user UUID or, in the case of a right, the role name. `actor_id` is the executing user. Passwords, hashes, session and CSRF tokens are not in the summary.

The CLI methods `create_user`, `assign_role`, `grant_permission` and `set_user_active` do not write a directory audit. There is no admin actor.

## Migration

`crates/ferrioxid-auth/migrations/0003_audit_operation.json` attaches the nullable column `operation_id` to `_ferrum_admin_audit`. `0001_auth_schema` and `0002_admin_journal` remain unchanged. Older lines read `operation_id` as empty. The history shows the first eight characters when a value exists.

## JSON

The JSON directory API uses the same transaction. It still does not require `confirm`. That remains open.
