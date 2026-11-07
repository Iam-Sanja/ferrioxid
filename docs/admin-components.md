# Admin-Bausteine

[Zur Übersicht](admin.md) · Vorlagen: [admin-templates.md](admin-templates.md) · Gestaltung: [admin-design-system.md](admin-design-system.md)

Die Bausteine sind Tera-Blöcke, keine React-Komponenten.

| Baustein | Ort |
|---|---|
| Rahmen, Seitenleiste, Kopf | `templates/base.html` |
| Feldgruppe | `templates/partials/field.html`, eingebunden als `field.html` |
| Tabelle, Suche, Blättern | `templates/models/list.html` |
| Hinweis und Warnung | `base.html`, Rollen `status` und `alert` |
| Fehlerseite | `templates/errors/error.html` |
| Benutzer, Rollen, Rechte | `templates/directory/` |

Felder entstehen in `html.rs` aus den Modellmetadaten. Unbekannte, verborgene und nur lesbare Felder werden nicht als schreibbare Eingabe ausgegeben. Die gemeinsame Prüfung `reject_locked` lehnt sie auch dann ab, wenn sie im POST auftauchen. Ein boolesches Feld ohne Wert im Formular übernimmt `default_bool`. Ein vorhandener Datensatzwert hat Vorrang.

## Geplant, nicht umgesetzt

Die offenen Zeilen dieser Tabelle sind noch nicht umgesetzt. Umgesetzte Zeilen nennen ihre Phase.

| Baustein | Phase | Verhalten ohne Script |
|---|---|---|
| Abmeldeformular auf Fehlerseiten | umgesetzt, v0.14 | POST mit CSRF, auch ohne Staff |
| Statusspalten Staff, Superuser, Aktiv | umgesetzt, v0.14 | Drei eigene Spalten |
| Listenspalte mit Bezeichner | umgesetzt, v0.14 | Metadaten, kein festes Feld `name` |
| Formulargruppe | umgesetzt, v0.15 | Label, Hinweis, Kontrolle, Fehler untereinander |
| Button primär, ruhig, Gefahr | umgesetzt, v0.15 | Dieselben Klassen überall |
| Rollenauswahl | umgesetzt, v0.15 | `<select>` vorhandener Rollen, kein Anlegen über die Zuweisung |
| Benutzerformular | umgesetzt, v0.16 | Anlegen, Bearbeiten und Passwort. Kein Hash, kein Superuser-Feld |
| Relationale Anzeige | umgesetzt, v0.16 | Name aus den Modellmetadaten, UUID als `title` |
| Filterauswahl | umgesetzt, v0.16 | Bool und Fremdschlüssel als `select`, Zahl als `number` |
| Bestätigung | umgesetzt, v0.16 | Checkbox `confirm=1`, serverseitig geprüft |
| Listenkonfiguration | umgesetzt, v0.17 | Link, Sortierung, Seitengröße aus den Metadaten |
| Formularabschnitte | umgesetzt, v0.17 | `fieldset` oder `details`, ohne Script |
| Speicherziele | umgesetzt, v0.17 | Drei Submit-Knöpfe, serverseitige Adresse |
| Auswahlseite | umgesetzt, v0.17 | GET, Suche, Seiten, Leserecht |
| Formularentwurf | umgesetzt, v0.18 | Verstecktes `_draft`, Sitzung, kein Passwort |
| Inline-Liste | umgesetzt, v0.18 | Kindzeilen und eigenes Anlegeformular |
| Sammelaktionsleiste | umgesetzt, v0.18 | Checkboxen, Aktionsauswahl, Bestätigungsseite |
| Historie | umgesetzt, v0.18 | Tabelle, Seiten, HTML-escaped |

Details und Tests: [admin-ux-backlog.md](admin-ux-backlog.md).
