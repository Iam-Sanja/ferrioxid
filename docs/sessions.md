# Sessions and Cookies

[Overview](index.md) · [auth.md](auth.md)

## Purpose

The admin and browser login uses server-side sessions. Anonymous visitors can have a separate server-side session. JSON Web Tokens are not the standard. The cookie value is an unguessable token. Ferrioxid stores only its SHA-256 hex digest and does not treat the client value as a session id.

## Architecture

Login generates a 32-byte session token with `OsRng`. The CSRF token is derived from the session ID and token hash using HMAC; see [csrf.md](csrf.md). The database stores SHA-256 digests as hex strings. Every successful authentication updates `last_seen_at` without extending `expires_at`.

`rotate_session` revokes the current session row and creates a new one with the same `expires_at`. It never renews an expired session. Login always creates a new session. Automatic rotation on every request is disabled because the CSRF token would otherwise change without a reliable channel to update clients.

## Responsible files

- `crates/ferrioxid-auth/src/service.rs`
- `crates/ferrioxid-auth/src/cookie.rs`
- `crates/ferrioxid-auth/src/config.rs`
- `crates/ferrioxid-auth/migrations/0001_auth_schema.json`

## Public API

`AuthService::login` returns an `IssuedSession` containing the cookie header, raw session token, CSRF token and `PublicUser`. HTTP handlers set the session cookie and return `csrf_token` and `user` in the login JSON. The raw session token is never returned in JSON.

`logout` revokes the session if the token is still known and deletes the cookie. Without a cookie, the HTTP 204 call remains. With a valid cookie, Logout requires the CSRF token.

## Data model: `_ferrioxid_sessions`

| Column | Meaning |
|---|---|
| `id` | UUID |
| `user_id` | Foreign key to `_ferrioxid_users`, `ON DELETE CASCADE` |
| `token_hash` |SHA-256-hex, unique|
| `csrf_token_hash` | SHA-256 hex digest of the CSRF token |
| `created_at`, `expires_at`, `last_seen_at` | Unix seconds |
| `revoked_at` |set at logout, rotation and revocation of all sessions|

Standard life: 12 hours absolute, 30 minutes without use.

## Security mechanisms

The default session cookie is `ferrioxid_session`. It uses `HttpOnly`, `Path=/`, `SameSite=Lax` and no `Domain` attribute. `Secure` follows `FERRIOXID_COOKIE_SECURE`, or `FERRIOXID_ENV=production` when the former is unset. `BIND_ADDRESS` does not affect this setting. Local HTTP works in development without `Secure`; production logs a warning if `Secure` is disabled.

Logout uses the same cookie attributes and `Max-Age=0`.

Tokens are not in URLs. The database row does not contain the raw token.

## Error cases

Expired, revoked, unknown and too long unused tokens result in `Unauthenticated`. A disabled user as well, even if the line were still valid.

## Example

The HTTP example uses the default `ferrioxid_session` cookie name.

```http
POST /auth/logout
Cookie: ferrioxid_session=...
x-csrf-token: ...
```

Answer 204 and `Set-Cookie` with empty value.

## Tests

`sqlite_tests` checks attachment, hash, expiration, idle timeout, revocation, rotation, inactive users and the secure cookie. `examples/myshop/tests/auth.rs` checks missing, manipulated and expired cookie and logout.

## Anonymous sessions

`_ferrioxid_anonymous_sessions` stores an opaque id, the token hash, the CSRF hash, optional flat JSON state, and the same expiry columns. There is no `user_id`. `ensure_anonymous_session` creates a row when the cookie is missing, unknown, revoked, expired, or idle. A later GET can replace a bad cookie. A state-changing call does not.

State is a JSON object of at most 32 scalar values, stored as text and bound as a parameter. It is not copied into the user session at login. Login revokes the anonymous row in the same transaction that inserts the new user session. That blocks session fixation of the anonymous id. `authenticate_token` does not accept an anonymous token.

Absolute lifetime and idle timeout are `FERRIOXID_SESSION_HOURS` and `FERRIOXID_IDLE_MINUTES`, the same values as user sessions. The anonymous cookie name is not configurable. It must stay distinct from `ferrioxid_session`.

## Limitations

Idle timeout and absolute runtime are checked on the next request, not by a background job. A renewed session takes over the original `expires_at`.

A protected POST that still carries an invalid user cookie stays unauthenticated. Do not treat that request as anonymous. The recovery sequence is: GET the login page, detect the invalid session, clear `ferrioxid_session`, let `ensure_anonymous_session` set the anonymous cookie and CSRF, then POST the login with that token and a listed `Origin`. `browser_login_flow_exposes_cookies_and_rejects_bad_sessions` in `crates/ferrioxid-api/src/testing.rs` is the executable example.

## Planned extensions

Device management and a session list are not implemented. `GET /auth/csrf` recalculates the same token and does not rewrite the session. Multiple tabs remain valid. Logout, password change and expiration make the token unusable because the session itself ends. The automatic rotation per request does not occur.
