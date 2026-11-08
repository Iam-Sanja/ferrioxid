# Projektvorlagen

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrioxid-cli/src/scaffold.rs`.

## Aufgabe

Zwei Vorlagen unterscheiden eine reine API von einer Anwendung mit Authentifizierung und Admin. Beide erhalten Staticfiles und einen leeren Medienordner. Nur Fullstack registriert `/contact` und den Admin. Uploads werden nicht automatisch öffentlich gemacht.

## Zuständige Dateien

- `crates/ferrioxid-cli/src/scaffold.rs`
- `crates/ferrioxid-cli/src/models.rs`
- `crates/ferrioxid/Cargo.toml` (`admin-ui`)
- `crates/ferrioxid-admin/Cargo.toml` (`embedded-admin`)

## Öffentliche Schnittstelle

| Befehl | Inhalt |
|---|---|
| `--template api` | Registry, CRUD, SQLite, Migration, kein Admin-Feature |
| `--template fullstack` | zusätzlich Auth, Admin-Registry, Feature `admin-ui` |

Standard ist `fullstack`.

## Struktur

```text
meinprojekt/
├── Cargo.toml
├── ferrum.toml
├── .gitignore
├── .env.example
├── README.md
├── .ferrum/project.json
├── src/main.rs
├── src/lib.rs
├── src/settings.rs
├── src/urls.rs
├── src/views/mod.rs
├── src/views/home.rs
├── src/models/mod.rs
├── src/models/product.rs
├── templates/
├── static/css/
├── static/js/
├── migrations/0001_initial.json
└── tests/api.rs
```

`src/views/` und `templates/home.html` schreibt nur `--template fullstack`. Die API-Vorlage registriert `api.ping` und behält die HTML-Startseite. Die genaue Liste steht in [project-structure.md](project-structure.md). Die Template-Version ist `0.24`.

`Cargo.lock` entsteht beim ersten Cargo-Lauf.

## Konfiguration

Beide Vorlagen aktivieren Tera und eine Startseite. API: `admin.enabled` und `embedded_assets` sind falsch, der Admin-Link fehlt. Fullstack: Admin und Auth sind wahr. Das Produktmodell der API ist anonym. Das Fullstack-Modell ist zusätzlich im Admin sichtbar. Die generierte Binary lehnt eine Bind-Adresse ab, die mit `0.0.0.0` beginnt.

## Datenfluss

`ferrioxid generate model Customer` schreibt `src/models/customer.rs` und ergänzt Zeilen vor den Markierungen `ferrum:models`, `ferrioxid:uses`, `ferrioxid:schema`, `ferrioxid:registry` und, falls vorhanden, `ferrioxid:admin`. Die Tabelle heißt `customers`. Ein zweiter Aufruf desselben Namens ändert nichts und endet mit einem Fehler.

## Fehlerfälle

Ein ungültiger Modellname, eine fehlende Markierung oder ein bereits registriertes Modell bricht ab. Die übrigen Dateien bleiben dann unverändert, sofern der Schreibvorgang noch nicht begonnen hat. Der Generator ersetzt keine beliebigen Rust-Dateien außerhalb dieser Markierungen.

## Sicherheitsregeln

Die API-Vorlage verlangt das Admin-Feature nicht und hängt keine Admin-Routen ein. Fullstack-Projekte aktivieren das leere Feature `admin-ui` und registrieren den Admin. Beide Vorlagen kompilieren ohne Node.js. Das Frontend-Verzeichnis `apps/admin-ui` gibt es nicht mehr.

## Beispiel

```powershell
ferrioxid new shop --template api --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
cd shop
ferrioxid generate model Customer
```

## Tests

`generator.rs` prüft beide Vorlagen mit `cargo check`, die Modellregistrierung durch einen zweiten `cargo check` und die Ablehnung eines doppelten Namens.

## Einschränkungen

Besitzerfelder werden nicht automatisch aus der Sitzung gesetzt. Das ist geplant. Die Vorlage enthält ein anonymes Produktbeispiel, keine Objektregel. Authentifizierung im Fullstack-Projekt muss der Entwickler mit `ferrioxid admin create-user` und Rollen vervollständigen.
