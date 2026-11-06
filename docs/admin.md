# Ferrum Admin

[Zur Übersicht](index.md) · Backend: [admin-backend.md](admin-backend.md) · Oberfläche: [admin-frontend.md](admin-frontend.md) · Sicherheit: [admin-security.md](admin-security.md)

**Stand:** implementiert in v0.8, Create-Policy und Asset-Feature in v0.9. Die Cargo-Version bleibt `0.1.0`. Admin-Metadaten ändern das Datenbankschema nicht. Admin-Create verwendet dieselbe `can_create`-Regel wie REST.

## Zweck

Ein Modell mit `admin = true` liefert Navigation, Tabelle, Formular, Detail und Löschen. Die React-Oberfläche rendert diese Angaben. Autorisierung bleibt auf dem Server.

## Zuständige Dateien

| Bereich | Dateien |
|---|---|
| Metadaten | `crates/ferrum-core/src/lib.rs`, `crates/ferrum-macros/src/lib.rs` |
| Admin-API | `crates/ferrum-admin/src/` |
| Listenabfrage | `crates/ferrum-db/src/sql.rs` (`query_admin_page`) |
| Oberfläche | `apps/admin-ui/src/` |
| Beispiel | `examples/myshop/src/models/`, `examples/myshop/src/main.rs` |
| Test | `examples/myshop/tests/admin_http.rs`, `apps/admin-ui/src/admin.test.tsx` |

## Datenfluss

Rust-Struct → `FerrumModel` erzeugt `AdminModelMeta` neben dem bestehenden `ModelDescriptor` → `AdminRegistry::add` → `GET /admin/api/models/{modell}/schema` → React-Formular. Schreibende Aufrufe laufen durch Session, Staff-Prüfung, Modellrecht, CSRF und die vorhandene Repository-Validierung.

## Konfiguration

`App::with_admin(registry)` hängt die Admin-Routen an. Das Frontend wird beim Rust-Build eingebettet. `FERRUM_ADMIN_DIST` bleibt ein Entwicklungs-Override. Fehlt beides, antwortet `/admin` mit 503. Der Ablauf steht in [admin-assets.md](admin-assets.md).

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
| Keine Sitzung | 401 |
| Kein Staff oder fehlendes Modellrecht | 403 |
| Unbekanntes Modell, falsche Sortierung, gesperrtes Feld | 404 oder 422 |
| Objektregel lehnt einen Datensatz ab | 404 |
| Admin-Build fehlt | 503 |

## Tests

`examples/myshop/tests/admin_http.rs` prüft Registry, CRUD, Whitelist, CSRF-Bootstrap, anonyme Anfrage, Staff ohne Recht, Objektregel und Superuser-Sperre gegen SQLite. `apps/admin-ui` prüft Login, Navigation, Tabelle, Validierung, gesperrte Aktion und Löschdialog.

## Grenzen

Kein Plugin-System, kein OAuth, kein MFA, keine Mehrsprachigkeit, kein Mandantensystem. Superuser-Flags lassen sich im Admin nicht setzen. Die Benutzerverwaltung ist Superusern vorbehalten und kein generisches Modell-CRUD.

## Später

SQL-Scopes für Objektregeln, eingebettete Assets, eigene Feld-Widgets und feiner steuerbare Verzeichnisrechte.
