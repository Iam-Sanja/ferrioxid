# User and role directory

[For overview](index.md) · Security: [admin-security.md](admin-security.md)]

**Stand:** v0.16. Only a registered staff superuser reaches the endpoints. The check remains on the server. The HTML confirmation `confirm=1` does not apply to the JSON API.

## API

| Methode | Pfad | Wirkung |
|---|---|---|
| `GET` | `/admin/api/directory/users` |User without `password_hash`, with sorted role names|
| `POST` | `/admin/api/directory/users/{id}/active` |Activate or deactivate|
| `POST` | `/admin/api/directory/users/{id}/staff` | Staff-Flag setzen |
| `GET`/`POST` | `/admin/api/directory/roles` |Read or create roles|
| `GET` | `/admin/api/directory/roles/{role}/permissions` |Role codes|
| `POST`/`DELETE` | `/admin/api/directory/roles/{role}/permissions/{code}` |Give or withdraw right|
| `POST`/`DELETE` | `/admin/api/directory/assignments` |Engage or resolve roles in email|

`GET /auth/me` remains the small `PublicUser` without roles. The roles are only in the directory response. They arise from `_ferrum_roles` and `_ferrum_user_roles` without reading the password hash.

Your own deactivation and withdrawal of your own staff access answer 422. Changes to superusers and permission codes with `superuser` respond 403. There is no endpoint that makes a user a superuser.

## Surface surface

`/admin/directory/users` searches the email on the server side, sorts it and shows 25 lines per page. Status and roles are in the table. Deactivate, change staff, assign rights and assign roles are POST forms with CSRF. Breaking off is the link back, not a dialogue.

The HTML lock for your own deactivation and for superusers answers 403, even without confirmation. Superuser allocation remains the CLI. The admin has no field for this and rejects `is_superuser`.

Active, Staff and Superuser are in separate columns. The list links to ads, edits and roles if the line is not a superuser. Activate and Staff are only for other, unprotected user forms. A new role is created only via the role form.

| Methode | Pfad | Wirkung |
|---|---|---|
| `GET`/`POST` | `/admin/directory/users/new` |Email, password, repetition, active, staff, optional existing roles|
| `GET` | `/admin/directory/users/{id}` |Email, flags, roles, permission codes|
| `GET`/`POST` | `/admin/directory/users/{id}/edit` |Email, Active, Staff. Roles on the existing allocation|
| `POST` | `/admin/directory/users/{id}/password` |New password, then session revocation|

These paths are not model CRUD routes. Password hashes and plaintext passwords are not in HTML, logs or forwarding. A superuser without staff does not enter the admin. Users are deactivated, not deleted.

v0.28 separates the navigation: user list and role list are separate entries under "User and Access". “Create role” is written only on `/admin/directory/roles`. The user list assigns existing roles. There is no physical deletion. It would affect meetings, role ties and history and is reserved for its own functional status. The surface does not offer it.

Check writing HTML actions of the directory in this order: CSRF, staff superuser, affected line, superuser and self-lock, then `confirm`. Only the value `1` applies. If it is missing, the answer is 422 and the data set remains. `true`, `on` or an empty value do not apply. The confirmation does not replace any authorization.

Password change and deactivation revoke all target sessions. If a superuser changes his own password, his session also ends and the response leads to login. Staff withdrawal does not delete the session. The next request reads `is_staff` again and receives 403.

## Tests

`examples/myshop/tests/admin_html.rs` locates a user, creates a role, assigns and revokes a permission, assigns the role, creates a user, sets the password, checks the invalid old session, staff revocation, confirmation, CSRF, superuser lock and a non-superuser. `examples/myshop/tests/admin_http.rs` checks the JSON locks. The JSON API does not require the check mark.
