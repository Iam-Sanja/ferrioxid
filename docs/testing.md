# Tests, Qualität und Continuous Integration

[Zur Übersicht](index.md) · **IST-Dateien:** `.github/workflows/rust.yml`, `examples/myshop/tests/postgres_api.rs`.

## Teststufen (IST)

1. **Build/Static:** Cargo und Clippy prüfen den gesamten Workspace.
2. **Rust-Tests:** `cargo test --workspace` führt Rust-Tests aus; ein mit `#[ignore]` markierter PostgreSQL-Test läuft nicht standardmäßig.
3. **Echter HTTP/PostgreSQL-Test:** Der optionale Test `postgres_http_roundtrip` startet einen Axum-Listener auf einem zufälligen Localhost-Port, führt SQL-Bootstrap aus und testet POST/GET.
4. **Schema-Test:** `boolean_default_is_emitted_into_postgres_schema` prüft die DDL-Zeichenkette ohne Datenbank.

### Lokale Befehle (Projekt-Root)

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

PowerShell für optionale DB-Prüfung:

```powershell
docker compose up -d db
$env:DATABASE_URL="postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum"
cargo test -p myshop --test postgres_api -- --ignored --nocapture
```

Linux/macOS:

```bash
docker compose up -d db
DATABASE_URL=postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum cargo test -p myshop --test postgres_api -- --ignored --nocapture
```

**WARNUNG:** Der Test entfernt die festen Tabellen `ferrum_v01_integration_products` und `ferrum_v01_required_only` mit `DROP TABLE IF EXISTS` vor/nach dem Lauf. Nur auf einer **entbehrlichen lokalen Entwicklungsdatenbank** ausführen. Wenn `DATABASE_URL` fehlt, beendet sich der ignorierte Test nach Aktivierung ohne eigentliche DB-Prüfung: Das ist **kein** Nachweis einer erfolgreichen Datenbankintegration.

## Bereits abgedeckte Fälle (Quellcode)

- 201 bei Create, servergenerierte UUID, Felder und Bool-Default.
- GET liefert 200, `limit:100`, initial `truncated:false`.
- Invalid Create: leerer/Whitespace-Name und negativer Preis → 422.
- `#[field(required)]` ohne `min_length` wird im zweiten Testmodell geprüft.
- PATCH nur `name`, leeres `{}`, leerer Name, negativer Preis, unbekannte UUID.
- DELETE mit 204 ohne Körper, folgendes GET und zweites DELETE mit 404.
- Privates Modell: PATCH und DELETE liefern 403 mit `title`, `status` und `detail`.
- Nullable `note`: fehlendes Feld, Wert und JSON-`null`.
- Typisierte Repository-Liste wird gelesen.
- Nach über 100 Datensätzen: `truncated:true`, genau 100 Datenobjekte.
- DDL enthält `BOOLEAN NOT NULL DEFAULT TRUE`.
- Unit-Tests ohne Datenbank: `PatchField`-Deserialisierung und Ablehnung von `id` sowie `readonly` im Update-DTO.

## CI-Zustand (IST)

`.github/workflows/rust.yml` startet rustfmt, Clippy und `cargo test --workspace` auf Ubuntu. **Es startet keinen PostgreSQL-Service.** Die DB/HTTP-Prüfung ist daher nicht Teil der Standard-CI.

Lokaler Lauf in diesem Workspace nach dem CRUD-Ausbau: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` und `cargo test --workspace` waren erfolgreich. Bestanden haben die Unit-Tests `patch_field_distinguishes_missing_null_and_value`, `boolean_default_is_emitted_into_postgres_schema` und `patch_dto_distinguishes_missing_null_value_and_rejects_readonly_fields`. `postgres_http_roundtrip` blieb ignoriert. Ein früherer Versuch mit gesetzter `DATABASE_URL` ohne laufenden Server endete mit `PoolTimedOut` und zählt nicht als bestandener Datenbanktest. Auf diesem Rechner sind weder Docker noch PostgreSQL installiert.

## Geplanter Testausbau (SOLL)

| Bereich | Tests | Akzeptanz |
|---|---|---|
| Makros | Compile-Pass/Compile-Fail | Ungültige Attribute → klare Diagnostics |
| Core | Model Registry | Deterministische, eindeutige Registrierung |
| DB | PostgreSQL E2E/Transaction | Parametrisierung, Rollback, Fehlermapping |
| API | Status, DTO, JSON-Fehler, Grenzen | Identischer dokumentierter Contract |
| Migrationen | Diff, Drift, Locks, Failure Recovery | Kein stiller Datenverlust |
| Auth | 401/403, Session, CSRF, IDOR | Kein Rechte-Bypass |
| Admin | E2E für UI/Backend | Policy auch serverseitig durchgesetzt |
| Deployment | Container/Proxy | Kein anonymer Public-Default |

## Berichtspflicht für Coding-KIs

Nach jeder Änderung: vollständige Befehle, Exit-Codes und relevante Fehlermeldungen angeben; nicht ausgeführte Prüfungen ausdrücklich als **nicht ausgeführt** markieren. Kein `#[allow(dead_code)]` zur Warnungsunterdrückung, wenn stattdessen echte typsichere Nutzung möglich ist.

## Ergänzte Detail- und Sicherheitstests (Code vorhanden, Lauf noch ausstehend)

Der ignorierte PostgreSQL-HTTP-Test prüft zusätzlich GET per UUID (200), eine nicht vorhandene UUID (404) sowie anonyme Listen- und Detailzugriffe auf ein Modell ohne Freigabe (403). Wenn der Test manuell mit `--ignored` gestartet wird, ist `DATABASE_URL` Pflicht: fehlende Konfiguration gilt als Fehler, nicht als bestandener Test.
