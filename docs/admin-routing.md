# Admin routes

[Overview](admin-native.md) · Security: [admin-security.md](admin-security.md) · CSRF: [csrf.md](csrf.md)

## HTML

| Method | Path | Effect |
|---|---|---|
| GET | `/admin`, `/admin/` | Dashboard; requests without a session redirect to login. |
| GET | `/admin/login` | Render the form; create or reuse `ferrioxid_login_csrf`. |
| POST | `/admin/login` | Validate login CSRF, origin and credentials; redirect with 303 on success. |
| POST | `/admin/logout` |Check the session CSRF and end the session.|
| POST | `/admin/preferences` |Language and theme. Requires Staff and Session CSRF.|
| GET | `/admin/models` | Redirect to `/admin`. |
| GET | `/admin/models/{model}` |Query: `q`, `sort`, `dir`, `page`, `filter.{feld}`.|
| GET | `/admin/models/{model}/new` | Empty creation form. |
| POST | `/admin/models/{model}/new` |Create, then 303 on the detail.|
| GET | `/admin/models/{model}/{id}` | Detail. |
| GET, POST | `/admin/models/{model}/{id}/edit` | Render the edit form or save changes. |
| GET | `/admin/models/{model}/{id}/delete` | Show confirmation without deleting. |
| POST | `/admin/models/{model}/{id}/delete` | Delete, then redirect to the list with 303. |
| GET | `/admin/directory/users` |User. Staff superusers only.|
| GET, POST | `/admin/directory/users/new` |Create the user. No superuser field.|
| GET | `/admin/directory/users/{id}` |User detail without hash.|
| GET, POST | `/admin/directory/users/{id}/edit` |Email, Active and Staff.|
| POST | `/admin/directory/users/{id}/password` |Password set and revoke sessions of the target.|
| POST | `/admin/directory/users/{id}/active` |Activate or deactivate.|
| POST | `/admin/directory/users/{id}/staff` |Give or withdraw staff.|
| GET, POST | `/admin/directory/roles` |Display and create roles.|
| GET | `/admin/directory/permissions` |Permissions, optional `?role=`.|
| POST | `/admin/directory/roles/{role}/permissions` |`action=grant` or `revoke`.|
| POST | `/admin/directory/assignments` |Assign or withdraw role.|
| GET | `/admin/assets/admin.css` | Stylesheet. |
| GET | `/admin/assets/admin.js` | Optional JavaScript. |

GET does not change data. Writing actions are POST and respond to success with 303. Query parameters do not set, change or delete anything.

## JSON

`/admin/api` remains the existing JSON API: configuration, session, models, records and directory. It reads CSRF from the header `x-csrf-token`. HTML forms send the same token as the `csrf_token` field. The two paths are shared by `authenticate_csrf`.

## Login before a session exists

Session CSRF is available only after login. A visitor who already has `ferrioxid_anonymous` can still submit this form: the cookie guard checks `ferrioxid_login_csrf` and does not demand the anonymous session token. The login form therefore uses a separate `ferrioxid_login_csrf` cookie: 64 hex characters, HttpOnly, path `/`, SameSite from auth configuration, `Secure` when the session cookie is secure, and a lifetime of at most ten minutes. `Cache-Control: no-store` keeps the form and cookie together. Tabs reuse the same valid cookie, which is deleted after successful login. JSON `POST /auth/login` does not use this HTML login cookie.

`Origin`, or `Referer` when Origin is absent, must match the allowlist. When both are missing, origin validation permits the request, but the login cookie and token are still required. `AuthConfig::apply_bound_address` derives loopback origins from `BIND_ADDRESS` or `server.host` and `server.port` in `ferrioxid.toml`. Unless `FERRIOXID_ALLOWED_ORIGINS` is explicitly set, the origins are `http://127.0.0.1:{port}` and `http://localhost:{port}` for the bound port. Public hosts and request Host headers are never added. `Origin: null` is accepted only with `Sec-Fetch-Site: same-origin`, as browsers may send it for a same-origin form with `Referrer-Policy: no-referrer`. Rejection shows “The security check failed”. Logs record the reason code and, for origin rejection, only source, category and port; they never log cookies or tokens.

`next` is only accepted if the value begins with `/admin`, has a maximum of 200 characters and contains neither `//`, `\`, `://` nor control characters. Everything else becomes `/admin`.

## Errors

| Situation | HTML |
|---|---|
| Auth is not configured | 403, plain text |
| No session | 303 to `/admin/login?next=` |
| Not staff | 403, plain text `staff access is required` |
| Missing model permission | 403, error page |
|Data set not visible|404, same text as for a missing record|
|Invalid sorting or invalid filter| 422 |
|Missing or incorrect CSRF| 403 |
| Invalid credentials, unknown email or inactive user | 401, identical response text |

The JSON API retains its previous status codes. An anonymous JSON request remains 401.
