# Admin-Gestaltung

[Zur Übersicht](admin-native.md) · Vorlagen: [admin-templates.md](admin-templates.md) · North Star: [../DESIGN.md](../DESIGN.md)

## Aufgabe

`crates/ferrum-admin/static/css/admin.css` ist das einzige Stylesheet. Es wird unter `/admin/assets/admin.css` ausgeliefert. Es gibt keine CDN-Adresse und kein Tailwind im nativen Admin.

Die Fläche heißt in `DESIGN.md` „Die Nachtschicht“: hell und dunkel sind zwei Beleuchtungen desselben Tisches. Diese Seite beschreibt die Tokens, die der Code wirklich setzt.

## Fläche

Die Seite ist ein Raster aus Tür und Inhalt. Die Tür ist `16rem` breit und zeigt Marke, Anwendungskopf, Übersicht, Modelle und für Superuser die Gruppe Verzeichnis. Der aktuelle Eintrag trägt `aria-current="page"`, Schriftstärke 650 und eine Unterstreichung. Er bekommt keine zweite Farbfläche.

Der Kopf zeigt den Seitentitel, die E-Mail, Sprache, Darstellung und Abmelden. Sprache und Darstellung sind ein POST auf `/admin/preferences`. Abmelden ist ein POST mit CSRF. Ein Sprunglink führt zu `#content`.

Unter 800 Pixel wird das Raster eine Spalte. Die Tür wird eine umbrechende Linkliste unter dem Kopf. Sie bleibt ohne Script offen.

## Tokens

Grundstufe ist `0.25rem`.

| Token | Wert | Verwendung |
|---|---|---|
| `--space-1` | 0.25rem | Label zur Kontrolle, enge Gruppen |
| `--space-2` | 0.5rem | Innenabstand, Flag |
| `--space-3` | 0.75rem | Abstand zwischen Feldern und Werkzeugen |
| `--space-4` | 1rem | Karteninnenraum, Kopfabstand |
| `--space-5` | 1.25rem | Seitenrand oben, Werkzeugleiste |
| `--space-6` | 1.75rem | Unabhängige Abschnitte |
| `--radius-control` | 0.6rem | Feld, Auswahl, Button |
| `--radius-notice` | 0.7rem | Hinweis und Fehlerfläche |
| `--radius-table` | 0.8rem | Tabelle |
| `--radius-card` | 1rem | Karte |
| `--control` | 2.5rem | Mindesthöhe von Feld, Auswahl und Button |
| `--form-max` | 40rem | Formular in der Karte |
| `--headline` | 1.6rem | Seitentitel |
| `--title` | 1.05rem | Kartenüberschrift |
| `--label` | 0.92rem | Feldbeschriftung |

Farben sind Rollen, nicht Einzelflächen. `--ink`, `--muted`, `--paper`, `--card`, `--focus`, `--danger`, `--ok` wechseln mit `data-theme`. `--line` ist die Kontrollkante (`#6d7b90` hell, `#8aa0bd` dunkel) und hält mindestens 3:1 zur Fläche. `--divider` trennt Zeilen und Karten und ist ruhiger. `--danger-ink` ist im Hellen weiß und im Dunkeln `#0f172a`, weil Weiß auf `#fb7185` unter 4,5:1 liegt.

`--brand` kommt aus der geprüften Primärfarbe, aber nur wenn weiße Schrift darauf mindestens 4,5:1 erreicht. Sonst bleibt `#0f172a`. Im Dunkeln färbt `--brand` weder Tür noch Primärbutton. Die JSON-API liefert die konfigurierte Farbe unverändert.

Der Fokus ist ein 3px-Ring, 2px außerhalb der Kante, nur bei `:focus-visible`. Auf der Tür ist der Ring in der Türschrift. Die Checkbox bleibt 1.2rem, die klickbare Beschriftung ist mindestens 2.75rem hoch. Dasselbe gilt für die Türlinks.

Karten im Hellen haben den Ruhehauch `0 1px 2px rgba(15, 23, 42, .06)`. Im Dunkeln ist der Schatten aus.

## Komponenten

| Klasse | Rolle |
|---|---|
| `button`, `.button` | Primäraktion, Fläche `--action` |
| `.ghost` | Zweite Aktion, transparente Fläche |
| `.danger` | Zerstörende Hauptaktion |
| `.ghost.danger` | Zerstörende Aktion in einer Zeile |
| `.field` | Label, Hinweis, Kontrolle, Fehler |
| `.notice` | Erfolg, `role="status"` |
| `.alert` | Fehlerfläche, `role="alert"` |
| `.flag` | Statuswort mit Rand, die Farbe trägt die Bedeutung nicht allein |
| `.uuid` | Gekürzte UUID, der volle Wert steht im `title` |
| `.empty-state` | Leere Liste mit nächstem Schritt, wenn einer existiert |

`templates/partials/field.html` ist die gemeinsame Feldgruppe und wird als `field.html` eingebunden.

## Tabellen und Formulare

Die erste lesbare Spalte, die keine UUID ist, ist fett. UUIDs sind gedämpft und gekürzt. Sortierte Spalten setzen `aria-sort`. Suche und Filter sind eine Zone, „Neuer Eintrag“ die andere. Filtern und Suchen sind ruhige Buttons. Zurücksetzen ist ein Link auf die Liste ohne Query. Seiten bleiben serverseitig bei 25 Zeilen.

Pflicht und optional stehen im Label. Ein Fremdschlüssel hat den Hinweis, einen vorhandenen Eintrag zu wählen, und bei Pflicht kein leeres „Keine“. Der Link „Weiter suchen“ führt auf die Auswahlseite. Nur-lesen-Felder sind Text, kein Eingabefeld, und werden nicht gesendet. Fehler stehen unter dem Feld. Ein benannter Abschnitt ist ein `fieldset` ohne Kartenrand. Ein optionaler Abschnitt ist `details`. Die Legende ist der Abschnittsname, kein Zusatz über der Überschrift. Die drei Speicherknöpfe stehen in einer Zeile und werden unter 800 Pixel zur vollen Breite.

## Verzeichnis

Die Rollenzuweisung ist ein `<select>` der vorhandenen Rollen und ein `<select>` der E-Mail-Adressen. Beide beginnen mit „Bitte wählen“. Fehlt jede Rolle, gibt es kein Textfeld, sondern einen Verweis auf die Rollenseite. Der POST lehnt eine unbekannte Rolle ab, statt sie anzulegen. Anlegen bleibt das Formular auf der Rollenseite.

Deaktivieren, Staff ändern, Zuweisen, Entziehen, Rechte ändern, Benutzer anlegen, Profil speichern und Passwort setzen zeigen das Häkchen „Ich bestätige diese Änderung.“ Der Server akzeptiert nur `confirm=1`, nach Sitzung, Staff-Superuser und den bestehenden Sperren. Das Häkchen ist keine Berechtigung und kein CSRF-Ersatz.

## Barriere

`lang` folgt der Katalogsprache. Jedes Feld hat ein Label. Die Tür, der Pfad und die Seitennavigation haben Namen. Tabellen haben `scope="col"`. Status sind Wörter. Kernaktionen sind Links oder Formulare und funktionieren ohne `admin.js`. `:focus-visible` zeichnet einen 3px-Ring 2px außerhalb. Das Script setzt nur den Fokus auf das erste ungültige Feld und enthält keinen Tastaturhandler. Eine Checkbox, deren Input im Label liegt, hat kein zusätzliches `for`. Der erste Speicherknopf ist ein visuell verstecktes `type="submit"` mit `tabindex="-1"`, damit Enter speichert, auch wenn die Auswahl `formaction` nutzt. Die HTML-Struktur prüft `admin_v018_drafts_inlines_bulk_and_audit`. Ein synthetischer Tab aus der Cursor-Browserautomatisierung hat den Fokus in v0.16 und v0.17 nicht bewegt. v0.18 hat dieselbe Struktur erneut geprüft und den Tastaturweg der Automatisierung nicht als bestanden gewertet.

## Grenze

Die Projektfarbe färbt die dunkle Tür und den dunklen Primärbutton nicht. Benutzerdetail, Benutzerformular, Passwort, Statusauswahl, Rollen, relationale Anzeige, Filterauswahl, Bestätigung, Inline-Liste, Sammelaktionsleiste und Historie nutzen dieselben Feldgruppen, Karten und Tabellen. Klassen: `.bulk-bar`, `.inline-block`, `.inline-head`, `.audit-detail`. Es gibt keine Bewegung, die `prefers-reduced-motion` abschalten müsste.
