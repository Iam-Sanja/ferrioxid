# Entwicklungsserver

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-cli/src/develop.rs`.

## Aufgabe

`ferrum dev` übersetzt das Projekt, startet die Binary und startet sie nach relevanten Dateiänderungen neu. `ferrum build` erzeugt einen Release-Build.

## Zuständige Dateien

- `crates/ferrum-cli/src/develop.rs`
- `crates/ferrum-cli/src/main.rs`

## Öffentliche Schnittstelle

```powershell
ferrum dev
ferrum build
```

Beide Befehle lesen `ferrum.toml` im Arbeitsverzeichnis oder einem Elternverzeichnis.

## Konfiguration

`BIND_ADDRESS` und `DATABASE_URL` gelten für den gestarteten Prozess. Ein Fullstack-Projekt mit `embedded_assets = true` verlangt vor dem Release-Build `apps/admin-ui/dist/index.html` im referenzierten Ferrum-Repository. Fehlt es, führt `ferrum build` dort `npm run build` aus. Der laufende Server startet Node.js nicht.

## Datenfluss

`dev` startet die Binary aus `target/debug` oder aus `CARGO_TARGET_DIR/debug`, wenn diese Variable gesetzt ist. Der Wächter beobachtet das Projekt rekursiv. `target`, `node_modules`, `.git`, `dist` und `embedded-admin` werden ignoriert. Nur `.rs`, `.toml` und `.json` lösen einen Neustart aus. Zwischen zwei Starts liegen mindestens 400 Millisekunden. Die CLI startet `cargo build --bin <paket>` und danach die Binary. Ein Absturz startet den Server nicht erneut, bis sich eine Datei ändert. Strg+C beendet den Kindprozess. Es läuft höchstens eine Serverinstanz.

`build` prüft bei eingebetteten Assets das Frontend und führt danach `cargo build --release` aus. Das Ergebnisverzeichnis wird ausgegeben. Liegt `CARGO_TARGET_DIR` in der Umgebung, ist das der Release-Ordner dort.

## Fehlerfälle

| Fall | Verhalten |
|---|---|
| Keine `ferrum.toml` | Abbruch mit Hinweis |
| Übersetzungsfehler | Meldung, der alte Prozess ist beendet, kein Neustart bis zur nächsten Änderung |
| Admin-Assets fehlen und `npm run build` scheitert | `ferrum build` endet mit Fehler |
| Adresse beginnt mit `0.0.0.0` | Die generierte Anwendung beendet sich |

## Sicherheitsregeln

Der Wächter startet keine Shell mit dem Dateinamen. Kindprozesse werden beim Beenden der CLI beendet. Produktionsbuilds binden das Frontend ein und brauchen danach kein Node.js.

## Beispiele

Windows und Linux:

```powershell
ferrum dev
```

```bash
ferrum dev
```

Beenden in beiden Fällen mit Strg+C.

## Tests

Die Integration in `crates/ferrum-cli/tests/generator.rs` setzt `FERRUM_DEV_EXIT_MS` und prüft, dass der Server lauscht und der Prozess danach endet. Ein Unit-Test prüft, dass Pfade unter `target` und `node_modules` nicht relevant sind. Ein weiterer Unit-Test beendet einen Kindprozess.

## Einschränkungen

Frontend-Änderungen starten Vite nicht automatisch. Dafür ist ein separater `npm run dev` in `apps/admin-ui` vorgesehen, zusammen mit `FERRUM_ADMIN_DIST`. `FERRUM_DEV_EXIT_MS` ist nur eine Testhilfe und kein Benutzerbefehl.
