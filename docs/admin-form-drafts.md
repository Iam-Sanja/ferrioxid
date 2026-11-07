# Formularentwürfe

[Zur Übersicht](admin.md) · Sicherheit: [admin-security.md](admin-security.md)

## Zweck

Die Fremdschlüssel-Auswahl ist eine eigene GET-Seite. Der Entwurf hält die übrigen, noch nicht gespeicherten Felder auf dem Server, gebunden an die Sitzung. Die URL enthält die Draft-ID und den gewählten Schlüssel, nicht den Formularinhalt.

`save=rows` schreibt denselben Entwurf und keine Modellzeile. Schlüssel `inline_*`, `extra_*` und `basis` bleiben erhalten. Passwörter, CSRF und Schlüssel mit `password` werden nicht gespeichert. Grenze bleibt 8 KiB, 30 Minuten und 20 Entwürfe je Sitzung.

## Speicher

Tabelle `_ferrum_admin_drafts` aus `crates/ferrum-auth/migrations/0002_admin_journal.json`.

| Spalte | Inhalt |
|---|---|
| `id` | Zufällige UUID |
| `session_id` | Text-UUID der Sitzung, Fremdschlüssel auf `_ferrum_sessions.id` mit `ON DELETE CASCADE` |
| `user_id` | Text-UUID des Benutzers |
| `model_key` | Registrierter Modellschlüssel |
| `record_id` | Leer beim Anlegen, sonst die Datensatz-ID |
| `purpose` | `form` oder `bulk` |
| `payload` | JSON, höchstens 8192 Bytes |
| `expires_at` | 30 Minuten nach dem Anlegen |

Höchstens 20 Entwürfe je Sitzung. Abgelaufene und danach die ältesten werden gelöscht. Jeder Aufruf legt eine neue ID an. Zwei Tabs überschreiben sich nicht.

## Was nicht gespeichert wird

Passwortfelder, Namen mit `password`, `token`, `secret`, `csrf` oder Endungen `_hash` und `_secret`, das CSRF-Feld, `next`, `save` und unbekannte Schlüssel. Ein lesender Zugriff prüft Sitzung, Benutzer, Modell, Zweck und Ablauf. Ein fremder Benutzer erhält keinen Entwurf.

## Ablauf

1. `POST /admin/models/{model}/draft` oder `POST /admin/models/{model}/{id}/draft` speichert die erlaubten Felder und antwortet 303 auf die Auswahlseite mit `?draft=`.
2. Die Auswahl verlinkt zurück mit `draft` und dem gewählten Fremdschlüssel.
3. Das Formular setzt `_use_draft`, füllt fehlende Felder aus dem Entwurf und lässt Query-Auswahlen gewinnen.
4. Ein Validierungsfehler rendert die gesendeten Werte und behält die Draft-ID. Er setzt `_use_draft` nicht.
5. Ein erfolgreiches Speichern löscht den Entwurf. Abbrechen ist GET und wartet auf den Ablauf.

Der endgültige POST prüft Felder, Relationen und Policies erneut. Der Entwurf ist keine Schreibfreigabe.

Code: `crates/ferrum-admin/src/drafts.rs`, Handler in `html.rs`. Test: `admin_v018_drafts_inlines_bulk_and_audit`.
