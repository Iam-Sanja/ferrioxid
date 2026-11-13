# Unified admin record editor

Model records use `/admin/models/{model}/{id}` for both display and editing.
The server shows an editor only when the caller can read and update that object;
read-only users see a detail page. Update permission alone does not grant read access.
The existing `/edit` GET and POST routes remain supported for bookmarks and callers.

Save returns to the list. Save and continue stays on the canonical record URL;
Save and add another is offered only with create permission. Legacy `save=detail`
and `save=edit` submissions continue to open the saved record. Cancel returns to
the list without submitting. Related records and their pagination remain available
alongside editable fields; composed inline forms retain their existing draft handling.

Read-only and sensitive fields remain protected on the server, including forged
POST requests. Delete is shown only when the caller can delete that particular
object and still requires the existing confirmation and CSRF checks. Unavailable
foreign-key values keep a generic selected placeholder instead of silently becoming
NULL; submissions still undergo the existing relation authorization checks.

## Users and roles

`/admin/directory/users/{id}` combines profile fields, assigned roles, password
changes and effective permissions. Existing role assignment and revocation forms
remain separate POST actions with their existing confirmation and CSRF checks.
Protected superusers retain a read-only detail page. Users can be deactivated;
physical user deletion is not supported. Existing self-protection remains enforced.
The `/edit` alias remains available.

`/admin/directory/roles/{id}` opens one role by UUID and reuses the existing
permission grant/revoke forms. The role name and UUID are read-only: role renaming
and deletion are not supported by the directory API. Dedicated role creation at
`/admin/directory/roles/new` remains available. The old permissions overview and
its `?role=` filter continue to work. Directory access still requires an active
staff superuser.

## Navigation and accessibility

Primary table cells contain native links, including without JavaScript. Clicking
other non-interactive cells opens the same record. Checkboxes, labels, action menus,
links, editable content, modifier clicks and text selection retain their native
behavior. Keyboard navigation uses the real links and controls rather than adding
an extra focus stop to every row. Existing German and English catalogs supply the
labels; light and dark themes remain supported.

## Regression checks

`cargo test -p myshop --test admin_html` exercises actual HTTP routes against
disposable in-memory databases, including permissions, forged fields, CSRF,
protected users, role changes, deletion and legacy URLs.

The optional browser check uses HTML captured by that HTTP test. It verifies DOM
navigation and controls with the current admin assets; it does not replace backend
authorization tests or perform browser-driven database writes.

```powershell
$fixturePath = Join-Path $env:TEMP ([guid]::NewGuid().ToString())
$env:FEXU_BROWSER_FIXTURES = $fixturePath
cargo test -p myshop --test admin_html canonical_editors_preserve_permissions_readonly_fields_roles_and_navigation
Remove-Item Env:FEXU_BROWSER_FIXTURES
python -m pip install playwright
python -m playwright install chromium
python scripts/test_admin_navigation.py --fixtures $fixturePath --artifacts "$env:TEMP/admin-editor-screenshots"
```

Use a fresh fixture directory; the test refuses to overwrite an existing one.
An installed Chromium browser can be selected with `--browser-executable`.
