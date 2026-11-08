# Admin backlog: error, design, directory

[Zur Übersicht](index.md) · Roadmap: [roadmap.md](roadmap.md) · Vergleich: [admin-django-parity.md](admin-django-parity.md)

**State of implementation:** v0.22. Own pages, actions, filters and widgets can be registered. The known distances and colors of the existing admin remain unchanged. Log-out without staff, the flag notes, the default columns, the design tokens, the role selection, the user care, the server-side confirmation, the readable foreign key, the filter selection, the list configuration, fieldsets, the storage targets, the selection page, form drafts, has-many inlines, the opt-in parent-child form, collection actions, the history, the UTC date hierarchy and the atomic directory audit are implemented. Cancel does not immediately delete a draft. Many-to-many and nested inlines are missing.

Impeccable 4.8.0 is below `.cursor/skills/impeccable/`. For v0.15, context, criteria, audit, layout and polishing were executed. The report is at work. The visual world remains “The Night Shift” from `DESIGN.md`.

## P0 – Deregistration without staff rights

Transformed into v0.14. `require_get` renders for one session without Staff `error.html` with `staff_required`, CSRF and without sidebar. `POST /admin/logout` is still bound to `authenticate_csrf` and does not require `is_staff`.

### Abnahme

- Each response to a valid session contains an unsubscribe, also 403 and 404 in the admin.
- The log-off is a POST with session CSF, not a GET and not a link without a token.
- `is_staff` does not decide on the right to terminate its own session.
- After success, the session is revoked, the session cookie is emptied, response 303 to `/admin/login`.
- A non-staff will not reach a model or directory route.
- A POST without or with incorrect CSRF does not change the session and answers 403.
- Unsubscribe does not remove model rights and does not open a third-party session.

### Tests

In `examples/myshop/tests/admin_html.rs`:

1. User with `is_staff = false` loads `GET /admin`. The answer is HTML, contains a logout form and not just the plain text.
2. `POST /admin/logout` with valid CSRF delivers 303. A following `GET /admin` with the old cookie leads to the registration.
3. `POST /admin/logout` without CSRF delivers 403. `GET /auth/me` or a re-protected retrieval shows that the session is still valid.
4. The same user cannot read `GET /admin/models/...` either before or after a failed logout.
5. A Staff user can continue to log out of the existing header.

### Dateien

`html.rs` (`require_get`, `error_page`, `logout`), `templates/errors/error.html`, `templates/base.html`, `examples/myshop/tests/admin_html.rs`, `docs/admin-security.md`.

## P0 – Staff and Superuser

Implemented in v0.14. The three flags remain separate. The CLI prints them after they are created. The user list has its own columns.

### Abnahme

The three flags remain separate. No login and no admin call continues `is_staff` silently.

- `ferrioxid admin create-user --superuser` without `--staff` indicates on the console that the admin requires `is_staff` and still sets the user as the flags were set.
- The 403 page of a registered superuser without staff states that the admin access needs the staff flag. It does not reveal whether other users exist.
- The directory line separates Staff, Superuser and Active as their own words. Color alone does not carry the status.
- Documentation of the combinations in `docs/auth.md` and here.

| aktiv | Staff | Superuser | Admin-Modelle |List|
|---|---|---|---|---|
| nein | beliebig | beliebig | keine Sitzung | keine Sitzung |
| ja | nein | nein |403, can be logged out| 403 |
| ja | nein | ja |403, can be logged out| 403 |
| ja | ja | nein |Only with model law| 403 |
| ja | ja | ja | alle registrierten Modelle | ja |

### Tests

CLI test: Superuser without staff generates a clue line and `is_staff = false`. HTML test: the same user sees the explanatory 403 page and can log out. A staff superuser sees the directory. A staff without a superuser does not see it.

### Dateien

`crates/ferrioxid-cli/src/main.rs`, `crates/ferrioxid-cli/src/text.rs`, `html.rs`, `templates/directory/users.html`, `templates/errors/error.html`, `docs/auth.md`.

## P0 – Listenspalten

Transformed into v0.14. The order is in [admin-native.md](admin-native.md)]. A model without string field falls back to the first other visible field. `name` is not a requirement.

### Abnahme

- Standard columns come from the metadata, not the literal `name`.
- Candidates for the first readable column, in this order: a string field with `list_display`, otherwise the first non-sensitive string field that is not hidden, otherwise only `id`.
- `#[admin(list_display)]` remains the explicit list and determines the order of the marked fields. `id` remains available, but does not have to be the only column if an identifier exists.
- Unmarked fields do not appear in addition once at least one field except `id` is marked. Thus, a conscious list remains stable.
- If no field except `id` has a flag, the fallback applies above. Existing myshop models with marking do not change their columns.
- Write protected columns have a visible identifier but do not change the value.
- `hidden` and sensitive fields remain invisible, even in the fallback.
- Fields without model rights or outside the scope do not appear. The line of an invisible data set remains 404, as before.
- No model must have `name`. `title`, `email` or another string field can be the identifier.

### Tests

1. Model only with `id` and a string without `list_display`: the list shows the string and the UUID.
2. Model with `list_display` on a field: exactly these columns plus the existing action. No additional fallback.
3. Model without string field: only `id` and action, no panic and no empty header.
4. `password_hash` with `hidden` does not appear in the list or in the fallback.
5. myshop product list contains the name.

### Dateien

`crates/ferrioxid-macros/src/lib.rs`, `crates/ferrioxid-admin/src/registry.rs`, `crates/ferrioxid-admin/src/html.rs`, `templates/models/list.html`, `crates/ferrioxid-cli/src/scaffold.rs`, `examples/myshop/tests/admin_html.rs`.

The generator will continue `list_display` on the name field for new fullstack projects so that the explicit list and fallback show the same.

## P1 – Gestaltung

Source: `static/css/admin.css`, `templates/`, `docs/admin-design-system.md`.

### Abnahme

| Thema | Stand |
|---|---|
| Spacing-Tokens `--space-1` bis `--space-6` |implemented, multiple of 0.25rem|
|Entries of equal height, error under the field| umgesetzt |
|Groups of forms, card maximum 40rem| umgesetzt, Partial `field.html` |
| Buttons primär, ruhig, Gefahr | umgesetzt |
|Table actions by distance|implemented, details, editing, deletion|
|Search and filter as your own zone|implemented, server-side|
|Focus visible, even on the door| umgesetzt, `:focus-visible` |
| Rollenauswahl |implemented, `<select>`, unknown role is not created|
|Emptiness, mistakes, success| umgesetzt |
|Current navigation without a script menu| umgesetzt |
|Contrast and project color|implemented, gate at 4.5:1, dark door without project color|

### Is, that remains

Eine Spalte aus Seitenleiste und Inhalt, Umbruch unter 800 Pixel, Tabellen mit horizontalem Scroll, Tokens für hell, dunkel und `system`, Fokus-Outline, Mindesthöhe 2,5 rem, Sprunglink, `lang`, Labels, `role="status"` und `role="alert"`. Quelle: `static/css/admin.css`, `templates/base.html`.

### Vertrag

The acceptance above is this contract. It is fulfilled.

### Not in v0.15

No new data fields, no audit log, no change in rights flags, no JavaScript menu as the only way.

## P1 – User management

Phase v0.16. Today's directory routes remain. New writing paths are separate admin routes, not the generic model CRUD of the Auth tables.

### Ist

Search via e-mail, sorting, toggle 25 lines per page, Active and Staff, view roles, assign or withdraw role by choice, give and take rights. Own deactivation and own staff withdrawal are blocked. Superuser lines are not changeable. There is no admin way to make someone a superuser. Password hashes do not leave the server. Create, detail, email, active, staff, roles and password are HTML routes on `AuthService`. `confirm=1` is server-side mandatory. Password change and deactivation revoke the target sessions.

### Soll

| Funktion | Abnahme |
|---|---|
| Anlegen |Email, password twice, staff optional. Superuser is not selectable. Password 12 to 128 characters, Argon2id|
| Bearbeiten |Change email, clearly, same check as the CLI|
|Active and Staff|Existing blocks remain|
| Rollen |Selection, not free text. Display of current roles. Implemented in v0.15|
| Rechte |Only for staff superusers. No code containing `superuser`|
| Passwort |Setting revokes all sessions of the goal. The form never shows the hash and never the new password after the redirect|
| Sitzungen |Security-relevant change revokes the target sessions. Your own session is only preserved if the target is another user and your own role does not change|
|Search, sorting, filters, pages|Next in SQL, page size capped|
| Detail |One page per user without hash, without tokens|
| Selbstsperre |You cannot remove your own active flag, your own staff flag and your own last superuser right|
| HTML |No `password_hash`, no session token, no CSRF value in visible text or in the URL|

### Tests

Create, duplicate e-mail, password too short, change e-mail, change password and then invalidate old session, self-lock, superuser flag not set, non-superuser gets 403, hash not in HTML. `html_admin_works_without_javascript` covers creation, password, old session, staff withdrawal, confirmation, CSRF, superuser lock and non-superuser. Duplicate e-mail and too short a password run over the same service errors, but are not claimed as separate cases in this HTML run.

### Vertrag

The user, password, session and confirmation acceptance is fulfilled. Drafts, inlines, collection actions and history are implemented in v0.18 and tested in `admin_v018_drafts_inlines_bulk_and_audit`.

### Dateien

`html.rs`, `templates/directory/`, `ferrioxid-auth` service methods that already exist for the CLI, `admin_html.rs`, `admin_http.rs`. No new migration as long as the auth tables are sufficient.

## Dependencies

```text
v0.14 Abmelden, Flags, Spalten
  → v0.15 Tokens, Abstände, Rollen-Select   umgesetzt
    → v0.16 Benutzer anlegen und Passwort   umgesetzt
      → v0.17 Listen, Fieldsets, Speicherziele, Auswahlseite   umgesetzt
        → v0.18 Entwürfe, Inlines, Aktionen, Historie   umgesetzt
```

v0.16 may not duplicate the password rules of the CLI, but call `AuthService`. The audit goal is `_ferrum_admin_audit`. The template names remain fixed.
