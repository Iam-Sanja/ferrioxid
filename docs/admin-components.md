# Admin-Bausteine

[Zur Übersicht](admin.md) · Vorlagen: [admin-templates.md](admin-templates.md) · Gestaltung: [admin-design-system.md](admin-design-system.md)

Die Bausteine sind Tera-Blöcke, keine React-Komponenten.

| Baustein | Ort |
|---|---|
| Rahmen, Seitenleiste, Kopf | `templates/base.html` |
| Feld | `templates/models/form.html` |
| Tabelle, Suche, Blättern | `templates/models/list.html` |
| Hinweis und Warnung | `base.html`, Rollen `status` und `alert` |
| Fehlerseite | `templates/errors/error.html` |
| Benutzer, Rollen, Rechte | `templates/directory/` |

Felder entstehen in `html.rs` aus den Modellmetadaten. Unbekannte, verborgene und nur lesbare Felder werden nicht als schreibbare Eingabe ausgegeben. Die gemeinsame Prüfung `reject_locked` lehnt sie auch dann ab, wenn sie im POST auftauchen.
