# Admin-Backlog: Fehler, Gestaltung, Verzeichnis

[Zur Übersicht](index.md) · Roadmap: [roadmap.md](roadmap.md) · Vergleich: [admin-django-parity.md](admin-django-parity.md)

**Stand der Umsetzung:** v0.22. Eigene Seiten, Aktionen, Filter und Widgets sind registrierbar. Die bekannten Abstände und Farben des bestehenden Admins bleiben unverändert. Abmelden ohne Staff, die Flag-Hinweise, die Standardspalten, die Design-Tokens, die Rollenauswahl, die Benutzerpflege, die serverseitige Bestätigung, lesbare Fremdschlüssel, die Filterauswahl, die Listenkonfiguration, Fieldsets, die Speicherziele, die Auswahlseite, Formularentwürfe, Has-Many-Inlines, das opt-in Eltern-Kind-Formular, Sammelaktionen, die Historie, die UTC-Datumshierarchie und das atomare Verzeichnis-Audit sind umgesetzt. Abbrechen löscht einen Entwurf nicht sofort. Many-to-Many und verschachtelte Inlines fehlen.

Impeccable 4.8.0 liegt unter `.cursor/skills/impeccable/`. Für v0.15 wurden Kontext, Critique, Audit, Layout und Feinschliff ausgeführt. Der Bericht steht im Arbeitsgang. Die visuelle Welt bleibt „Die Nachtschicht“ aus `DESIGN.md`.

## P0 – Abmelden ohne Staff-Recht

Umgesetzt in v0.14. `require_get` rendert für eine Sitzung ohne Staff `error.html` mit `staff_required`, CSRF und ohne Seitenleiste. `POST /admin/logout` ist unverändert an `authenticate_csrf` gebunden und verlangt kein `is_staff`.

### Abnahme

- Jede Antwort an eine gültige Sitzung enthält eine Abmeldung, auch 403 und 404 im Admin.
- Die Abmeldung ist ein POST mit Sitzungs-CSRF, nicht ein GET und nicht ein Link ohne Token.
- `is_staff` entscheidet nicht über das Recht, die eigene Sitzung zu beenden.
- Nach Erfolg ist die Sitzung widerrufen, das Session-Cookie geleert, Antwort 303 auf `/admin/login`.
- Ein Nicht-Staff erreicht danach keine Modell- oder Verzeichnisroute.
- Ein POST ohne oder mit falschem CSRF ändert die Sitzung nicht und antwortet 403.
- Die Abmeldung hebt keine Modellrechte auf und öffnet keine fremde Sitzung.

### Tests

In `examples/myshop/tests/admin_html.rs`:

1. Benutzer mit `is_staff = false` lädt `GET /admin`. Die Antwort ist HTML, enthält ein Logout-Formular und nicht nur den Klartext.
2. `POST /admin/logout` mit gültigem CSRF liefert 303. Ein folgendes `GET /admin` mit dem alten Cookie führt zur Anmeldung.
3. `POST /admin/logout` ohne CSRF liefert 403. `GET /auth/me` oder ein erneuter geschützter Abruf zeigt, dass die Sitzung noch gilt.
4. Derselbe Benutzer kann `GET /admin/models/...` nicht lesen, weder vor noch nach einem fehlgeschlagenen Logout.
5. Ein Staff-Benutzer kann sich weiter über die bestehende Kopfzeile abmelden.

### Dateien

`html.rs` (`require_get`, `error_page`, `logout`), `templates/errors/error.html`, `templates/base.html`, `examples/myshop/tests/admin_html.rs`, `docs/admin-security.md`.

## P0 – Staff und Superuser

Umgesetzt in v0.14. Die drei Flags bleiben getrennt. Die CLI druckt sie nach dem Anlegen. Die Benutzerliste hat eigene Spalten.

### Abnahme

Die drei Flags bleiben getrennt. Kein Login und kein Admin-Aufruf setzt `is_staff` still nach.

- `ferrum admin create-user --superuser` ohne `--staff` weist auf der Konsole darauf hin, dass der Admin `is_staff` verlangt, und legt den Benutzer trotzdem so an, wie die Flags gesetzt wurden.
- Die 403-Seite eines angemeldeten Superusers ohne Staff nennt, dass der Admin-Zugang das Staff-Flag braucht. Sie verrät nicht, ob andere Benutzer existieren.
- Die Verzeichniszeile trennt Staff, Superuser und Aktiv als eigene Wörter. Farbe allein trägt den Status nicht.
- Dokumentation der Kombinationen in `docs/auth.md` und hier.

| aktiv | Staff | Superuser | Admin-Modelle | Verzeichnis |
|---|---|---|---|---|
| nein | beliebig | beliebig | keine Sitzung | keine Sitzung |
| ja | nein | nein | 403, abmelden möglich | 403 |
| ja | nein | ja | 403, abmelden möglich | 403 |
| ja | ja | nein | nur mit Modellrecht | 403 |
| ja | ja | ja | alle registrierten Modelle | ja |

### Tests

CLI-Test: Superuser ohne Staff erzeugt eine Hinweiszeile und `is_staff = false`. HTML-Test: derselbe Benutzer sieht die erklärende 403-Seite und kann sich abmelden. Ein Staff-Superuser sieht das Verzeichnis. Ein Staff ohne Superuser sieht es nicht.

### Dateien

`crates/ferrum-cli/src/main.rs`, `crates/ferrum-cli/src/text.rs`, `html.rs`, `templates/directory/users.html`, `templates/errors/error.html`, `docs/auth.md`.

## P0 – Listenspalten

Umgesetzt in v0.14. Die Reihenfolge steht in [admin-native.md](admin-native.md). Ein Modell ohne String-Feld fällt auf das erste andere sichtbare Feld zurück. `name` ist keine Voraussetzung.

### Abnahme

- Standardspalten kommen aus den Metadaten, nicht aus dem Literal `name`.
- Kandidaten für die erste lesbare Spalte, in dieser Reihenfolge: ein String-Feld mit `list_display`, sonst das erste nicht sensible String-Feld, das nicht verborgen ist, sonst nur `id`.
- `#[admin(list_display)]` bleibt die ausdrückliche Liste und bestimmt die Reihenfolge der markierten Felder. `id` bleibt verfügbar, muss aber nicht die einzige Spalte sein, wenn ein Bezeichner existiert.
- Nicht markierte Felder erscheinen nicht zusätzlich, sobald mindestens ein Feld außer `id` markiert ist. So bleibt eine bewusste Liste stabil.
- Hat kein Feld außer `id` eine Markierung, gilt der Fallback oben. Bestehende myshop-Modelle mit Markierung ändern ihre Spalten nicht.
- Schreibgeschützte Spalten tragen ein sichtbares Kennzeichen, ändern den Wert aber nicht.
- `hidden` und sensible Felder bleiben unsichtbar, auch im Fallback.
- Felder ohne Modellrecht oder außerhalb des Scopes erscheinen nicht. Die Zeile eines unsichtbaren Datensatzes bleibt 404, wie bisher.
- Kein Modell muss `name` besitzen. `title`, `email` oder ein anderes String-Feld können der Bezeichner sein.

### Tests

1. Modell nur mit `id` und einem String ohne `list_display`: die Liste zeigt den String und die UUID.
2. Modell mit `list_display` auf einem Feld: genau diese Spalten plus die bestehende Aktion. Kein zusätzlicher Fallback.
3. Modell ohne String-Feld: nur `id` und Aktion, kein Panic und kein leerer Header.
4. `password_hash` mit `hidden` erscheint weder in der Liste noch im Fallback.
5. myshop-Produktliste enthält den Namen.

### Dateien

`crates/ferrum-macros/src/lib.rs`, `crates/ferrum-admin/src/registry.rs`, `crates/ferrum-admin/src/html.rs`, `templates/models/list.html`, `crates/ferrum-cli/src/scaffold.rs`, `examples/myshop/tests/admin_html.rs`.

Der Generator setzt bei neuen Fullstack-Projekten weiter `list_display` auf das Namensfeld, damit die ausdrückliche Liste und der Fallback dasselbe zeigen.

## P1 – Gestaltung

Umgesetzt in v0.15. Quelle: `static/css/admin.css`, `templates/`, `docs/admin-design-system.md`.

### Abnahme

| Thema | Stand |
|---|---|
| Spacing-Tokens `--space-1` bis `--space-6` | umgesetzt, Vielfache von 0.25rem |
| Eingaben gleicher Höhe, Fehler unter dem Feld | umgesetzt |
| Formulargruppen, Karte höchstens 40rem | umgesetzt, Partial `field.html` |
| Buttons primär, ruhig, Gefahr | umgesetzt |
| Tabellenaktionen mit Abstand | umgesetzt, Details, Bearbeiten, Löschen |
| Suche und Filter als eigene Zone | umgesetzt, serverseitig |
| Fokus sichtbar, auch auf der Tür | umgesetzt, `:focus-visible` |
| Rollenauswahl | umgesetzt, `<select>`, unbekannte Rolle wird nicht angelegt |
| Leere, Fehler, Erfolg | umgesetzt |
| Aktuelle Navigation ohne Script-Menü | umgesetzt |
| Kontrast und Projektfarbe | umgesetzt, Tor bei 4,5:1, dunkle Tür ohne Projektfarbe |

### Ist, das bleibt

Eine Spalte aus Seitenleiste und Inhalt, Umbruch unter 800 Pixel, Tabellen mit horizontalem Scroll, Tokens für hell, dunkel und `system`, Fokus-Outline, Mindesthöhe 2,5 rem, Sprunglink, `lang`, Labels, `role="status"` und `role="alert"`. Quelle: `static/css/admin.css`, `templates/base.html`.

### Vertrag

Die Abnahme oben ist dieser Vertrag. Er ist erfüllt.

### Nicht in v0.15

Keine neuen Datenfelder, kein Audit-Log, keine Änderung der Rechteflags, kein JavaScript-Menü als einziger Weg.

## P1 – Benutzerverwaltung

Phase v0.16. Die heutigen Verzeichnisrouten bleiben. Neue Schreibwege sind eigene Admin-Routen, nicht das generische Modell-CRUD der Auth-Tabellen.

### Ist

Suche über E-Mail, Sortierung, 25 Zeilen je Seite, Aktiv und Staff umschalten, Rollen anzeigen, Rolle per Auswahl zuweisen oder entziehen, Rechte geben und nehmen. Eigenes Deaktivieren und eigener Staff-Entzug sind blockiert. Superuser-Zeilen sind nicht veränderbar. Es gibt keinen Admin-Weg, jemanden zum Superuser zu machen. Passwort-Hashes verlassen den Server nicht. Anlegen, Detail, E-Mail, Aktiv, Staff, Rollen und Passwort sind HTML-Routen auf `AuthService`. `confirm=1` ist serverseitig Pflicht. Passwortänderung und Deaktivierung widerrufen die Zielsitzungen.

### Soll

| Funktion | Abnahme |
|---|---|
| Anlegen | E-Mail, Passwort zweimal, Staff optional. Superuser ist nicht wählbar. Passwort 12 bis 128 Zeichen, Argon2id |
| Bearbeiten | E-Mail ändern, eindeutig, dieselbe Prüfung wie die CLI |
| Aktiv und Staff | Bestehende Sperren bleiben |
| Rollen | Auswahl, nicht Freitext. Anzeige der aktuellen Rollen. In v0.15 umgesetzt |
| Rechte | Weiter nur für Staff-Superuser. Kein Code, der `superuser` enthält |
| Passwort | Setzen widerruft alle Sitzungen des Ziels. Das Formular zeigt nie den Hash und nie das neue Passwort nach dem Redirect |
| Sitzungen | Sicherheitsrelevante Änderung widerruft die Zielsitzungen. Die eigene Sitzung bleibt nur erhalten, wenn das Ziel ein anderer Benutzer ist und die eigene Rolle sich nicht ändert |
| Suche, Sortierung, Filter, Seiten | Weiter in SQL, Seitengröße gedeckelt |
| Detail | Eine Seite je Benutzer ohne Hash, ohne Token |
| Selbstsperre | Man kann das eigene Aktiv-Flag, das eigene Staff-Flag und das eigene letzte Superuser-Recht nicht entfernen |
| HTML | Kein `password_hash`, kein Session-Token, kein CSRF-Wert in sichtbarem Text oder in der URL |

### Tests

Anlegen, doppelte E-Mail, zu kurzes Passwort, E-Mail ändern, Passwort ändern und danach alte Sitzung ungültig, Selbstsperre, Superuser-Flag nicht setzbar, Nicht-Superuser erhält 403, Hash nicht im HTML. `html_admin_works_without_javascript` deckt Anlegen, Passwort, alte Sitzung, Staff-Entzug, Bestätigung, CSRF, Superuser-Sperre und Nicht-Superuser ab. Doppelte E-Mail und zu kurzes Passwort laufen über dieselben Service-Fehler, sind in diesem HTML-Lauf aber nicht als eigene Fälle behauptet.

### Vertrag

Die Benutzer-, Passwort-, Sitzungs- und Bestätigungsabnahme ist erfüllt. Entwürfe, Inlines, Sammelaktionen und Historie sind in v0.18 umgesetzt und in `admin_v018_drafts_inlines_bulk_and_audit` geprüft.

### Dateien

`html.rs`, `templates/directory/`, `ferrum-auth` Service-Methoden, die es schon für die CLI gibt, `admin_html.rs`, `admin_http.rs`. Keine neue Migration, solange die Auth-Tabellen reichen.

## Abhängigkeiten

```text
v0.14 Abmelden, Flags, Spalten
  → v0.15 Tokens, Abstände, Rollen-Select   umgesetzt
    → v0.16 Benutzer anlegen und Passwort   umgesetzt
      → v0.17 Listen, Fieldsets, Speicherziele, Auswahlseite   umgesetzt
        → v0.18 Entwürfe, Inlines, Aktionen, Historie   umgesetzt
```

v0.16 darf die Passwortregeln der CLI nicht duplizieren, sondern `AuthService` rufen. Das Audit-Ziel ist `_ferrum_admin_audit`. Die Vorlagennamen bleiben fest.
