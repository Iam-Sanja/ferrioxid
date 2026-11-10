# CSRF for multiple admin tabs

[Overview](index.md) · [sessions.md](sessions.md) · [admin-security.md](admin-security.md)

## Purpose

Writing cookie requests need a CSRF token that is tied to the valid session. A retrieval in a second browser tab must not revoke the write permission of the first tab.

## Architecture

The token is an HMAC-SHA256 via the fixed prefix `ferrioxid-csrf-v1`, the session ID and the SHA-256 hex of the session token. The key is 32 bytes. Therefore, the same session always results in the same token. `GET /auth/csrf` does not write the database. Since v0.12, `authenticate_csrf` accepts the same token from an HTML field. `authenticate` continues to read only `x-csrf-token`. `App::into_router` installs a default cookie-write layer before `with_state`. It covers merged routes, auth and admin routes, and generated JSON routes, including `allow_anonymous`. POST, PUT, PATCH and DELETE that send the configured session cookie or `ferrioxid_anonymous` need a matching CSRF proof. A non-empty `x-csrf-token` header is used as that proof. If the header is empty, the layer reads `csrf_token` from `application/x-www-form-urlencoded` or `multipart/form-data` and then restores the body. JSON clients send the header; a `csrf_token` property inside JSON is not a proof, because it would also be an unknown model field. `guard_cookie_write` returns `Stateless` when neither session cookie is present, including when `Authorization: Bearer` is set. The bearer value is not validated. Cookie-less public writes stay open. A present session cookie is still checked. Without an `AuthService`, the default cookie names `ferrioxid_session` and `ferrioxid_anonymous` fail closed.

Logout, password change and expiration revoke or discard the session. After that, the check fails before the token is even compared. A token of another session does not fit because ID and token hash enter the HMAC.

## Responsible file

- `crates/ferrioxid-auth/src/service.rs`
- `crates/ferrioxid-auth/src/config.rs`
- `crates/ferrioxid-auth/src/routes.rs`
- `crates/ferrioxid-auth/src/cookie.rs`
- `crates/ferrioxid-admin/src/html.rs`
- `crates/ferrioxid-api/src/cookie_guard.rs`

## Public APIs

- `AuthConfig.csrf_secret`
- `AuthConfig.anonymous_cookie_name` (`ferrioxid_anonymous`)
- `AuthService::ensure_anonymous_session`
- `AuthService::authenticate_browser_form`
- `AuthService::authenticate_anonymous_csrf`
- `AuthService::guard_cookie_write`
- `AuthService::login_replacing`
- `GET /auth/anonymous`
- `AuthService::refresh_csrf`
- `GET /auth/csrf` returns `{ "csrf_token": "..." }`
- Header `x-csrf-token` for unsafe JSON methods
- HTML field `csrf_token` with the same HMAC
- Cookie `ferrioxid_login_csrf` only for `POST /admin/login`. It is 64 Hex characters, HttpOnly, `Path=/`, SameSite from the Auth configuration, `Secure` only at `cookie_secure` and `Max-Age=600`. A valid cookie is reused for other tabs. An existing `ferrioxid_anonymous` or `ferrioxid_session` cookie does not replace this check: the global cookie guard accepts the login double-submit and still rejects a missing or mismatched login token. The anonymous CSRF token is not a login proof. A successful login revokes the anonymous session. `POST /auth/password-reset` and `POST /auth/password-reset/choose` use the same separation for `ferrioxid_reset_csrf`. JSON `POST /auth/login` has no login cookie; with an anonymous cookie it still needs the anonymous CSRF proof, and without a session cookie it stays a stateless credential request.

## Data flow

1. Login creates the session and calculates the token.
2. The database stores only `digest(token)` in `csrf_token_hash`, not the raw token.
3. Each retrieval recalculates the same token.
4. `authenticate` opens the session and compares the header in constant time.

## Security rules

- Tokens are not in URLs and are not logged. A rejected HTML login internally only calls `origin_rejected`, `form_token_missing`, `login_csrf_missing` or `login_csrf_mismatch`.
- If the browser sends `Origin` or `Referer`, the value must match exactly the permitted origins. The list comes from `FERRIOXID_ALLOWED_ORIGINS` or the loopback bind address, not the host header. `http://127.0.0.1:3000` only applies if the process is really tied to port 3000 or the variable calls it.
- `Referrer-Policy: no-referrer` allows browsers to send `Origin: null` to an HTML form POST, even if the page is same-origin. This only applies if `Sec-Fetch-Site: same-origin` is additionally available. One side cannot set this header itself. `null` with `cross-site`, `same-site` or without this header remains rejected. `null` is not on the Allowlist.
- The session cookie remains `HttpOnly`.
- The comparison uses `constant_eq`.
- Development uses the fixed key `ferrioxid-dev-csrf-key-32-bytes!`.
- `FERRIOXID_CSRF_SECRET` is 64 hex characters. Otherwise, Ferrioxid reads `.ferrioxid/csrf.key` and creates the file once if it is missing. Production should set the environment variable so that a restart does not invalidate all sessions.
- Anonymous browser forms that call `authenticate_browser_form` use the same HMAC. The cookie is `ferrioxid_anonymous`: HttpOnly, `Path=/`, no `Domain`, SameSite from the auth configuration, and `Secure` when `cookie_secure` is set. The raw token is not returned as JSON and is not written to the auth error text.
- `GET /auth/anonymous` reuses or creates that session and returns only `csrf_token`. It does not authenticate a user.
- When `cookie_secure` is true, unsafe cookie requests must present `Origin` or `Referer`. The value must be `https` and on `FERRIOXID_ALLOWED_ORIGINS`, except the existing `Origin: null` plus `Sec-Fetch-Site: same-origin` case. HTTP origins are rejected in that mode. Development with `cookie_secure = false` still allows a missing origin.
- Login revokes the presented anonymous row and sets a new `ferrioxid_session`. The old anonymous token and its CSRF value do not authenticate the new session. Logout of an anonymous cookie requires its CSRF token.

## Error cases

| Case | Response |
|---|---|
|Header missing or token is wrong| 403, `csrf token is missing or invalid` |
|Token of another session| 403 |
|Session expired, revoked or user inactive on a safe method| 401 |
|Unsafe method with an expired or unknown session cookie| 403 before the handler |
|Logout without tokens|403, the session remains|
|Session cookie without a valid CSRF proof|403 before the handler|
|Default session cookie and no `AuthService`|403|
|No session cookie, including `Authorization: Bearer`|Not blocked by this layer|

## Example

The HTTP example uses the default `ferrioxid_session` cookie name.

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

`examples/myshop/tests/admin_http.rs` checks two retrievals, a successful JSON write access, a fake token and a non-session token. `examples/myshop/tests/admin_html.rs` checks the login cookie, a wrong token, the session field, logout and an expired session. `html_login_csrf_respects_the_bound_port` checks `127.0.0.1` and `localhost` on the bound port, the old port 3000 as a foreign origin, missing and manipulated cookie, missing and incorrect form field, a foreign origin, two tabs, login, logout and login again. `examples/myshop/tests/auth.rs` checks the JSON logout. `examples/myshop/tests/cookie_csrf.rs` checks POST, PUT, PATCH and DELETE on a handler that does not call a guard, a generated public product route, form and multipart fields, bearer-only clients, a renamed session cookie and a router without `AuthService`. JSON `POST /auth/login` does not require CSRF when no session cookie is present. A present session cookie is checked.

## Limitations

All tabs in a session share a token. Logout therefore ends all tabs of this session. There is no token list with single revocation. The key in `.ferrioxid/csrf.key` is in the working directory and is only the development source if no environment variable is set. The layer reads at most 8 MiB when it has to inspect a body, and only when a session cookie is present and `x-csrf-token` is empty. Routes added after `into_router` are outside the layer. `ferrioxid_login_csrf`, `ferrioxid_reset` and `ferrioxid_locale` do not start it. Development with `cookie_secure = false` still allows a missing origin.
