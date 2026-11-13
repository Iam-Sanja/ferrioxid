# Admin security

[Overview](admin.md) · Auth: [auth.md](auth.md) · Policies: [policies.md](policies.md)

## Purpose

The admin uses the existing users, sessions and permissions with an additional staff-access check. It does not weaken public-route policies or accept anonymous admin access. `GET /admin/api/config` is the only admin endpoint without a staff check and contains no secrets. Branding colors and text are validated before output. The directory requires both staff and superuser access. Password hashes never leave the server. Stable error codes supplement HTTP status codes; invalid email and invalid password responses remain indistinguishable.

A child ID from the POST only applies if the record in the transaction belongs to the parent object. A foreign or manipulated foreign key is replaced or rejected. Rights are read again in the transaction. See [admin-transactional-inlines.md](admin-transactional-inlines.md).

## Test per request

1. Session about `AuthService::authenticate`.
2. Active user.
3. `is_staff`.
4. Model permission `ressource.read|create|update|delete`.
5. Bound object rule for a single record.
6. CSRF at any writing method.
7. With `/admin/pages/{key}` additionally the registered code via `AuthService::holds` before the handler runs.
8. For a separate bulk action, the model right `update`, the object rule and the declared column list before a value is written.

An active superuser skips model permissions when the resource is set. If `admin_permission` and `permissions` are empty, `permit` only allows superusers. The staff flag remains mandatory in both cases.

## CSRF bootstrap

`GET /auth/csrf` requires a valid session. If present, Origin or Referer must be allowed. A missing origin is accepted without enabling CORS. The endpoint returns the same HMAC token without changing the session token or expiry, so other tabs remain valid. See [csrf.md](csrf.md).

The HTML login additionally checks `fexu_login_csrf` against the form field. That check still runs when `fexu_anonymous` or `fexu_session` is already present. The global cookie guard validates the login double-submit on `POST /admin/login` and does not accept the anonymous session token instead. The permitted origins follow the bind address and not a hardwired port 3000. The host header is not read for this. Logs contain the basic code, not the cookie and not the token.

The token is not in `localStorage` and not in the URL. The session cookie remains `HttpOnly`.

## Lists and foreign keys

Sort, search and filter fields are from `AdminModelMeta`. In addition, identifiers must correspond to `^[A-Za-z_][A-Za-z0-9_]*$` and a real column. Values are bound parameters.

A bound object rule filters List, Detail, PATCH, DELETE, Form Select, and Selection Page via `QueryScope`. `total` uses the same filter. A single rejected record is 404. An unbound rule remains 403 with `object policy is not bound`. The select loads a maximum of 100 lines. The selection page uses `admin_per_page` of the target, capped at 100. A UUID that the user is not allowed to read at the registered destination is rejected before writing. `save` can only be `detail`, `edit` or `new`. The destination address is built by the server. See [query-scopes.md](query-scopes.md) and [admin-relational-widgets.md](admin-relational-widgets.md).

## Privilege escalation

Model permissions do not grant directory access, and the directory cannot grant or modify superuser status. Self-deactivation and removal of one’s own staff status are blocked. Sensitive names (`password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`) must be hidden in the admin and excluded from responses.

## Logout

`POST /admin/logout` checks the session and CSRF field without requiring `is_staff`. It revokes the session, clears the cookie and redirects with 303 to `/admin/login`. A missing or incorrect CSRF token rejects logout and leaves the session valid.

`require_get` distinguishes the following cases:

- No session: redirect to the login page.
- Session without staff: HTML 403 with the text “Management requires staff access.” and logout form, without sidebar and without model data
- Staff: the requested page
- Staff without model permissions or without superuser for the directory: HTML 403 “This action is not allowed.”

The JSON API under `/admin/api` returns error responses rather than HTML logout forms. `html_admin_works_without_javascript` covers the HTML behavior.

Error pages obtain `is_superuser` through the same authentication checks as the rest of the admin. Navigation grants no permissions. Staff superusers see the directory link even on a 403 page; other staff do not. Users without staff status receive a logout form without a sidebar.

HTML directory actions requiring confirmation call `require_confirmation` and accept only `confirm=1`. The JSON directory API has no confirmation field; it still requires a session, staff superuser status and CSRF.

## Tests

JSON tests cover anonymous requests, missing staff status, unknown models, invalid sorting and filters, read-only `id`, object scopes, missing CSRF, repeated CSRF retrieval, invalid tokens and blocked superuser changes. `admin_html.rs` checks the equivalent form flows, redirect validation and HTML escaping. `admin_assets_reject_traversal_and_keep_the_api` covers asset handling.

## Limitations

The login limit is process-local. Anonymous demo writing routes outside the admin remain without CSRF, as documented in v0.7. The filter decoding is not completely UTF-8.

## Drafts, bulk actions and history

A draft form belongs to `session_id` and `user_id`. The ID is a UUID. The payload contains only allowed model fields, never password, hash, token or CSRF. A foreign user and an expired ID do not read anything. The final POST validates and re-checks the policy.

A bulk action accepts only one registered name. The first POST request saves the IDs and does not write a model line. `confirm=1` is missing: no change. Before writing, the transaction reads each ID with write scope and object rule. A missing, foreign or double-rejected ID cancels the entire selection. `delete` and `SetBool` run in the same transaction as their audit lines. A foreign key error rolls everything back. The selection is capped on `max_ids`, built-in actions on 100. `scope=page` ignores sent IDs and reads the current page again.

History does not store sensitive values. A staff without a superuser only sees events with their own `actor_id`. Directory events occur after the Auth writing process and do not share a transaction with it because the Auth service uses the pool.

## Later

A directory administrator separate from the superuser. OAuth, MFA and individually revocable CSRF tokens per tab are planned and not implemented.
