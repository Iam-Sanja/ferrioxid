# User and role directory

[Overview](index.md) · Security: [admin-security.md](admin-security.md)

**Milestone:** v0.16. Only an authenticated staff superuser can access these endpoints. Authorization is checked on the server. The HTML confirmation requirement `confirm=1` does not apply to the JSON API.

## API

| Method | Path | Effect |
|---|---|---|
| `GET` | `/admin/api/directory/users` |User without `password_hash`, with sorted role names|
| `POST` | `/admin/api/directory/users/{id}/active` |Activate or deactivate|
| `POST` | `/admin/api/directory/users/{id}/staff` | Set the staff flag |
| `GET`/`POST` | `/admin/api/directory/roles` |Read or create roles|
| `GET` | `/admin/api/directory/roles/{role}/permissions` |Role codes|
| `POST`/`DELETE` | `/admin/api/directory/roles/{role}/permissions/{code}` | Grant or revoke a permission |
| `POST`/`DELETE` | `/admin/api/directory/assignments` | Assign or revoke a role using the user's email |

`GET /auth/me` remains the small `PublicUser` without roles. The roles are only in the directory response. They arise from `_ferrum_roles` and `_ferrum_user_roles` without reading the password hash.

Your own deactivation and withdrawal of your own staff access answer 422. Changes to superusers and permission codes with `superuser` respond 403. There is no endpoint that makes a user a superuser.

## User interface

`/admin/directory/users` searches email addresses on the server, sorts results and shows 25 rows per page. Status and roles are displayed in the table. Deactivation, staff changes, permission changes and role assignment use POST forms with CSRF. Cancel is a link back, not a dialog.

The HTML lock for your own deactivation and for superusers answers 403, even without confirmation. Superuser allocation remains the CLI. The admin has no field for this and rejects `is_superuser`.

Active, Staff and Superuser have separate columns. Non-superuser rows link to detail, edit and role-assignment views. Active and Staff can be changed only for other, unprotected users. Roles can be created only through the role form.

| Method | Path | Effect |
|---|---|---|
| `GET`/`POST` | `/admin/directory/users/new` |Email, password, repetition, active, staff, optional existing roles|
| `GET` | `/admin/directory/users/{id}` |Email, flags, roles, permission codes|
| `GET`/`POST` | `/admin/directory/users/{id}/edit` |Email, Active, Staff. Roles on the existing allocation|
| `POST` | `/admin/directory/users/{id}/password` |New password, then session revocation|

These paths are separate from model CRUD routes. Password hashes and plaintext passwords never appear in HTML, logs or redirect URLs. A superuser still needs staff status to enter the admin. Users are deactivated rather than deleted.

v0.28 separates the user and role lists into entries under "Users and access". Roles are created only on `/admin/directory/roles`; the user list assigns existing roles. Physical user deletion is not available. Its effects on sessions, role assignments and history require a separate milestone.

Check writing HTML actions of the directory in this order: CSRF, staff superuser, affected line, superuser and self-lock, then `confirm`. Only the value `1` applies. If it is missing, the answer is 422 and the record remains. `true`, `on` or an empty value do not apply. The confirmation does not replace any authorization.

Password change and deactivation revoke all target sessions. If a superuser changes his own password, his session also ends and the response leads to login. Staff withdrawal does not delete the session. The next request reads `is_staff` again and receives 403.

## Tests

`examples/myshop/tests/admin_html.rs` locates a user, creates a role, assigns and revokes a permission, assigns the role, creates a user, sets the password, checks the invalid old session, staff revocation, confirmation, CSRF, superuser lock and a non-superuser. `examples/myshop/tests/admin_http.rs` checks the JSON locks. The JSON API does not require the check mark.
