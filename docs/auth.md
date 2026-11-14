# Authentication

[Overview](index.md) · Sessions: [sessions.md](sessions.md) · Rights: [permissions.md](permissions.md) Object rules: [policies.md](policies.md)

## Purpose

Fexu validates users, passwords and sessions on the server. The `fexu-auth` crate is independent of the HTML interface; the admin and generated CRUD routes use the same authentication service. Authentication errors use `problem_json`. Unknown users and incorrect passwords receive the same `Invalid email or password` message with code `auth.invalid_credentials`. The user directory adds role names without changing the `PublicUser` response from `/auth/me`.

Admin changes to users, roles and permissions use `*_on` methods to run in the same transaction as the audit entry. Password changes and deactivation also revoke sessions in that transaction. CLI operations do not write this audit log. See [admin-directory-audit.md](admin-directory-audit.md). Users can reset forgotten passwords as described in [password-reset.md](password-reset.md); delivery is covered in [mail.md](mail.md). Confirmed TOTP, recovery codes, session revoke and account close are in [auth-providers.md](auth-providers.md). The threat model for protocols that are not implemented is in [auth-threat-model.md](auth-threat-model.md).

## Architecture

`fexu-auth` depends on `fexu-core`, `fexu-db` and `fexu-migrations`, not `fexu-api`. `fexu-api` depends on `fexu-auth` and registers the Auth routes along with the model routes. The CLI calls the same service methods.

Passwords use Argon2id and must contain 12 to 128 characters. The CLI reads passwords without echoing them, or from `FEXU_PASSWORD`. Only a SHA-256 hex digest of each session token is stored. Object policies call `can_create` before inserting a row; assigning the owner ID from the session is planned. The CSRF token is derived from the key and session when needed, and only its hash is stored. HTML forms pass it as the `csrf_token` field to `authenticate_csrf`. Roles and permissions are read from the database on every authorization check.

## Responsible files

|File| Responsibility |
|---|---|
| `crates/fexu-auth/src/password.rs` |Argon2id, PHC string, test|
| `crates/fexu-auth/src/service.rs` |User, login, permissions, rate limit|
| `crates/fexu-auth/src/store.rs` |SeaQuery against the Auth tables|
| `crates/fexu-auth/src/schema.rs` |Snapshot and Migration|
| `crates/fexu-auth/src/routes.rs` | `/auth/*` |
| `crates/fexu-auth/src/recovery.rs` |Reset request, exchange and closure|
| `crates/fexu-auth/migrations/0004_password_reset.json` |Token hash, exchange hash, outbox|
| `crates/fexu-auth/migrations/0001_auth_schema.json` | Sealed schema |
| `crates/fexu-api/src/lib.rs` | Authentication guards for generated routes |

## Public API

- `AuthConfig`, `AuthConfig::development`, `AuthConfig::from_env`, `AuthConfig::apply_bound_address`
- `AuthService::create_user`, `list_users`, `set_user_active`
- `login`, `login_replacing`, `logout`, `authenticate_token`, `change_password`, `rotate_session`
- `ensure_anonymous_session`, `authenticate_browser_form`, `authenticate_anonymous_csrf`, `guard_cookie_write`, `read_anonymous_state`, `write_anonymous_state`
- `authorize`, `ensure_role`, `grant_permission`, `revoke_permission`, `assign_role`, `revoke_role`
- `request_reset`, `exchange_token`, `complete_reset`
- `apply_migrations`
- `App::with_auth`, `App::register`, `App::register_policy`

`PublicUser` contains `id`, `email`, `is_active`, `is_staff` and `is_superuser`. No hash and no token.

## Data model

See [sessions.md](sessions.md) and [permissions.md](permissions.md). Authentication timestamps use Unix seconds (`i64`); model fields separately support the date and time types described in [date-time-types.md](date-time-types.md).

Email is trimmed and stored in lower case. It is unique, a maximum of 254 characters long and needs exactly one `@` and one point in the domain.

A new password has 12 to 128 characters. The hash uses Argon2id, version `0x13`, 19 456 KiB, two iterations and one thread. Salt and parameters are in the PHC string.

## Security mechanisms

- Unknown email, incorrect password and deactivated user deliver the same message `Invalid email or password`.
- If the user is missing, a fixed dummy hash is checked so that the Argon2 work is not eliminated.
- Deactivated users cannot log in. Deactivation via CLI or Admin immediately revokes the saved sessions.
- A password change through the CLI or admin revokes all user sessions. The admin uses `validate_new_password` and `hash_password`, never displays the hash and never puts the password in a redirect URL.
- The admin writes staff and role changes to the same user record. The next `authenticate_token` check reads the updated flags without deleting the session.
- Login is limited to five failed attempts per process in 15 minutes. The key is peer plus normalized email.
- `X-Forwarded-For` applies only if `FEXU_TRUST_PROXY` is expressly `1` or `true` and the immediate peer is listed in `FEXU_TRUSTED_PROXIES`. A `*` entry is ignored. The first hop is then evaluated.
- Security logs never include passwords, session tokens, anonymous tokens, login CSRF cookies or form tokens. `AnonymousSession`'s `Debug` output redacts the CSRF token and cookie. Database errors are still logged by the existing auth error path and can contain SQL text, not the raw cookie.
- Anonymous and authenticated sessions are different tables and cookies. A user cookie is never downgraded to an anonymous session. Login always creates a new user session id.
- Without `FEXU_ALLOWED_ORIGINS`, `AuthConfig::apply_bound_address` replaces the development allowlist with loopback origins for the bound address. Public hosts and request Host headers are never added. `Origin: null` is invalid except for the explicit same-origin browser case documented in [csrf.md](csrf.md). Requests without Origin or Referer still require the login CSRF cookie and token.

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
cargo run -p fexu-cli -- migrate up
cargo run -p fexu-cli -- admin create-user --email admin@example.com --superuser
cargo run -p fexu-cli -- auth list-users
```

`create-user` asks for the password hidden and confirms it. There is no `--password` and no preset admin password.

```http
POST /auth/login
{"email":"admin@example.com","password":"example-password"}
```

The response sets `Set-Cookie: fexu_session=...` and returns `user` and `csrf_token`. The session token is not included in the JSON.

## Tests

`crates/fexu-auth/src/password.rs`, `crates/fexu-auth/src/sqlite_tests.rs` and `examples/myshop/tests/auth.rs` run with SQLite. PostgreSQL is not required for this.

## Limitations

- The rate limit is in the process. Several processes do not share it. The map can be replaced later. The reset uses its own key and the same window.
- TOTP-based MFA with recovery codes and a login challenge is implemented. OIDC, OAuth and JWT authentication for external APIs are not implemented. Password reset is implemented but delivery is not guaranteed; see [auth providers](auth-providers.md).
- `is_active` is required for a valid session. Inactive accounts cannot log in.
- `is_staff` opens the administration and does not bypass model permissions.
- `is_superuser` bypasses roles and object rules, but does not replace `is_staff`. A superuser without staff receives 403 in the admin and can log out there.
- `fexu admin create-user` sets only the supplied flags and prints Active, Staff and Superuser status. Using `--superuser` without `--staff` leaves admin access disabled.

## Planned extensions

A distributed/multi-process rate limiter and OIDC/OAuth sign-in are not implemented. TOTP MFA, recovery codes and password reset are implemented; see [auth providers](auth-providers.md) and [auth threat model](auth-threat-model.md).

## Admin and CSRF Bootstrap

`GET /auth/csrf` returns the existing session’s CSRF token without rotating the session or invalidating other tabs; see [csrf.md](csrf.md). The admin requires `is_staff`. The directory API under `/admin/api/directory` and HTML pages under `/admin/directory/users` require staff superusers and cannot set `is_superuser`. See [admin-directory.md](admin-directory.md) and [admin-security.md](admin-security.md). `sqlite_admin_security_crud_and_csrf_bootstrap` covers the CSRF bootstrap.
