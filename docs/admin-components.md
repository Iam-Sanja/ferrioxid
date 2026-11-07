# Admin-Komponenten

[Zur Übersicht](admin-frontend.md)

## Zweck

Die Oberfläche besteht aus wenigen eigenen Komponenten. Es gibt keine zusätzliche UI-Bibliothek. `ConfirmDialog` fängt Escape, setzt den Fokus auf Abbrechen und bestätigt Löschen sowie kritische Verzeichnisaktionen. `DataTable` sucht, sortiert, blättert und formatiert Integer, `_cents` und `_at`. `DynamicForm` zeigt Pflichtfelder und übersetzte Feldfehler. `Sidebar` markiert den aktiven Link.

## Dateien

| Komponente | Datei | Aufgabe |
|---|---|---|
| `Layout` | `src/components/Layout.tsx` | Sidebar, Inhaltsfläche, Heller/Dunkler Modus |
| `Sidebar` | `src/components/Sidebar.tsx` | Modellnavigation, Rollenlink nur für Superuser, Abmelden |
| `DataTable` | `src/components/DataTable.tsx` | Spalten, Sortierung, Trefferbereich, leerer Zustand |
| Schaltflächen | `src/styles/app.css` | `.btn-primary`, `.btn-secondary`, `.btn-danger` |
| `DynamicForm` | `src/components/DynamicForm.tsx` | Pflichtfeldprüfung und Speichern |
| `FieldRenderer` | `src/components/FieldRenderer.tsx` | Text, Zahl, Checkbox, Fremdschlüssel |
| `AuthProvider` | `src/auth/AuthProvider.tsx` | Benutzer nach `/auth/me` und `/auth/csrf` |
| `LoginPage` | `src/auth/LoginPage.tsx` | Anmeldung, Staff-Hinweis |

## Datenfluss

`Layout` lädt `GET /admin/api/models`. `ModelList` lädt Schema und `records`. `DynamicForm` übergibt nur die Werte, die der Aufrufer sendet. `ModelEdit` filtert vorher auf Felder ohne `readonly`. `FieldRenderer` lädt Optionen erst, wenn `foreign_key` gesetzt ist, und zeigt die Servermeldung, wenn die Auswahl verweigert wird.

## Konfiguration

Keine Laufzeitkonfiguration im Browser. Die Basisadresse ist `/admin`, die API bleibt auf demselben Origin.

## Sicherheit

Komponenten blenden Aktionen aus, wenn `schema.actions` sie nicht enthält. Der Server prüft dieselbe Aktion noch einmal. Der Löschdialog ist ein `dialog` mit Abbrechen und Bestätigen.

## Fehler

Ladezustand ist ein kurzer Text. Listenfehler und Beziehungsfehler nutzen `role="alert"`. Ein 403 auf der Modellanlage bleibt auf der Seite sichtbar.

## Beispiel

Ein Suchfeld setzt `q` und springt auf Seite 1. Ein Sortierknopf existiert für `sortable` und für `id`.

## Tests

Die Vitest-Datei rendert Tabelle, Formular, Login, gesperrtes Anlegen und den Löschdialog gegen einen nachgebildeten `fetch`.

## Grenzen

Tabellen haben keine Spaltenauswahl. Formulare haben keine Feldgruppen. Dialoge nutzen das native `dialog`-Element.

## Später

Tastaturfokus-Falle für den Dialog und wiederverwendbare Fehlertabelle für mehrere Felder.
