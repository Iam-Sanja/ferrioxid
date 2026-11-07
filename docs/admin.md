# Ferrum Admin

[Zur Übersicht](index.md) · Backend: [admin-backend.md](admin-backend.md) · Oberfläche: [admin-frontend.md](admin-frontend.md) · Sicherheit: [admin-security.md](admin-security.md)

**Stand:** v0.13 rendert den Admin serverseitig. v0.13.1 bindet die Login-Origins an die tatsächliche Serveradresse. Die Cargo-Version bleibt `0.1.0`. Admin-Metadaten ändern das Datenbankschema nicht. Anlegen verwendet dieselbe `can_create`-Regel wie REST. Listen filtern in SQL. Das Verzeichnis bleibt Staff-Superusern vorbehalten. Branding kommt aus `ferrum.toml`. Details stehen in [admin-native.md](admin-native.md).

## Zweck

Ein Modell mit `admin = true` liefert Navigation, Tabelle, Formular, Detail und Löschen. Tera rendert diese Angaben. Autorisierung bleibt auf dem Server.

## Zuständige Dateien

| Bereich | Dateien |
|---|---|
| Metadaten | `crates/ferrum-core/src/lib.rs`, `crates/ferrum-macros/src/lib.rs` |
| Admin-API | `crates/ferrum-admin/src/` |
| Listenabfrage | `crates/ferrum-db/src/sql.rs` (`query_admin_page`) |
| Oberfläche | `crates/ferrum-admin/templates/`, `crates/ferrum-admin/static/` |
| Beispiel | `examples/myshop/src/models/`, `examples/myshop/src/main.rs` |
| Test | `examples/myshop/tests/admin_http.rs`, `examples/myshop/tests/admin_html.rs` |

## Datenfluss

Rust-Struct → `FerrumModel` erzeugt `AdminModelMeta` neben dem bestehenden `ModelDescriptor` → `AdminRegistry::add` → HTML unter `/admin/models/{modell}` und JSON unter `/admin/api`. Schreibende Aufrufe laufen durch Session, Staff-Prüfung, Modellrecht, CSRF und die vorhandene Repository-Validierung.

## Konfiguration

`App::with_admin(registry)` hängt HTML und JSON an. Vorlagen und CSS sind im Binary. Ohne konfigurierte Authentifizierung antwortet `GET /admin` mit 403. Der Ablauf steht in [admin-routing.md](admin-routing.md).

## Öffentliche Nutzung

```rust
let admin = ferrum::admin::AdminRegistry::new()
    .add::<Category>()
    .add::<Product>()
    .add::<InternalNote>();
App::new(db).with_auth(auth).with_admin(admin)
```

Die Oberfläche liegt unter `http://127.0.0.1:3000/admin`. Die API liegt unter `/admin/api`.

## Sicherheit

Jede Admin-Anfrage braucht eine gültige Sitzung und `is_staff = true`. `allow_anonymous` öffnet den Admin nicht. Ein Superuser ohne Staff-Flag kommt nicht hinein. Modellrechte bleiben nötig, außer die Admin-Berechtigung ist leer: dann darf nur ein Superuser das Modell bedienen. Objekt-Policy-Listen bleiben blockiert.

## Fehler

| Lage | Antwort |
|---|---|
| Keine Sitzung, HTML | 303 zur Anmeldung |
| Keine Sitzung, JSON | 401 |
| Kein Staff oder fehlendes Modellrecht | 403 |
| Unbekanntes Modell, falsche Sortierung, gesperrtes Feld | 404 oder 422 |
| Objektregel lehnt einen Datensatz ab | 404 |

## Tests

`examples/myshop/tests/admin_http.rs` prüft die JSON-API. `examples/myshop/tests/admin_html.rs` prüft Login, Liste, Suche, Filter, Sortierung, Blättern, Anlegen, Ändern, Löschen, Verzeichnis, CSRF und Escaping über HTML-Formulare.

## Grenzen

Kein Plugin-System, kein OAuth, kein MFA, kein Mandantensystem. Sprachen sind Deutsch und Englisch. Superuser-Flags lassen sich im Admin nicht setzen. Die Benutzerverwaltung ist Superusern vorbehalten und kein generisches Modell-CRUD.

## Später

SQL-Scopes für Objektregeln, eingebettete Assets, eigene Feld-Widgets und feiner steuerbare Verzeichnisrechte.
