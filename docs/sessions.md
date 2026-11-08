# Sessions and Cookies

[Overview](index.md) · [auth.md](auth.md)

## Purpose

The admin and browser login uses server-side sessions. JSON Web Tokens are not the standard.

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

## Data model: `_ferrum_sessions`

| Column | Meaning |
|---|---|
| `id` | UUID |
| `user_id` | Foreign key to `_ferrum_users`, `ON DELETE CASCADE` |
| `token_hash` |SHA-256-hex, unique|
| `csrf_token_hash` | SHA-256 hex digest of the CSRF token |
| `created_at`, `expires_at`, `last_seen_at` | Unix seconds |
| `revoked_at` |set at logout, rotation and revocation of all sessions|

Standard life: 12 hours absolute, 30 minutes without use.

## Security mechanisms

The default cookie name remains `ferrum_session` for compatibility. It uses `HttpOnly`, `Path=/`, `SameSite=Lax` and no `Domain` attribute. `Secure` follows `FERRUM_COOKIE_SECURE`, or `FERRUM_ENV=production` when the former is unset. `BIND_ADDRESS` does not affect this setting. Local HTTP works in development without `Secure`; production logs a warning if `Secure` is disabled.

Logout uses the same cookie attributes and `Max-Age=0`.

Tokens are not in URLs. The database row does not contain the raw token.

## Error cases

Expired, revoked, unknown and too long unused tokens result in `Unauthenticated`. A disabled user as well, even if the line were still valid.

## Example

The preserved HTTP example uses `ferrioxid_session` as a placeholder cookie name. Replace it with the default `ferrum_session` or your configured cookie name; it is not an alias.

```http
POST /auth/logout
Cookie: ferrioxid_session=...
x-csrf-token: ...
```

Answer 204 and `Set-Cookie` with empty value.

## Tests

`sqlite_tests` checks attachment, hash, expiration, idle timeout, revocation, rotation, inactive users and the secure cookie. `examples/myshop/tests/auth.rs` checks missing, manipulated and expired cookie and logout.

## Limitations

Idle timeout and absolute runtime are checked on the next request, not by a background job. A renewed session takes over the original `expires_at`.

## Planned extensions

Device management and a session list are not implemented. `GET /auth/csrf` recalculates the same token and does not rewrite the session. Multiple tabs remain valid. Logout, password change and expiration make the token unusable because the session itself ends. The automatic rotation per request does not occur.
