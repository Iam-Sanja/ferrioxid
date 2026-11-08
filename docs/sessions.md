# Sessions and Cookies

[For overview](index.md) · [auth.md](auth.md)]

## Zweck

The admin and browser login uses server-side sessions. JSON Web Tokens are not the standard.

## Architektur

The login generates 32 bytes from `OsRng` for the session token. The CSRF token is an HMAC via session ID and token hash, see [csrf.md](csrf.md)]. The database stores SHA-256 as a hex. `last_seen_at` is set at every successful exam. `expires_at` will not be renewed.

`rotate_session` revokes the current line and creates a new one with the same `expires_at`. A meeting that has already expired will not be renewed. The login always creates a new session. An automatic rotation at each request is switched off because the CSRF token would change without a reliable channel.

## Responsible files

- `crates/ferrioxid-auth/src/service.rs`
- `crates/ferrioxid-auth/src/cookie.rs`
- `crates/ferrioxid-auth/src/config.rs`
- `crates/ferrioxid-auth/migrations/0001_auth_schema.json`

## Public API

`AuthService::login` delivers `IssuedSession` with cookie line, raw token, CSRF token and `PublicUser`. HTTP handlers only set the cookie and return tokens plus users in login JSON as `csrf_token` and `user`. Subsequent replies do not contain session tokens.

`logout` revokes the session if the token is still known and deletes the cookie. Without a cookie, the HTTP 204 call remains. With a valid cookie, Logout requires the CSRF token.

## Datenmodell `_ferrum_sessions`

| Spalte | Bedeutung |
|---|---|
| `id` | UUID |
| `user_id` | Foreign Key auf `_ferrum_users`, `ON DELETE CASCADE` |
| `token_hash` |SHA-256-hex, unique|
| `csrf_token_hash` | SHA-256-Hex des CSRF-Tokens |
| `created_at`, `expires_at`, `last_seen_at` | Unix-Sekunden |
| `revoked_at` |set at logout, rotation and revocation of all sessions|

Standard life: 12 hours absolute, 30 minutes without use.

## Security mechanisms

The cookie is called `ferrioxid_session`. It is `HttpOnly`, `Path=/`, `SameSite=Lax` and without `Domain`. `Secure` stands only if `cookie_secure` is set. This is decided by `FERRUM_COOKIE_SECURE` or, if the variable is missing, `FERRUM_ENV=production`. `BIND_ADDRESS` does not matter. Local HTTP remains usable in development without `Secure`. Production without `Secure` writes a warning.

Logout uses the same cookie attributes and `Max-Age=0`.

Tokens are not in URLs. The database row does not contain the raw token.

## Error cases

Expired, revoked, unknown and too long unused tokens result in `Unauthenticated`. A disabled user as well, even if the line were still valid.

## Example

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
