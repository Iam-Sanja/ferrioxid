# Admin-Sicherheit

[For overview](admin.md) · Auth: [auth.md](auth.md) · Policies: [policies.md](policies.md)]

## Zweck

The admin is a second door in front of the same users, sessions and rights. It does not loosen a public route and does not accept anonymous release. `GET /admin/api/config` is the only admin route without staff check and does not contain secrets. Colors and texts are checked before output. The directory remains staff and superuser. Password hashes do not leave the server. Error codes do not replace status codes and make wrong email and wrong password indistinguishable.

A child ID from the POST only applies if the record in the transaction belongs to the parent object. A foreign or manipulated foreign key is replaced or rejected. Rights are read again in the transaction. See [admin-transactional-inlines.md](admin-transactional-inlines.md)].

## Test per request

1. Session about `AuthService::authenticate`.
2. Active user.
3. `is_staff`.
4. Modellrecht `ressource.read|create|update|delete`.
5. Bound object rule for a single data set.
6. CSRF at any writing method.
7. With `/admin/pages/{key}` additionally the registered code via `AuthService::holds` before the trader runs.
8. For a separate collective action, the model right `update`, the object rule and the declared column list before a value is written.

An active superuser skips model rights when the resource is set. If `admin_permission` and `permissions` are empty, `permit` only allows superusers. The staff flag remains mandatory in both cases.

## CSRF-Bootstrap

`GET /auth/csrf` only works with valid session. Origin or referers must be allowed when they are sent. A missing origin is allowed because no open CORS policy exists. The call charges the same HMAC token again. Session tokens and expiration remain. Simultaneous tabs remain valid. Details are available in [csrf.md](csrf.md)].

The HTML login additionally checks `ferrioxid_login_csrf` against the form field. The permitted origins follow the bind address and not a hardwired port 3000. The host header is not read for this. Logs contain the basic code, not the cookie and not the token.

The token is not in `localStorage` and not in the URL. The session cookie remains `HttpOnly`.

## Lists and foreign keys

Sort, search and filter fields are from `AdminModelMeta`. In addition, identifiers must correspond to `^[A-Za-z_][A-Za-z0-9_]*$` and a real column. Values are bound parameters.

A bound object rule filters List, Detail, PATCH, DELETE, Form Select, and Selection Page via `QueryScope`. `total` uses the same filter. A single rejected record is 404. An unbound rule remains 403 with `object policy is not bound`. The select loads a maximum of 100 lines. The selection page uses `admin_per_page` of the target, capped at 100. A UUID that the user is not allowed to read at the registered destination is rejected before writing. `save` can only be `detail`, `edit` or `new`. The destination address is built by the server. See [query-scopes.md](query-scopes.md) and [admin-relational-widgets.md](admin-relational-widgets.md)].

## Privilegieneskalation

Normal model rights do not open the directory. Superuser forgiveness is blocked. A user cannot remove their own protective flags. Sensitive names (`password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`) must be hidden in the admin and removed from responses.

## Abmelden

`POST /admin/logout` checks the session and the CSRF field. `is_staff` is not a condition for this. The session is revoked and the session cookie is emptied. The answer is 303 on `/admin/login`. A missing or foreign token allows the session to pass.

`require_get` unterscheidet:

- no session: forwarding for registration
- Session without staff: HTML 403 with the text “Management requires staff access.” and unsubscription form, without sidebar and without model data
- Staff: the requested page
- Staff without model rights or without superuser for the directory: HTML 403 “This action is not allowed.”

The JSON API under `/admin/api` continues to respond with plain text and is not a logout form. `html_admin_works_without_javascript` checks the HTML cases.

Session error pages solve `is_superuser` using the same authentication as the rest of the area. The navigation does not derive any rights from this. A staff superuser also sees the directory link on a 403 page. A staff without a superuser does not see him. Without staff, the sidebar remains, the deregistration form remains.

HTML directory actions that show the check mark call `require_confirmation`. Only `confirm=1` is valid. The JSON directory API does not have this field. She remains tied to Session, Staff Superuser and CSRF.

## Tests

The JSON test checks anonymous admin request, missing staff, unknown model, incorrect sorting, incorrect filter, read-only `id`, object scope, missing CSRF, same token in two fetches, a non-session token and the superuser lock. `admin_html.rs` checks the same rules via forms, open forwards and escaptes HTML. Assets reviews `admin_assets_reject_traversal_and_keep_the_api`.

## Grenzen

The login limit is process-local. Anonymous demo writing routes outside the admin remain without CSRF, as documented in v0.7. The filter decoding is not completely UTF-8.

## Drafts, Collective Actions and History

A draft form belongs to `session_id` and `user_id`. The ID is a UUID. The payload contains only allowed model fields, never password, hash, token or CSRF. A foreign user and an expired ID do not read anything. The final POST validates and re-checks the policy.

A collective action accepts only one registered name. The first POST request saves the IDs and does not write a model line. `confirm=1` is missing: no change. Before writing, the transaction reads each ID with write scope and object rule. A missing, foreign or double-rejected ID cancels the entire selection. `delete` and `SetBool` run in the same transaction as their audit lines. A foreign key error rolls everything back. The selection is capped on `max_ids`, built-in actions on 100. `scope=page` ignores sent IDs and reads the current page again.

History does not store sensitive values. A staff without a superuser only sees events with their own `actor_id`. Directory events occur after the Auth writing process and do not share a transaction with it because the Auth service uses the pool.

## Later

A directory administrator separate from the superuser. OAuth, MFA and individually revocable CSRF tokens per tab are planned and not implemented.
