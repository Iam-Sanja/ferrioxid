# Admin-Assets im Binary

[Zur Übersicht](index.md) · [admin-frontend.md](admin-frontend.md) · [deployment.md](deployment.md)

## Zweck

Ein produktiver Ferrum-Prozess soll HTML, CSS und JavaScript des Admins ausliefern, ohne dass Node.js läuft.

## Architektur

`crates/ferrum-admin/build.rs` kopiert `apps/admin-ui/dist` nach `crates/ferrum-admin/embedded-admin`, wenn dort `index.html` liegt. `include_dir` bettet dieses Verzeichnis ein. `embedded-admin` ist generiert und steht in `.gitignore`.

Zur Laufzeit gilt:

1. Ist `FERRUM_ADMIN_DIST` ein Verzeichnis, wird nur dieses ausgeliefert.
2. Sonst das eingebettete Verzeichnis.
3. Fehlt `index.html`, antwortet `/admin` mit 503.

## Verantwortliche Dateien

- `crates/ferrum-admin/build.rs`
- `crates/ferrum-admin/src/assets.rs`
- `apps/admin-ui/package.json`
- `apps/admin-ui/vite.config.ts`

## Öffentliche APIs

- `FERRUM_ADMIN_DIST`
- `GET /admin`, `GET /admin/` und `GET /admin/{*path}`
- `/admin/api` ist davor gemountet und wird nicht vom Dateihandler übernommen

## Datenfluss

Vite schreibt gehashte Dateien unter `dist/assets`. Der Rust-Build kopiert sie. Anfragen ohne Dateiendung fallen auf `index.html` zurück. Eine fehlende Datei mit Endung, zum Beispiel `.js` oder `.css`, ist 404. Der Content-Type folgt der Endung.

## Sicherheitsregeln

- `..` und absolute Pfade werden abgelehnt.
- Ausgeliefert wird nur das Override-Verzeichnis oder das eingebettete Frontend.
- Private Quelldateien liegen nicht in diesem Baum.
- Node.js wird zur Laufzeit nicht gestartet.

## Fehlerfälle

| Fall | Antwort |
|---|---|
| Weder Override noch eingebettetes `index.html` | 503 |
| Unbekanntes Asset mit Endung | 404 |
| Path-Traversal | 404, kein Dateiinhalt aus dem Repository |
| `/admin/api/...` ohne Sitzung | JSON-Fehler, kein HTML |

## Beispiel

```powershell
cd apps/admin-ui
npm run build
cd ../..
cargo build --workspace
```

Entwicklung mit einem frischen Frontend, ohne das Binary neu zu bauen:

```powershell
$env:FERRUM_ADMIN_DIST = "D:\04_Projekte_Arbeit\Projekte\ferrum\apps\admin-ui\dist"
cargo run -p myshop
```

## Tests

`admin_assets_reject_traversal_and_keep_the_api` in `examples/myshop/tests/admin_http.rs` setzt `FERRUM_ADMIN_DIST` auf ein temporäres Verzeichnis und prüft HTML, die SPA-Unterseite, `text/javascript`, einen unbekannten Asset-Pfad, Traversal und dass `/admin/api/models` JSON bleibt.

## Einschränkungen

Ein Checkout ohne vorheriges `npm run build` erzeugt ein Binary ohne Admin-HTML. Der Server kompiliert trotzdem und antwortet auf `/admin` mit 503. Der Embed entsteht beim Kompilieren. Ein späteres `npm run build` allein ändert ein bereits gebautes Binary nicht.
