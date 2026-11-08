# CSRF for multiple admin tabs

[Zur Übersicht](index.md) · [sessions.md](sessions.md) · [admin-security.md](admin-security.md)

## Zweck

Writing cookie requests need a CSRF token that is tied to the valid session. A retrieval in a second browser tab must not revoke the write permission of the first tab.

## Architektur

The token is an HMAC-SHA256 via the fixed prefix `ferrioxid-csrf-v1`, the session ID and the SHA-256 hex of the session token. The key is 32 bytes. Therefore, the same session always results in the same token. `GET /auth/csrf` does not write the database. Since v0.12, `authenticate_csrf` accepts the same token from an HTML field. `authenticate` continues to read only `x-csrf-token`. A request without a session cookie remains for public forms without this check.

Logout, password change and expiration revoke or discard the session. After that, the check fails before the token is even compared. A token of another session does not fit because ID and token hash enter the HMAC.

## Responsible file

- `crates/ferrioxid-auth/src/service.rs`
- `crates/ferrioxid-auth/src/config.rs`
- `crates/ferrioxid-auth/src/routes.rs`
- `crates/ferrioxid-auth/src/cookie.rs`
- `crates/ferrioxid-admin/src/html.rs`

## Public APIs

- `AuthConfig.csrf_secret`
- `AuthService::refresh_csrf`
- `GET /auth/csrf` liefert `{ "csrf_token": "..." }`
- Header `x-csrf-token` for unsafe JSON methods
- HTML field `csrf_token` with the same HMAC
- Cookie `ferrioxid_login_csrf` only for `POST /admin/login` before a session exists. It is 64 Hex characters, HttpOnly, `Path=/`, SameSite from the Auth configuration, `Secure` only at `cookie_secure` and `Max-Age=600`. A valid cookie is reused for other tabs.

## Datenfluss

1. Login creates the session and calculates the token.
2. The database stores only `digest(token)` in `csrf_token_hash`, not the raw token.
3. Each retrieval recalculates the same token.
4. `authenticate` opens the session and compares the header in constant time.

## Sicherheitsregeln

- Tokens are not in URLs and are not logged. A rejected HTML login internally only calls `origin_rejected`, `form_token_missing`, `login_csrf_missing` or `login_csrf_mismatch`.
- If the browser sends `Origin` or `Referer`, the value must match exactly the permitted origins. The list comes from `FERRUM_ALLOWED_ORIGINS` or the loopback bind address, not the host header. `http://127.0.0.1:3000` only applies if the process is really tied to port 3000 or the variable calls it.
- `Referrer-Policy: no-referrer` allows browsers to send `Origin: null` to an HTML form POST, even if the page is same-origin. This only applies if `Sec-Fetch-Site: same-origin` is additionally available. One side cannot set this header itself. `null` with `cross-site`, `same-site` or without this header remains rejected. `null` is not on the Allowlist.
- The session cookie remains `HttpOnly`.
- The comparison uses `constant_eq`.
- Development uses the fixed key `ferrioxid-dev-csrf-secret-32-bytes!`.
- `FERRUM_CSRF_SECRET` is 64 hex characters. Otherwise, Ferrioxide reads `.ferrum/csrf.key` and creates the file once if it is missing. Production should set the environment variable so that a restart does not invalidate all sessions.
- Anonymous demo writing routes do not check CSRF. They are not attached to the cookie.

## Error cases

| Fall | Antwort |
|---|---|
|Header missing or token is wrong| 403, `csrf token is missing or invalid` |
|Token of another session| 403 |
|Session expired, revoked or user inactive| 401 |
|Logout without tokens|403, the session remains|

## Example

```rust
let token = auth.refresh_csrf(&session_token).await?;
let again = auth.refresh_csrf(&session_token).await?;
assert_eq!(token, again);
```

```http
GET /auth/csrf
Cookie: ferrioxid_session=...

POST /admin/api/models/admin_products/records
Cookie: ferrioxid_session=...
x-csrf-token: ...
```

## Tests

`examples/myshop/tests/admin_http.rs` checks two retrievals, a successful JSON write access, a fake token and a non-session token. `examples/myshop/tests/admin_html.rs` checks the login cookie, a wrong token, the session field, logout and an expired session. `html_login_csrf_respects_the_bound_port` checks `127.0.0.1` and `localhost` on the bound port, the old port 3000 as a foreign origin, missing and manipulated cookie, missing and incorrect form field, a foreign origin, two tabs, login, logout and login again. `examples/myshop/tests/auth.rs` checks the JSON logout. JSON `POST /auth/login` does not require a CSRF cookie.

## Limitations

All tabs in a session share a token. Logout therefore ends all tabs of this session. There is no token list with single revocation. The key in `.ferrum/csrf.key` is in the working directory and is only the development source if no environment variable is set.
