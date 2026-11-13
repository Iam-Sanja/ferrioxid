# Directory audit

[Overview](admin.md) · Auth: [auth.md](auth.md) · Transactions: [transactions.md](transactions.md) · History: [admin-audit-log.md](admin-audit-log.md)

**Milestone:** v0.20. The Cargo version remains `0.1.0`.

The HTML and JSON directory endpoints call the same functions in `fexu-admin`. Auth changes, session revocation and the audit entry share one transaction. If auditing fails, the previous authentication state is preserved.

## Workflow

1. Check input. Argon2id hashes are created before `begin` so that the SQLite write lock is not held during hashing.
2. Begin the transaction.
3. Reload the acting user. The user must be active, staff and superuser. If superuser access was revoked in the meantime, the operation ends with `Forbidden`.
4. Check the target state: profile, password, active and staff changes cannot target superusers. Self-deactivation and removal of one's own staff access are blocked. Permission codes containing `superuser` are rejected. Granting superuser access remains a CLI operation.
5. Apply the change. Password changes and deactivation revoke the target user's sessions in the same transaction.
6. Write an audit line with `operation_id`.
7. Commit. Any error before it rolls back hash, flags, roles, permissions, sessions, and audit.

## Events

| Event | `action` | Summary |
|---|---|---|
|Creating Users| `user_create` |Email, including the role names set|
|Email, Active, Staff| `user_update` |new email, `active`/`inactive` or `staff`/`not-staff`|
| Password | `password_reset` | Empty |
| Role | `role_assign` / `role_revoke` | Role name |
| Permission | `permission_grant` / `permission_revoke` | Permission code |

`record_id` is the user UUID or, in the case of a right, the role name. `actor_id` is the executing user. Passwords, hashes, session and CSRF tokens are not in the summary.

The CLI methods `create_user`, `assign_role`, `grant_permission` and `set_user_active` do not write a directory audit. There is no admin actor.

## Migration

`crates/fexu-auth/migrations/0003_audit_operation.json` attaches the nullable column `operation_id` to `_fexu_admin_audit`. `0001_auth_schema` and `0002_admin_journal` remain unchanged. Older lines read `operation_id` as empty. The history shows the first eight characters when a value exists.

## JSON

The JSON directory API uses the same transaction. It still does not require `confirm`. That remains open.
