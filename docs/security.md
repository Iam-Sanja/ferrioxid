# Security – threat model and security boundaries

[Overview](index.md) · **Implemented security mechanisms and required safeguards. Fexu `0.1.0` is pre-release and is not ready for unrestricted production use.**

## Implemented protection mechanisms

- Models without anonymous access or a configured permission policy receive 403 handlers for GET, POST, PATCH and DELETE, using the `ApiError` contract.
- Create and update DTOs validate declared fields. `deny_unknown_fields` rejects auto-generated IDs and fields excluded from the relevant DTO. Read-only fields remain writable at creation; managed timestamps are excluded from both DTOs.
- INSERT, UPDATE and DELETE use bound parameters through Fexu's SQLite/rusqlite or PostgreSQL/SQLx driver. The DELETE and UPDATE statements always contain `WHERE "id" = $1`. Table and column names come only from checked macro attributes.
- `ApiError` logs DB details internally and delivers only generic information to the outside world at 500. Since v0.11, the same answer has a stable `code`. SQL and hashes are also replaced if a free text contained them. HTML error pages use the same rule: a 500 response and the built-in fallback do not include stack traces, SQL or the template error chain.
- Axum’s JSON-Extractor uses Serde DTOs with `deny_unknown_fields` on the Create type as standard.
- Demo bind is `127.0.0.1:3000`; Docker Postgres port is published on `127.0.0.1`.
- `can_create` checks the Create input before the INSERT. A foreign owner ID is rejected if the rule requires it. The standard of the method is `false`. Generated projects do not bind to `0.0.0.0` and do not contain a standard password.
- Since v0.12, static files remain in the configured root. Uploads use UUID names, check the file contents and belong to the logged-in user. The request ID comes from the server. Logs do not contain a query string. Details are available in [staticfiles.md](staticfiles.md), [uploads.md](uploads.md) and [middleware.md](middleware.md).

## Authentication (actual, v0.7)

Users, Argon2id, server-side sessions, anonymous server-side sessions, HttpOnly cookies, CSRF for cookie write accesses, roles and model permissions are implemented. Anonymous sessions and their CSRF checks are on `main` after PR #62 and remain Partial: cookie-less `allow_anonymous` writes stay open. Details are available in [auth.md](auth.md), [sessions.md](sessions.md), [csrf.md](csrf.md), [permissions.md](permissions.md) and [policies.md](policies.md).

`allow_anonymous = true` remains an explicit public release. Product and category in myshop are such demo endpoints. Protected routes refuse without session and right.

## Remaining gaps (actual)

OAuth, MFA and JWT for external APIs remain unimplemented; password reset is available as described in [password-reset.md](password-reset.md). Login rate limits are process-local. Object-filtered lists require explicit [query scopes](query-scopes.md). `App::into_router` rejects cookie-authenticated POST, PUT, PATCH and DELETE requests unless the session CSRF check passes. `POST /admin/login` is the exception that still requires its own `fexu_login_csrf` double-submit when an anonymous or user session cookie is present; a missing login token stays forbidden and the anonymous session is revoked only after a successful login. Password-reset forms use `fexu_reset_csrf` the same way. That layer covers generated `allow_anonymous` JSON routes and handlers that forget `guard_cookie_write`. A request with neither `fexu_session` nor `fexu_anonymous` stays unauthenticated, including `Authorization: Bearer`; the bearer value is not validated. `allow_anonymous` therefore still permits a cookie-less write. A browser route that must refuse that bypass layers `fexu::require_browser`; a bearer header is not treated as a session. myshop `/contact` rejects every POST when no `AuthService` is configured. `IssuedSession`, `IssuedExchange` and `AuthConfig` debug output redacts the raw session token, CSRF token, cookie and CSRF key. Concurrent PATCH requests have no version field to detect lost updates. Public deployment still requires appropriate proxy and operational controls. When `Secure` is on, an unsafe cookie request without `Origin` or `Referer` is rejected. The proxy must forward those headers. `X-Forwarded-For` is used only for the login rate-limit key when `FEXU_TRUST_PROXY` is set. It is not an origin. Routes attached after `into_router` are outside this layer. Development with `cookie_secure = false` still allows a missing origin.

## Planned threat model

| Threat |Target action| Evidence |
|---|---|---|
| Unauthorized reads/writes | Deny by default and server-side policies | 401/403 tests |
| IDOR | Check each object policy after authenticating the caller | Cross-user access tests |
| Mass assignment | Never expose shared database models directly as writable DTOs | Field-masking tests |
| CSRF | Protect cookie-authenticated state changes | Cross-origin tests |
| SQL injection | Bound values and validated identifiers | Fuzz and negative tests |
| Brute force | Rate limits, risk-based lockout and auditing | Login stress tests |
| Session theft | HttpOnly, Secure, SameSite, rotation and expiry | Session tests |
| Secrets in logs | Redaction and generic API errors | Log review |
| Public demo exposure | Bind and configuration checks; no production defaults | Deployment tests |

## Implementation targets (planned)

- `auth` and policies are separate concepts: authentication determines identity, authorization decides access.
- RBAC alone is not sufficient for object-level/field-level policies.
- By default, no anonymous CRUD and no automatically exported sensitive fields.
- The interface may only display permissions. The server verification remains relevant.
- Auth context must not be created via client fields or headers with an unchecked trust chain.
- Before each productive release security review, dependency audit and documented threat model assumptions.

**Note:** `SECURITY.md` in the repository currently contains only a brief project information, not an established vulnerability disclosure process. It must be maintained for public releases.

`FexuError::Database` displays `database operation failed`. `fexu_core::diagnostic_chain` joins the source chain for server logs and redacts values after `password=`, `secret=`, `token=`, `csrf_token=`, `fexu_session=` and `fexu_anonymous=`. Do not send that string to the browser.

An invalid user cookie on an unsafe browser POST is not replaced by an anonymous session. Clear the cookie, call `ensure_anonymous_session` on the next GET, and post the new anonymous CSRF. The executable example is `browser_login_flow_exposes_cookies_and_rejects_bad_sessions`.

## Admin

The admin checks session, active user, staff, model right, object rule and write access CSRF. Path parameters are not unchecked to SQL identifiers. Sorting, searching and filtering are whitelists. Details and known limits are given in [admin-security.md](admin-security.md).
