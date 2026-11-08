# Admin-Routen

[For overview](admin-native.md) · Security: [admin-security.md](admin-security.md) · CSRF: [csrf.md](csrf.md)]

## HTML

| Methode | Pfad | Wirkung |
|---|---|---|
| GET | `/admin`, `/admin/` |Overview. Without session forwarding for registration.|
| GET | `/admin/login` |Form. Sets or takes over `ferrioxid_login_csrf`.|
| POST | `/admin/login` |Checks Login-CSRF, Origin and access data. Then 303.|
| POST | `/admin/logout` |Check the session CSF and end the session.|
| POST | `/admin/preferences` |Language and theme. Requires Staff and Session CSF.|
| GET | `/admin/models` | Weiterleitung auf `/admin`. |
| GET | `/admin/models/{model}` |Query: `q`, `sort`, `dir`, `page`, `filter.{feld}`.|
| GET | `/admin/models/{model}/new` | Leeres Formular. |
| POST | `/admin/models/{model}/new` |Create, then 303 on the detail.|
| GET | `/admin/models/{model}/{id}` | Detail. |
| GET, POST | `/admin/models/{model}/{id}/edit` |Form and storage.|
| GET | `/admin/models/{model}/{id}/delete` |Confirmation. Don't delete anything.|
| POST | `/admin/models/{model}/{id}/delete` |Delete, then add 303 to the list.|
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
| GET | `/admin/assets/admin.js` | Optionales Script. |

GET does not change data. Writing actions are POST and respond to success with 303. Query parameters do not set, change or delete anything.

## JSON

`/admin/api` remains the existing JSON API: configuration, session, models, records and directory. It reads CSRF from the header `x-csrf-token`. HTML forms send the same token as the `csrf_token` field. The two paths are shared by `authenticate_csrf`.

## Registration without a meeting

The session CSF exists only after login. The registration form therefore uses its own cookie `ferrioxid_login_csrf`. It is 64 Hex characters, HttpOnly, path `/`, SameSite from the Auth configuration, `Secure` only if the session cookie is `Secure`, and is valid for a maximum of ten minutes. The HTML response sets `Cache-Control: no-store` to keep the form and cookie together. Multiple tabs accept the same valid cookie. After successful registration, it will be deleted. The JSON route `POST /auth/login` remains without this cookie, as before.

`Origin` or, if no origin is sent, `Referer` must fit the Allowlist. If both are missing, the request is allowed from the point of view of the origin verification; The login cookie is independent of this obligation. The Allowlist reads `AuthConfig::apply_bound_address` from `BIND_ADDRESS` or from `server.host` and `server.port` in `ferrum.toml`. Without set `FERRUM_ALLOWED_ORIGINS`, the `http://127.0.0.1:{port}` and `http://localhost:{port}` are. An earlier fixed port 3000 no longer applies if the process is listening at another port. Public hosts and the request’s host header are not supplemented. `Origin: null` is invalid unless the browser simultaneously sends `Sec-Fetch-Site: same-origin`. This happens with `Referrer-Policy: no-referrer` for a form of the same origin. The visible error message remains "The security check failed". The log mentions the basic code and in case of an origin rejection only source, class and port, never the cookie or the token.

`next` is only accepted if the value begins with `/admin`, has a maximum of 200 characters and contains neither `//`, `\`, `://` nor control characters. Everything else becomes `/admin`.

## Fehler

| Lage | HTML |
|---|---|
|Auth is not configured| 403, Klartext |
| Keine Sitzung | 303 auf `/admin/login?next=` |
| Kein Staff | 403, Klartext `staff access is required` |
|Lack of model law| 403, Fehlerseite |
|Data set not visible|404, same text as for a missing data set|
|Invalid sorting or invalid filter| 422 |
|Missing or incorrect CSRF| 403 |
|Incorrect access data, unknown email or inactive user| 401, derselbe Text |

The JSON API retains its previous status codes. An anonymous JSON request remains 401.
