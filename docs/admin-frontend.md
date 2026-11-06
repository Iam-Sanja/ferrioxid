# Admin-Oberfläche

[Zur Übersicht](admin.md) · Bausteine: [admin-components.md](admin-components.md)

## Zweck

`apps/admin-ui` ist eine React-Anwendung mit TypeScript, Vite und Tailwind. Sie wird gebaut und vom Rust-Server unter `/admin` ausgeliefert. Im Betrieb läuft kein Vite-Prozess.

## Dateien

| Datei | Aufgabe |
|---|---|
| `apps/admin-ui/src/main.tsx` | Router mit Basis `/admin` |
| `apps/admin-ui/src/App.tsx` | Seiten und Staff-Schutz |
| `apps/admin-ui/src/api/client.ts` | `fetch` mit Cookie und CSRF im Speicher |
| `apps/admin-ui/src/auth/` | Sitzungswiederherstellung und Login |
| `apps/admin-ui/src/pages/` | Übersicht, Liste, Anlegen, Detail, Bearbeiten, Verzeichnis, 403, 404 |
| `apps/admin-ui/vite.config.ts` | `base: "/admin/"` |

## Datenfluss

Nach einem Neuladen ruft die Anwendung `GET /auth/me` und danach `GET /auth/csrf` auf. Das Token bleibt in einer Modulvariablen, nicht in `localStorage`. Schreibende Requests senden `x-csrf-token`. Cookies gehen mit `credentials: "include"`.

## Seiten

Login, Übersicht, Modellliste, Detail, Anlegen, Bearbeiten, Löschen mit Bestätigung, Verzeichnis für Superuser, 403 und 404. Hell und Dunkel über `localStorage` unter `ferrum-theme`. Das ist keine Geheimhaltung, nur die Darstellung.

## Skripte

```powershell
cd apps/admin-ui
npm install
npm test
npm run typecheck
npm run lint
npm run build
```

`lint` ist die TypeScript-Prüfung. Der Build schreibt `apps/admin-ui/dist`. Der folgende `cargo build` bettet dieses Verzeichnis ein. Siehe [admin-assets.md](admin-assets.md).

## Formularfelder

| Typ | Element |
|---|---|
| `string` | Textfeld |
| `integer` | Zahlenfeld |
| `boolean` | Checkbox |
| `uuid` | Text, Primärschlüssel nur lesbar |
| nullable | leere Auswahl oder leeres Feld |
| `foreign_key` | Auswahl über `/fields/{feld}/options` |

Beim Bearbeiten sendet das Formular nur beschreibbare Felder. Ein fehlendes Feld bleibt serverseitig unverändert. `null` schreibt SQL-NULL, wenn die Spalte das erlaubt.

## Fehler

Feldfehler mit Präfix `field:{name}:` erscheinen am Feld. Andere Fehler stehen über dem Formular. Eine blockierte Liste zeigt den Servertext. Eine fehlende Aktion blendet den Button aus und die Seite weist das Erstellen zusätzlich ab. Die Ausblendung ist keine Autorisierung.

## Tests

`apps/admin-ui/src/admin.test.tsx` mit Vitest und Testing Library: Login, Navigation, Tabelle, Validierung, fehlende Create-Aktion, Löschdialog, CSRF-Header.

## Grenzen

Keine Drag-and-Drop-Anpassung, keine Echtzeit, keine vollständige Internationalisierung. Texte der Oberfläche sind Deutsch. Servermeldungen bleiben Englisch, wie die übrige API.

## Später

Eigene Widgets und eine Komponentenbibliothek über Tailwind hinaus.
