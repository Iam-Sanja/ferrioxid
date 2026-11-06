# Tests, Qualität und Continuous Integration

[Zur Übersicht](index.md) · **IST-Dateien:** `.github/workflows/rust.yml`, `examples/myshop/tests/postgres_api.rs`, `crates/ferrum-db/src/lib.rs`.

## Teststufen (IST)

1. **Build und Lint:** `cargo fmt`, Clippy mit `-D warnings` und `cargo build --workspace`.
2. **Workspace-Tests:** `cargo test --workspace` führt Unit-Tests und `sqlite_http_roundtrip` aus.
3. **SQLite-HTTP:** `sqlite_http_roundtrip` nutzt `sqlite::memory:`, eine Pool-Verbindung und einen Axum-Listener auf einem zufälligen Localhost-Port. Die Datei `db.sqlite3` wird nicht verwendet.
4. **DDL ohne Server:** `boolean_default_is_emitted_into_postgres_schema` prüft die gerenderte PostgreSQL- und SQLite-Definition von `active`.
5. **Migrationen:** Snapshot, Diff, SQLite-Kette und `ferrum migrate` gegen eine temporäre Datei laufen in `cargo test --workspace`.
6. **Auth:** Passwort-, Sitzungs-, Migrations- und HTTP-Tests in `ferrum-auth` und `examples/myshop/tests/auth.rs` nutzen SQLite. Sie sind nicht ignoriert.
7. **PostgreSQL, optional:** `postgres_http_roundtrip` und `postgres_migration_roundtrip` sind `#[ignore]`. Sie laufen nur mit `--ignored` und gesetzter `DATABASE_URL`.

### Lokale Befehle

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Optionaler PostgreSQL-Lauf, nur gegen eine entbehrliche Datenbank:

```powershell
docker compose up -d db
$env:DATABASE_URL="postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum"
cargo test -p myshop --test postgres_api -- --ignored --nocapture
```

Der ignorierte Test verlangt `DATABASE_URL` und bricht ohne sie ab. Er löscht vor und nach dem Lauf die Tabellen `ferrum_v01_integration_products`, `ferrum_v01_required_only`, `ferrum_v01_private_products` und `ferrum_v01_notes`. Das ist kein Nachweis, solange kein PostgreSQL-Server erreichbar ist. Ein früherer Lauf ohne Server endete mit `PoolTimedOut`.

## Abgedeckte SQLite-Fälle (IST)

Dieselben HTTP-Prüfungen laufen im SQLite-Test:

- GET-Liste, GET nach UUID, POST, PATCH und DELETE.
- POST 201 mit UUID, Feldern und Bool-Default `true`.
- Leerer oder nur aus Leerzeichen bestehender Name und negativer Preis ergeben 422.
- `#[field(required)]` ohne `min_length`.
- PATCH nur mit `name`, leeres `{}`, leerer Name, negativer Preis und unbekannte UUID.
- DELETE 204 ohne Körper, folgendes GET 404 und zweites DELETE 404.
- Nicht freigegebenes Modell: Liste, Detail, PATCH und DELETE mit 403 und Ferrum-JSON.
- Nullable `note`: fehlender Schlüssel, Wert und JSON-`null`.
- Nach mehr als 100 Zeilen: `limit` 100, `truncated: true`, genau 100 Objekte.
- `PRAGMA foreign_keys` ist auf der Testverbindung `1`.
- `PatchField` unterscheidet fehlend, `null` und Wert. Das Update-DTO lehnt `id` und `readonly` ab.

Die JSON-Struktur ist für beide Backends dieselbe, weil die Handler dialektunabhängig sind. Der PostgreSQL-Test führt denselben Ablauf aus, wurde in diesem Arbeitsgang aber nicht gegen einen Server ausgeführt.

## CI (IST)

`.github/workflows/rust.yml` startet rustfmt, Clippy und `cargo test --workspace`. Dabei läuft der SQLite-Test. Es wird kein PostgreSQL-Service gestartet.

Die Migrations-Tests prüfen gleiche Snapshots, deterministische Spaltenreihenfolge, doppelte Namen, unbekannte Versionen, neue Tabelle, neue Spalte, gelöschte Spalte, unterstützte Typ- und Default-Änderung, blockierte Typänderung, leeren Diff, riskante Änderung, Umbenennungsverweigerung, explizite Spalten- und Tabellenumbenennung, erfolgreichen und fehlgeschlagenen SQLite-Umbau, blockiertes `NOT NULL`, passende und abweichende Baseline, destruktive Ketten ohne Baseline, `reseal`, Erstanwendung, zweite Migration mit erhaltenen Zeilen, wiederholtes `up`, manipulierte Datei und vorhandene Tabellen ohne Historie. `tests/constraints.rs` prüft Unique inklusive `NULL` und zusammengesetzter Schlüssel, Index-Anlage, wiederholtes `up`, Index-Entfernen, gültige und ungültige Foreign Keys, `restrict`, `cascade`, `set_null`, abgelehntes `set_null` auf `NOT NULL`, Baseline mit Indizes, zurückgerollte Unique-Migration und blockierte Änderung der Löschregel. Eine Umbenennung einer indizierten Spalte benennt den Index in SQLite um. Der CLI-Test `migrate_make_up_and_status_use_a_temporary_sqlite_database` nutzt eine temporäre SQLite-Datei. `examples/myshop/tests/relations.rs` prüft Belongs-To, Has-Many, leere Kinder, optionales `None`, doppelten Kategorienamen, einen ungültigen Foreign Key und die eingecheckte Kette `0001` plus `0002`.

PostgreSQL-Migrations- und HTTP-Tests bleiben ignoriert, solange kein Server läuft. Ein ignorierter Test zählt nicht als bestanden.

Die Qualitätsbefehle dieser Umsetzung stehen im Abschlussbericht des Arbeitsgangs. Ignoriert bleiben `postgres_http_roundtrip` und `postgres_migration_roundtrip`, solange kein PostgreSQL-Server läuft. Ein ignorierter Test ist kein Erfolg.

## Geplanter Ausbau (SOLL)

| Bereich | Tests | Akzeptanz |
|---|---|---|
| Makros | Compile-Fail | Ungültige Attribute erzeugen klare Diagnostics |
| Migrationen | Diff, Drift, Locks | Kein stiller Datenverlust |
| DB | Transaktionen, Constraint-Mapping | Gleiche Fehlercodes auf beiden Backends |
| Auth | 401/403, Session, CSRF | Kein Rechte-Bypass |
| Admin und Deployment | UI und Container | Policy serverseitig, keine anonyme Produktion |

## Berichtspflicht

Ausgeführte Befehle, Exit-Codes und relevante Fehler nennen. Nicht ausgeführte Prüfungen als nicht ausgeführt kennzeichnen. Kein pauschales `allow(dead_code)`, wenn der Wert typsicher verwendet werden kann.

## Admin

SQLite: `cargo test -p myshop --test admin_http`. Frontend: in `apps/admin-ui` die Skripte `test`, `typecheck`, `lint` und `build`. `seed_file_database_when_requested` tut ohne `FERRUM_E2E_DATABASE` nichts. PostgreSQL-Admin-Tests gibt es nicht.
