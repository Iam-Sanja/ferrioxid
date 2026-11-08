# Admin backlog: errors, design and directory

[Overview](index.md) · Roadmap: [roadmap.md](roadmap.md) · Comparison: [admin-django-parity.md](admin-django-parity.md)

**Implementation status:** v0.22. Custom pages, actions, filters and widgets can be registered. Logout for non-staff sessions, separate status flags, metadata-based columns, design tokens, role selection, user management, server-side confirmation, readable relations, filters, list configuration, fieldsets, save destinations, selection pages, drafts, has-many inlines, opt-in parent-child forms, bulk actions, history, UTC date hierarchy and transactional directory auditing are implemented. Cancel leaves drafts until expiration. Many-to-many and nested inlines remain unavailable.

The v0.15 design work used the locally installed Impeccable 4.8.0 skill at `.cursor/skills/impeccable/` for context, criteria, auditing, layout and polish. The visual direction remains “The Night Shift” from `DESIGN.md`.

## P0 – Logout without staff access

Implemented in v0.14. For a valid non-staff session, `require_get` renders `error.html` with `staff_required`, CSRF and no sidebar. `POST /admin/logout` uses `authenticate_csrf` without requiring `is_staff`.

### Acceptance criteria

- Every admin response for a valid session, including 403 and 404, offers logout.
- Logout is a CSRF-protected POST, never a GET or tokenless link.
- `is_staff` does not determine whether a user may end their own session.
- Success revokes the session, clears its cookie and redirects with 303 to `/admin/login`.
- A non-staff user cannot access model or directory routes.
- Missing or invalid CSRF returns 403 without changing the session.
- Logout changes neither model permissions nor another user's session.

### Tests

`examples/myshop/tests/admin_html.rs` checks:

1. A user with `is_staff = false` receives an HTML 403 from `GET /admin`, including a logout form.
2. `POST /admin/logout` with valid CSRF returns 303; the old cookie then redirects to login.
3. Logout without CSRF returns 403, and the session remains valid for `GET /auth/me` or another protected request.
4. The user cannot read `GET /admin/models/...`, including after a failed logout.
5. Staff users can still log out from the header.

### Files

`html.rs` (`require_get`, `error_page`, `logout`), `templates/errors/error.html`, `templates/base.html`, `examples/myshop/tests/admin_html.rs`, `docs/admin-security.md`.

## P0 – Staff and Superuser

Implemented in v0.14. Active, Staff and Superuser are independent flags. The CLI reports them after creation; the user list gives each its own column.

### Acceptance criteria

- Neither login nor an admin request silently sets `is_staff`.
- `ferrioxid admin create-user --superuser` without `--staff` reports that staff access is required for the admin while preserving the requested flags.
- The 403 page explains the missing staff flag without revealing whether other accounts exist.
- Active, Staff and Superuser are expressed in words rather than color alone.
- [auth.md](auth.md) documents the combinations.

| Active | Staff | Superuser | Admin models | Directory |
|---|---|---|---|---|
| no | either | either | No session | No session |
| yes | no | no | 403; logout available | 403 |
| yes | no | yes | 403; logout available | 403 |
| yes | yes | no | Model permissions required | 403 |
| yes | yes | yes | All registered models | yes |

### Tests

The CLI reports that a superuser without staff cannot enter the admin and preserves `is_staff = false`. HTML tests cover the explanatory 403 and logout. A staff superuser sees the directory; ordinary staff do not.

### Files

`crates/ferrioxid-cli/src/main.rs`, `crates/ferrioxid-cli/src/text.rs`, `html.rs`, `templates/directory/users.html`, `templates/errors/error.html`, `docs/auth.md`.

## P0 – List columns

Implemented in v0.14. The current column rules are in [admin-native.md](admin-native.md); models do not require a `name` field.

### Acceptance criteria

- Columns come from metadata, not a hard-coded `name` field.
- Explicit `#[admin(list_display)]` annotations define the list in struct order. Once any non-ID field is marked, unmarked fields are not added by fallback.
- Without explicit non-ID columns, choose the first visible, non-sensitive text field; use the documented fallback when no text field exists. `id` remains available.
- Read-only fields may label records without being editable.
- Hidden and sensitive fields never appear, including through fallback.
- Permissions and query scopes still apply. Inaccessible records return 404.
- `title`, `email` or another text field can label a record.

### Tests

1. An unmarked text field appears beside the UUID.
2. Explicit columns appear without extra fallback fields.
3. A model without a text field has a valid ID/action list without a panic or empty header.
4. Hidden `password_hash` never appears.
5. The myshop product list includes its name.

### Files

`crates/ferrioxid-macros/src/lib.rs`, `crates/ferrioxid-admin/src/registry.rs`, `crates/ferrioxid-admin/src/html.rs`, `templates/models/list.html`, `crates/ferrioxid-cli/src/scaffold.rs`, `examples/myshop/tests/admin_html.rs`.

The generator marks the name field with `list_display` in new fullstack projects so explicit selection and fallback agree.

## P1 – Design

Sources: `static/css/admin.css`, `templates/`, [admin-design-system.md](admin-design-system.md).

### Acceptance criteria

| Topic | Status |
|---|---|
| Spacing tokens `--space-1` through `--space-6` | Implemented; multiples of 0.25rem |
| Equal-height controls with errors below fields | Implemented |
| Field groups and a maximum 40rem form width | Implemented, shared `field.html` partial |
| Primary, quiet and danger buttons | Implemented |
| Separated detail, edit and delete actions | Implemented |
| Search and filters in a separate toolbar group | Implemented, server-side |
| Visible focus, including the sidebar | Implemented, `:focus-visible` |
| Role selection | Implemented, `<select>`; assignment never creates an unknown role |
| Empty, error and success states | Implemented |
| Current-page navigation without a script-only menu | Implemented |
| Contrast and branding | Implemented, 4.5:1 text contrast gate; dark sidebar keeps its own color |

### Existing behavior

The layout has a sidebar and content area, stacking below 800 pixels, with horizontally scrolling tables. Light, dark and `system` themes share tokens. Controls provide visible focus and a minimum height of 2.5rem. The page includes a skip link, `lang`, labels, `role="status"` and `role="alert"`. Sources: `static/css/admin.css`, `templates/base.html`.

### Contract

These design requirements are implemented. The v0.15 phase did not add data fields, auditing, permission-flag changes or a mandatory JavaScript menu; later milestones added the separately documented features.

## P1 – User management

The v0.16 directory uses dedicated admin write routes backed by `AuthService`, not generic CRUD endpoints for auth tables.

### Implemented

Email search and sorting, 25-row pagination, Active and Staff controls, role display and assignment, permission changes, user creation, detail, profile and password forms are implemented. Self-deactivation and removal of one's own staff access are blocked. Superuser records cannot be changed through the directory. Hashes never leave the server. The server requires `confirm=1`; password changes and deactivation revoke target sessions.

### Acceptance criteria

| Feature | Requirement |
|---|---|
| Creation | Email, password twice, optional staff; no superuser selector. Passwords contain 12–128 characters and use Argon2id. |
| Editing | Email validation matches the CLI. |
| Active and Staff | Existing target and self-lockout checks remain. |
| Roles | Select existing roles rather than entering free text; show current assignments. |
| Permissions | Staff superusers only; reject codes containing `superuser`. |
| Password | Revoke target sessions; never display the hash or echo the password after redirect. |
| Sessions | Revoke target sessions for security-sensitive changes; do not affect an unrelated acting user. |
| Search and pagination | Server-side queries with a capped page size. |
| Detail | One page per user without hashes or tokens. |
| Self-lockout | Do not remove the actor's active or staff flag; superuser changes remain a CLI operation. |
| HTML | No `password_hash`, session token or visible/URL CSRF value. |

### Tests

`html_admin_works_without_javascript` covers creation, password changes, invalidation of the old session, staff revocation, confirmation, CSRF, superuser protection and non-superuser access. Duplicate email and weak-password checks use the same service validation; they are not claimed as separate cases in this HTML test.

### Contract

User, password, session and confirmation requirements are implemented. Drafts, inlines, bulk actions and history were added in v0.18 and tested by `admin_v018_drafts_inlines_bulk_and_audit`.

### Files

`html.rs`, `templates/directory/`, the `ferrioxid-auth` service methods used by the CLI, `admin_html.rs`, `admin_http.rs`. No migration is needed while the existing auth tables suffice.

## Dependencies

```text
v0.14 Abmelden, Flags, Spalten
  → v0.15 Tokens, Abstände, Rollen-Select   umgesetzt
    → v0.16 Benutzer anlegen und Passwort   umgesetzt
      → v0.17 Listen, Fieldsets, Speicherziele, Auswahlseite   umgesetzt
        → v0.18 Entwürfe, Inlines, Aktionen, Historie   umgesetzt
```

The historical diagram is retained verbatim. v0.16 uses `AuthService` rather than duplicating CLI password rules. Audit records use `_ferrum_admin_audit`; template names remain fixed.
