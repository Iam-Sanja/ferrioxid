# Projektvorlagen

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-cli/src/scaffold.rs`.

## Aufgabe

Zwei Vorlagen unterscheiden eine reine API von einer Anwendung mit Authentifizierung und Admin. Beide erhalten Staticfiles und einen leeren Medienordner. Nur Fullstack registriert `/contact` und den Admin. Uploads werden nicht automatisch öffentlich gemacht.

## Zuständige Dateien

- `crates/ferrum-cli/src/scaffold.rs`
- `crates/ferrum-cli/src/models.rs`
- `crates/ferrum/Cargo.toml` (`admin-ui`)
- `crates/ferrum-admin/Cargo.toml` (`embedded-admin`)

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
├── src/main.rs
├── src/lib.rs
├── src/models/mod.rs
├── src/models/product.rs
├── migrations/0001_initial.json
└── tests/api.rs
```

`Cargo.lock` entsteht beim ersten Cargo-Lauf.

## Konfiguration

Beide Vorlagen aktivieren Tera und eine Startseite. API: `admin.enabled` und `embedded_assets` sind falsch, der Admin-Link fehlt. Fullstack: Admin und Auth sind wahr. Das Produktmodell der API ist anonym. Das Fullstack-Modell ist zusätzlich im Admin sichtbar. Die generierte Binary lehnt eine Bind-Adresse ab, die mit `0.0.0.0` beginnt.

## Datenfluss

`ferrum generate model Customer` schreibt `src/models/customer.rs` und ergänzt Zeilen vor den Markierungen `ferrum:models`, `ferrum:uses`, `ferrum:schema`, `ferrum:registry` und, falls vorhanden, `ferrum:admin`. Die Tabelle heißt `customers`. Ein zweiter Aufruf desselben Namens ändert nichts und endet mit einem Fehler.

## Fehlerfälle

Ein ungültiger Modellname, eine fehlende Markierung oder ein bereits registriertes Modell bricht ab. Die übrigen Dateien bleiben dann unverändert, sofern der Schreibvorgang noch nicht begonnen hat. Der Generator ersetzt keine beliebigen Rust-Dateien außerhalb dieser Markierungen.

## Sicherheitsregeln

Die API-Vorlage verlangt das Admin-Feature nicht. Ein Workspace-Build des Ferrum-Repositories selbst aktiviert das Feature, weil `examples/myshop` es anfordert. Ein generiertes API-Projekt tut das nicht und kompiliert ohne Node.js. Fullstack-Projekte erhalten beim Kompilieren einen Build-Fehler, wenn `apps/admin-ui/dist/index.html` fehlt.

## Beispiel

```powershell
ferrum new shop --template api --ferrum-path D:\04_Projekte_Arbeit\Projekte\ferrum
cd shop
ferrum generate model Customer
```

## Tests

`generator.rs` prüft beide Vorlagen mit `cargo check`, die Modellregistrierung durch einen zweiten `cargo check` und die Ablehnung eines doppelten Namens.

## Einschränkungen

Besitzerfelder werden nicht automatisch aus der Sitzung gesetzt. Das ist geplant. Die Vorlage enthält ein anonymes Produktbeispiel, keine Objektregel. Authentifizierung im Fullstack-Projekt muss der Entwickler mit `ferrum admin create-user` und Rollen vervollständigen.
