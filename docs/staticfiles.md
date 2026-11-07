# Statische Dateien

**Stand:** v0.12, IST. Die Cargo-Version bleibt `0.1.0`.

## Verantwortlichkeit

`ferrum-static` liefert CSS, JavaScript, Bilder und Schriften aus einem ausdrücklich konfigurierten Verzeichnis. Eingebettete Admin-Assets unter `/admin` bleiben getrennt.

## Crates und Dateien

- `crates/ferrum-static/src/lib.rs`
- Einhängen: `ferrum::web::prepare` in `crates/ferrum-api/src/web.rs`
- Beispiel: `examples/myshop/static/`
- Generator: `static/css/app.css` und `static/js/app.js`

## Öffentliche API

`StaticOptions` und `mount`. Der Pfad ist `{url_prefix}/{*path}`. `serve` setzt `Content-Type`, `Content-Length`, `Cache-Control`, `ETag` und `X-Content-Type-Options: nosniff`. `If-None-Match` kann 304 liefern.

## Datenfluss

Die Route entfernt das Präfix, prüft jedes Segment und kanonisiert Root und Datei. Nur Dateien, deren kanonischer Pfad im Root beginnt, werden gelesen. Die Antwort enthält die Bytes und den ETag aus SHA-256.

## Konfiguration

```toml
[static]
enabled = true
url_prefix = "/static"
directory = "static"
cache_seconds = 3600
```

Neue Projekte setzen `enabled = true`. Die bestehende Workspace-Datei ohne Abschnitt lässt `enabled` auf `false`. `directory` ist relativ und darf `..` nicht enthalten. `url_prefix` darf `/admin`, `/api`, `/auth`, `/healthz` und `/uploads` nicht überdecken. `cache_seconds` ist höchstens 2592000. Die maximale Dateigröße ist 5 MiB.

## Sicherheit

Verboten sind leere Segmente, `\`, NUL, `.`, `..`, Punkt-Dateien, `.env`, SQLite-Dateien und `.pem`. Ein Symlink, der nach der Kanonisierung außerhalb des Roots liegt, wird nicht ausgeliefert. Verzeichnisse werden nicht aufgelistet. HTML ist kein erlaubter Typ. SVG aus dem Static-Root ist erlaubt, Upload-SVG nicht. Der Root muss existieren, sonst ist die Datei nicht erreichbar.

## Fehlerfälle

Fehlende, versteckte oder ausbrechende Pfade sind 404. Unbekannte Endungen sind 415. Zu große Dateien sind 413.

## Beispiel

`GET /static/css/app.css` nach `ferrum::static_files::mount(router, StaticOptions::new("static"))`.

## Tests

`cargo test -p ferrum-static` prüft CSS, JavaScript, PNG, MIME, 404, Cache, ETag, Traversal, versteckte Dateien und, wenn Windows den Symlink anlegen darf, einen Symlink nach außen.

## Einschränkungen

Es gibt keine Manifest-Fingerprints, kein Brotli und kein CDN. Der Entwicklungs- und der Produktionsmodus nutzen denselben Reader. Nur `cache_seconds` unterscheidet die Cache-Dauer.

## Erweiterung

Ein späterer Build-Schritt kann gehashte Dateinamen erzeugen. Die Route bleibt am konfigurierten Präfix.
