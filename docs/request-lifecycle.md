# Request-Lebenszyklus

**Stand:** v0.12, IST. Die Cargo-Version bleibt `0.1.0`.

## Verantwortlichkeit

Dieses Dokument beschreibt die Reihenfolge einer HTTP-Anfrage, nachdem Formulare, Staticfiles, Uploads und Middleware ergänzt wurden.

## Crates und Dateien

- Routeraufbau: `crates/ferrum-api/src/lib.rs`, `App::merge`, `App::into_router`
- Hülle: `crates/ferrum-api/src/web.rs`
- Middleware: `crates/ferrum-middleware/src/lib.rs`
- Seiten: `crates/ferrum-api/src/pages.rs`
- Auth: `crates/ferrum-auth/src/service.rs`

## Öffentliche API

`App::merge` hängt zustandsbehaftete Routen an, bevor `into_router` den Zustand bindet. `ferrum::web::prepare` montiert danach Staticfiles, wenn `[static] enabled` ist, und die Standard-Middleware.

## Datenfluss

```text
Body-Limit
  → Request-ID
  → Access-Log
  → Sicherheitsheader
  → Axum-Route
       → REST
       → Admin und eingebettete Assets
       → Tera-Seiten und Formulare
       → Staticfiles
       → ausdrücklich registrierte Uploads
  → Modell, Auth oder Storage
  → SQLite, PostgreSQL oder lokaler Speicher
```

Auth und CSRF laufen im Handler. Eine Middleware kann sie nicht überspringen, weil sie die Handler-Prüfung nicht ersetzt.

## Konfiguration

Staticfiles und Storage stehen in `ferrum.toml`. Das Body-Limit folgt der Upload-Größe, wenn `prepare` verwendet wird. myshop montiert Staticfiles und Middleware ausdrücklich, weil die Workspace-Datei Staticfiles auslässt.

## Sicherheit

Zuerst wird die Größe begrenzt, danach eine eigene Request-ID erzeugt. Logs sehen den Pfad ohne Query. Formulare prüfen Felder serverseitig. Uploads prüfen den Besitzer. Staticfiles bleiben im konfigurierten Root.

## Fehlerfälle

413 vom Body-Limit, 401 und 403 von Auth und CSRF, 404 von fremden Dateien und fehlenden Staticfiles, 415 von Typen, 422 von Formularen. Datenbanktexte bleiben generisch.

## Beispiel

Ein Fullstack-Projekt aus `ferrum new` ruft `mount_pages` und danach `ferrum::web::prepare` auf. `/contact` ist vor `into_router` gemerged.

## Tests

Die Middleware-Reihenfolge steht in `ferrum-middleware`. Auth-, CSRF- und CRUD-Tests bleiben die bestehenden SQLite-Tests.

## Einschränkungen

myshop und generierte Projekte unterscheiden sich im Einhängepunkt: myshop kennt die Workspace-Datei und montiert sein eigenes `static/`. `prepare` liest die entdeckte `ferrum.toml`.

## Erweiterung

Weitere äußere Schichten dürfen vor `prepare` liegen. Innere Auth-Middleware ist nicht geplant, solange die Handlerprüfung die Quelle der Entscheidung bleibt.
