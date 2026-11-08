# Authentifizierung

[For overview](index.md) · Sessions: [sessions.md](sessions.md) · Rights: [permissions.md](permissions.md)] Object rules: [policies.md](policies.md)]

## Zweck

Ferrioxid checks users, passwords and sessions on the server side. The Crate `ferrioxid-auth` is not attached to the HTML interface. The admin calls the same methods. Generated CRUD routes use the same test. Auth errors run through `problem_json`. `Invalid email or password` remains the same for unknown users and incorrect passwords and carries `auth.invalid_credentials`. The directory attaches role names without changing `PublicUser` from `/auth/me`.

Admin changes to users, roles and rights are made through `*_on` in a transaction with the audit. Session revocation for password and deactivation is part of this. The CLI methods remain without this audit. See [admin-directory-audit.md](admin-directory-audit.md)]. The user himself resets a forgotten password: [password-reset.md](password-reset.md)]. The shipment is in [mail.md](mail.md)].

## Architektur

`ferrioxid-auth` depends on `ferrioxid-core`, `ferrioxid-db` and `ferrioxid-migrations`, not `ferrioxid-api`. `ferrioxid-api` depends on `ferrioxid-auth` and registers the Auth routes along with the model routes. The CLI calls the same service methods.

Passwords are hashed with Argon2id and must have 12 to 128 characters. The CLI reads them covertly or from `FERRUM_PASSWORD`. Session tokens are stored only as SHA-256-hex. Object rules check Create input via `can_create` before a row is created. Setting the owner ID from the session is planned. The CSRF token is calculated from the key and session as needed; Only the hash is stored. HTML forms pass the same token as `csrf_token` field to `authenticate_csrf`. A second CSRF mechanism does not exist. Roles and permissions read each check again from the database.

## Responsible files

|File| Aufgabe |
|---|---|
| `crates/ferrioxid-auth/src/password.rs` |Argon2id, PHC string, test|
| `crates/ferrioxid-auth/src/service.rs` |User, login, rights, rate limit|
| `crates/ferrioxid-auth/src/store.rs` |SeaQuery against the Auth tables|
| `crates/ferrioxid-auth/src/schema.rs` |Snapshot and Migration|
| `crates/ferrioxid-auth/src/routes.rs` | `/auth/*` |
| `crates/ferrioxid-auth/src/recovery.rs` |Reset request, exchange and closure|
| `crates/ferrioxid-auth/migrations/0004_password_reset.json` |Token hash, exchange hash, outbox|
| `crates/ferrioxid-auth/migrations/0001_auth_schema.json` | Versiegeltes Schema |
| `crates/ferrioxid-api/src/lib.rs` | Guard vor generierten Routen |

## Public API

- `AuthConfig`, `AuthConfig::development`, `AuthConfig::from_env`, `AuthConfig::apply_bound_address`
- `AuthService::create_user`, `list_users`, `set_user_active`
- `login`, `logout`, `authenticate_token`, `change_password`, `rotate_session`
- `authorize`, `ensure_role`, `grant_permission`, `revoke_permission`, `assign_role`, `revoke_role`
- `request_reset`, `exchange_token`, `complete_reset`
- `apply_migrations`
- `App::with_auth`, `App::register`, `App::register_policy`

`PublicUser` contains `id`, `email`, `is_active`, `is_staff` and `is_superuser`. No hash and no token.

## Datenmodell

See [sessions.md](sessions.md) and [permissions.md](permissions.md)]. Timestamps are Unix seconds (`i64`) because ferrioxide does not have its own date type.

Email is trimmed and stored in lower case. It is unique, a maximum of 254 characters long and needs exactly one `@` and one point in the domain.

A new password has 12 to 128 characters. The hash uses Argon2id, version `0x13`, 19 456 KiB, two iterations and one thread. Salt and parameters are in the PHC string.

## Security mechanisms

- Unknown email, incorrect password and deactivated user deliver the same message `Invalid email or password`.
- If the user is missing, a fixed dummy hash is checked so that the Argon2 work is not eliminated.
- Deactivated users cannot log in. Deactivation via CLI or Admin immediately revokes the saved sessions.
- A password change via CLI or Admin revokes all user sessions. The admin password path uses `validate_new_password` and `hash_password`, does not show the hash and does not place the password in the forwarding.
- Staff and role changes write the admin in the same user line. The next `authenticate_token` exam reads the flags again. The sitting will not be deleted.
- Login is limited to five failed attempts per process in 15 minutes. The key is peer plus normalized email.
- `X-Forwarded-For` applies only if `FERRUM_TRUST_PROXY` is expressly `1` or `true`. The first hop is evaluated.
- Security logs do not name passwords, session tokens, login CSF cookies or form tokens.
- If `FERRUM_ALLOWED_ORIGINS` is not available, `AuthConfig::apply_bound_address` replaces the development list with the loopback originals of the address to which the process binds. Public hosts and the host header of a request are not included in this list. `Origin: null` remains invalid. A request without origin and without referer remains allowed, but the login CSF cookie is not.

## Error cases

| Situation | HTTP |
|---|---|
|Incorrect access data or deactivated user| 401, `Invalid email or password` |
|Missing or invalid session| 401, `Authentication required` |
|Missing eligibility| 403, `Access denied` |
|Too many login attempts| 429, `Too many login attempts` |
|Invalid e-mail or too short a new password| 422 |

## Example

```powershell
cargo run -p ferrioxid-cli -- migrate up
cargo run -p ferrioxid-cli -- admin create-user --email admin@example.com --superuser
cargo run -p ferrioxid-cli -- auth list-users
```

`create-user` asks for the password hidden and confirms it. There is no `--password` and no preset admin password.

```http
POST /auth/login
{"email":"admin@example.com","password":"example-password"}
```

The answer sets `Set-Cookie: ferrioxid_session=...` and provides `user` plus `csrf_token`. The session token is not in the JSON.

## Tests

`crates/ferrioxid-auth/src/password.rs`, `crates/ferrioxid-auth/src/sqlite_tests.rs` and `examples/myshop/tests/auth.rs` run with SQLite. PostgreSQL is not required for this.

## Limitations

- The rate limit is in the process. Several processes do not share it. The map can be replaced later. The reset uses its own key and the same window.
- There is no MFA, no OAuth and no JWT for external APIs. The e-mail reset is present in v0.23 and does not guarantee delivery.
- `is_active` is required for one session. An inactive account will be rejected upon registration.
- `is_staff` opens the administration and does not bypass model rights.
- `is_superuser` bypasses roles and object rules, but does not replace `is_staff`. A superuser without staff receives 403 in the admin and can log out there.
- `ferrioxid admin create-user` only sets the transferred flags and prints Active, Staff and Superuser. `--superuser` without `--staff` indicates the closed admin.

## Planned extensions

Multi-process rate limit, MFA and OAuth are reserved for later versions. The password reset is implemented.

## Admin and CSRF Bootstrap

`GET /auth/csrf` delivers the session token again. The session itself remains, and other tabs remain valid. Details are available in [csrf.md](csrf.md)]. The admin also requires `is_staff`. The directory under `/admin/api/directory` and the HTML pages under `/admin/directory/users` are reserved for staff superusers and cannot set `is_superuser`. See [admin-directory.md](admin-directory.md) and [admin-security.md](admin-security.md)]. The SQLite test `sqlite_admin_security_crud_and_csrf_bootstrap` checks the bootstrap.
