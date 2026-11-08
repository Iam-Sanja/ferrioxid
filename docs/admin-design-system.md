# Admin design

[Overview](admin-native.md) · Templates: [admin-templates.md](admin-templates.md) North Star: [../DESIGN.md](../DESIGN.md)

## Purpose

`crates/ferrioxid-admin/static/css/admin.css` is the only stylesheet, served at `/admin/assets/admin.css`. The native admin uses neither a CDN nor Tailwind. `.hierarchy` is a wrapping link list using the existing spacing tokens. Browser keyboard automation remains unreliable; date-input checks therefore verify the HTML labels, `for` attributes and native input types. There is no JavaScript keyboard handler.

`DESIGN.md` calls the visual direction “The Night Shift”: light and dark are two lighting conditions for the same workspace. This page describes the tokens actually used by the code.

## Scope

The layout is a grid with a `16rem` sidebar and a content area. The sidebar shows branding, the dashboard and registered apps with their models. Superusers also see users, roles and audit history under System. The current item uses `aria-current="page"`, font weight 650 and a violet background. Models are grouped under their app name rather than a single “Models” link.

The header shows the page title, email address, language, theme and logout control. Language and theme changes use POST on `/admin/preferences`; logout is a CSRF-protected POST. A skip link targets `#content`.

Below 800 pixels, the grid becomes one column. The sidebar remains an expanded vertical list beneath the header and works without JavaScript.

## Tokens

The basic level is `0.25rem`.

| Token | Value | Purpose |
|---|---|---|
| `--space-1` | 0.25rem | Label-to-control spacing and tightly related items |
| `--space-2` | 0.5rem | Padding and status flag |
| `--space-3` | 0.75rem | Gaps between fields and tools |
| `--space-4` | 1rem | Card padding and heading spacing |
| `--space-5` | 1.25rem | Page gutters and toolbar spacing |
| `--space-6` | 1.75rem |Independent sections|
| `--radius-control` | 0.6rem |Field, selection, button|
| `--radius-notice` | 0.7rem |Note and error area|
| `--radius-table` | 0.8rem | Table |
| `--radius-card` | 1rem | Card |
| `--control` | 2rem | Input, select and button height |
| `--form-max` | 40rem | Maximum form width within a card |
| `--headline` | 1.6rem | Page title |
| `--title` | 1.05rem |Card heading|
| `--label` | 0.92rem | Field label |

Colors represent roles rather than individual surfaces. `--ink`, `--muted`, `--paper`, `--card`, `--focus`, `--danger` and `--ok` change with `data-theme`. `--line` is the control border (`#6d7b90` in light mode, `#8aa0bd` in dark mode), with at least 3:1 contrast against its background. `--divider` provides quieter separators between rows and cards. `--danger-ink` is white in light mode and `#0f172a` in dark mode because white on `#fb7185` falls below 4.5:1.

`--focus` and `--action` use violet: `#5b21b6` and `#6d28d9` in light mode, and `#ddd6fe` in dark mode. Text on dark-mode action buttons is `#1e1033`. `--brand` colors the light-mode sidebar only when white text reaches at least 4.5:1 contrast; otherwise it stays `#0f172a`. Dark-mode sidebars do not use the project color.

Focus uses a 3px ring offset 2px from the control, shown only for `:focus-visible`. Sidebar focus rings use the sidebar text color. Checkboxes remain 1.2rem. Sidebar links are 2rem high, increasing to 2.75rem below 800 pixels.

Light-mode cards use the subtle shadow `0 1px 2px rgba(15, 23, 42, .06)`. Dark-mode cards have no shadow.

## Components

| Class | Role |
|---|---|
| `button`, `.button` | Primary action using `--action` |
| `.ghost` | Secondary action with a transparent background |
| `.danger` | Primary destructive action |
| `.ghost.danger` | Destructive inline action |
| `.field` |Label, note, control, error|
| `.notice` | Success, `role="status"` |
| `.alert` | Error message with `role="alert"` |
| `.flag` |Status word with border, the color does not carry the meaning alone|
| `.uuid` |Abbreviated UUID, the full value is in the `title`|
| `.empty-state` |Empty list with next step if one exists|

`templates/partials/field.html` is the common field group and is integrated as `field.html`.

## Tables and forms

The first readable non-UUID column is bold. UUIDs are muted and shortened. Sorted columns set `aria-sort`. Search and filters occupy one toolbar group; “New entry” occupies another. Search and filter buttons are visually quiet. Reset links to the list without a query. Pagination is server-side with 25 rows per page.

Labels state whether fields are required or optional. Foreign-key fields explain that an existing record must be selected; required fields have no empty “None” option. “Search further” links to the selection page. Read-only fields are text and are not submitted. Errors appear beneath the field. Named sections use `fieldset` without a card border; optional sections use `details`. The legend is the section name. The three save buttons share a row and become full-width below 800 pixels.

## List

Role assignment uses a `<select>` of existing roles and a `<select>` of email addresses. Both initially show “Please choose”. If no roles exist, the form links to the role page rather than offering free text. POST rejects unknown roles instead of creating them. The creation form remains on the role page.

Deactivation, staff changes, role and permission changes, user creation, profile saves and password changes require the “I confirm this change” checkbox. The server accepts only `confirm=1`, after session, staff-superuser and target checks. Confirmation replaces neither authorization nor CSRF validation.

## Safety boundary

`lang` follows the catalog language. Every field has a label; the header, breadcrumb and sidebar navigation have accessible names. Table headers use `scope="col"`, and statuses are expressed in words. Core actions use links or forms and work without `admin.js`. `:focus-visible` draws a 3px ring with a 2px offset. The script only focuses the first invalid field and adds no keyboard handler. A checkbox nested inside its label needs no additional `for`. A visually hidden `type="submit"` with `tabindex="-1"` makes Enter save even when visible buttons use `formaction`. `admin_v018_drafts_inlines_bulk_and_audit` checks the HTML structure. Synthetic Tab input in browser automation did not move focus in v0.16 or v0.17; v0.18 rechecked the structure without claiming that automation passed.

## Limitations

The project color does not recolor the dark sidebar or dark primary button. User details, user forms, password forms, status and role controls, relations, filters, confirmations, inlines, bulk actions and history share field groups, cards and tables. Classes include `.bulk-bar`, `.inline-block`, `.inline-head` and `.audit-detail`. The interface has no animations requiring a `prefers-reduced-motion` override.
