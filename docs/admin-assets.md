# Admin-Assets

[Zur Übersicht](admin.md) · Nativ: [admin-native.md](admin-native.md)

## Aufgabe

CSS und das optionale Script werden mit dem Rust-Binary ausgeliefert. Node.js startet dafür weder beim Bauen noch zur Laufzeit.

## Dateien

- `crates/ferrioxid-admin/static/css/admin.css`
- `crates/ferrioxid-admin/static/js/admin.js`
- `crates/ferrioxid-admin/src/assets.rs`
- `crates/ferrioxid-admin/build.rs`

`assets.rs` bindet die beiden Dateien mit `include_str!` ein und liefert nur `admin.css` und `admin.js`. Jeder andere Pfad, auch mit `..`, ist 404. Es gibt kein Verzeichnis-Override und kein `FERRUM_ADMIN_DIST`.

## Feature

`admin-ui` auf `ferrioxid` schaltet `ferrioxid-admin/embedded-admin` ein. Dieses Feature ist leer. Bestehende Fullstack-Projekte, die es in der `Cargo.toml` nennen, kompilieren weiter. Ein API-Projekt ohne das Feature kompiliert ebenfalls. Der Admin wird in beiden Fällen nicht aus einem Frontend-Build gelesen. Ob die Routen existieren, entscheidet `App::with_admin`.

## Test

`admin_assets_reject_traversal_and_keep_the_api` prüft, dass ein gesetztes `FERRUM_ADMIN_DIST` die Antworten nicht verändert, dass CSS und Script aus dem Binary kommen, unbekannte Pfade 404 sind und `/admin/api/models` JSON bleibt.
