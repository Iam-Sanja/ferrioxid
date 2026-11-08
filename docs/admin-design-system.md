# Admin-Gestaltung

[For overview](admin-native.md) · Templates: [admin-templates.md](admin-templates.md)] North Star: [../DESIGN.md](../DESIGN.md)]

## Aufgabe

`crates/ferrioxid-admin/static/css/admin.css` is the only style sheet. It is shipped under `/admin/assets/admin.css`. There is no CDN address and no tailwind in the native admin. `.hierarchy` is a breakthrough link list at the existing intervals. Browser keyboard automation remains unreliable. The proof of date input is the HTML: Label, `for` and the native input type. There is no JavaScript keyboard handler.

In `DESIGN.md`, the surface is called “The Night Shift”: light and dark are two illuminations of the same table. This page describes the tokens that the code really sets.

## Area

The page is a grid of door and content. The door is `16rem` wide. It shows brand, overview, the registered apps with their models, for superuser users and roles, and under system history. The current entry is `aria-current="page"`, font size 650 and a violet area. Models are under the app name. There is no single link “Models” to the overview.

The header shows the page title, the email, language, presentation and log-out. Language and presentation are a POST on `/admin/preferences`. Logging out is a POST with CSRF. A jump link leads to `#content`.

Under 800 pixels, the grid becomes a column. The door remains a vertical list under the head. It remains open without a script.

## Tokens

The basic level is `0.25rem`.

| Token | Wert | Verwendung |
|---|---|---|
| `--space-1` | 0.25rem |Label for control, close groups|
| `--space-2` | 0.5rem | Innenabstand, Flag |
| `--space-3` | 0.75rem |Distance between fields and tools|
| `--space-4` | 1rem |Card interior, head pitch|
| `--space-5` | 1.25rem |Side edge at the top, tool strip|
| `--space-6` | 1.75rem |Independent sections|
| `--radius-control` | 0.6rem |Field, selection, button|
| `--radius-notice` | 0.7rem |Note and error area|
| `--radius-table` | 0.8rem | Tabelle |
| `--radius-card` | 1rem | Karte |
| `--control` | 2rem |Level of field, selection and button|
| `--form-max` | 40rem |Form in the card|
| `--headline` | 1.6rem | Seitentitel |
| `--title` | 1.05rem |Card heading|
| `--label` | 0.92rem | Feldbeschriftung |

Farben sind Rollen, nicht Einzelflächen. `--ink`, `--muted`, `--paper`, `--card`, `--focus`, `--danger`, `--ok` wechseln mit `data-theme`. `--line` ist die Kontrollkante (`#6d7b90` hell, `#8aa0bd` dunkel) und hält mindestens 3:1 zur Fläche. `--divider` trennt Zeilen und Karten und ist ruhiger. `--danger-ink` ist im Hellen weiß und im Dunkeln `#0f172a`, weil Weiß auf `#fb7185` unter 4,5:1 liegt.

`--focus` and `--action` are violet: `#5b21b6` and `#6d28d9` in the light, `#ddd6fe` in the dark. The font on the dark action is `#1e1033`. `--brand` colors the bright door when white writing on it reaches at least 4.5:1. Otherwise, `#0f172a` remains. In the dark, `--brand` does not color the door.

The focus is a 3px ring, 2px outside the edge, only at `:focus-visible`. On the door is the ring in the door letter. The checkbox remains 1.2rem. Door links are 2rem high and under 800 pixels 2.75rem.

Cards in the bright have the resting touch `0 1px 2px rgba(15, 23, 42, .06)`. In the dark, the shadow is out.

## Komponenten

| Klasse | Rolle |
|---|---|
| `button`, `.button` |Primary action area `--action`|
| `.ghost` |Second action, transparent area|
| `.danger` | Zerstörende Hauptaktion |
| `.ghost.danger` |Destructive action in one line|
| `.field` |Label, note, control, error|
| `.notice` | Erfolg, `role="status"` |
| `.alert` |Fault area, `role="alert"`|
| `.flag` |Status word with border, the color does not carry the meaning alone|
| `.uuid` |Abbreviated UUID, the full value is in the `title`|
| `.empty-state` |Empty list with next step if one exists|

`templates/partials/field.html` is the common field group and is integrated as `field.html`.

## Tables and forms

The first legible column, which is not a UUID, is bold. UUIDs are muted and shortened. Sorted columns set `aria-sort`. Search and filters are one zone, “New entry” the other. Filtering and searching are quiet buttons. Reset is a link to the list without query. Pages remain server-side at 25 lines.

Mandatory and optional are in the label. A foreign key has the indication to select an existing entry and, if required, no empty "None". The “Search Next” link leads to the selection page. Read-only fields are text, not an input field, and are not sent. Mistakes are under the field. A named section is a `fieldset` without card edge. An optional section is `details`. The legend is the section name, no addition above the heading. The three memory buttons are in a line and become under 800 pixels to the full width.

## List

The role assignment is a `<select>` of the existing roles and a `<select>` of the email addresses. Both start with “Please choose”. If every role is missing, there is no text field, but a reference to the role page. The POST rejects an unknown role instead of creating it. Create the form remains on the roll page.

Deactivate, change staff, assign, revoke, change rights, create user, save profile and set password show the check mark “I confirm this change.” The server only accepts `confirm=1`, by session, staff superuser and the existing locks. The check mark is not an authorization and not a CSRF replacement.

## Barriere

`lang` follows the catalogue language. Each field has a label. The door, the path and the side navigation have names. Tables have `scope="col"`. Status are words. Core actions are links or forms and work without `admin.js`. `:focus-visible` draws a 3px ring 2px outside. The script only focuses on the first invalid field and does not contain a keyboard handler. A checkbox whose input is in the label does not have an additional `for`. The first memory button is a visually hidden `type="submit"` with `tabindex="-1"` for Enter to store, even if the selection uses `formaction`. The HTML structure checks `admin_v018_drafts_inlines_bulk_and_audit`. A synthetic tab from cursor browser automation did not move the focus in v0.16 and v0.17. v0.18 rechecked the same structure and did not pass the automation keyboard path.

## Grenze

The project color does not color the dark door and the dark primary button. User detail, user form, password, status selection, roles, relational display, filter selection, confirmation, inline list, collective action bar and history use the same field groups, maps and tables. Classes: `.bulk-bar`, `.inline-block`, `.inline-head`, `.audit-detail`. There is no movement that would have to turn `prefers-reduced-motion` off.
