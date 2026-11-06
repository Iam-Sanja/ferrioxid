# Implementierungsleitfaden für ausführende Coding-KIs

[Zur Übersicht](index.md) · **Status:** Arbeitskonventionen und prüfbare Zielkriterien.

## 1. Erst den vorhandenen Code verstehen

Lies in dieser Reihenfolge `Cargo.toml`, [architecture.md](architecture.md), [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [testing.md](testing.md), bevor du die betroffene Crate änderst. **Behaupte keine Feature-Vollständigkeit auf Basis einer SOLL-Dokumentation.**

## 2. Modulgrenzen respektieren

- Core: nur gemeinsame Verträge, keine ORM-/Axum-Abhängigkeit.
- Macros: syn-Parsing, Typcheck, Codegen; keine DB-Verbindung.
- DB: Connection, typsicheres Repository, DB-Fehler; keine HTTP-Statuscodes.
- API: Axum-Router, DTO-Extractor, HTTP-Response, keine neue Modellsyntax.
- CLI: Commands und Orchestrierung, keine doppelte ORM-Logik.
- Beispiel: ausschließlich öffentliche Ferrum-Fassade, kein interner Sondercode.

## 3. Pflichtablauf je Änderung

1. Konkretes Ziel und betroffene Pfade nennen.
2. Öffentliches API-/Datenmodell auf Breaking Changes prüfen.
3. Kleinste korrekte Implementierung wählen.
4. Tests für Positiv-, Negativ- und Sicherheitsfall ergänzen.
5. `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` und `cargo build --workspace` ausführen. Der SQLite-HTTP-Test gehört zu `cargo test`. PostgreSQL nur mit laufendem Server und `--ignored` prüfen.
6. Jedes ausgeführte Kommando mit Ergebnis dokumentieren; fehlende Tools oder DB offen benennen.
7. Betroffene Dokumentationsseiten in IST/SOLL aktualisieren und Cross-Links prüfen.

## 4. Änderungsvorlage (für Issue/PR)

```text
Titel:
Ziel / Motivation:
Betroffene Dateien (relative Pfade):
IST-Verhalten:
SOLL-Verhalten:
Öffentliche API / Breaking Changes:
Sicherheit / DB-Auswirkungen:
Tests inkl. echter Ausführung:
Dokumentationsänderungen:
Bekannte Einschränkungen:
```

## 5. Technische Definition of Done

- Keine unbehandelten Compilerfehler, kein stummes Schema-Drift-Verhalten.
- Kein ungerechtfertigtes `allow`-Lint oder toter Pseudocode.
- API/Repo-Verträge sind typisiert; `serde_json::Value` bleibt bei Bedarf **intern**.
- Exponierte Endpunkte sind ohne explizite Freigabe nicht öffentlich.
- Negative Tests bestätigen notwendige Ablehnung.
- README und Teilbereichs-Dokumentation beschreiben das **tatsächlich Implementierte**.

## 6. Entscheidungspapiere (ADR)

Für richtungsweisende Entscheidungen kurze ADRs unter `docs/adr/` anlegen:

```text
ADR-0001: Titel
Status: proposed | accepted | superseded
Kontext:
Entscheidung:
Alternativen:
Folgen:
Migrationsaufwand:
```

Vorgeschlagene ADR-Themen: Macro ↔ ORM-Strategie; Schema-Format; Zugriffspolicy; Admin-Schema-API; Migrations- und Rollback-Semantik. SOLL heißt hier **vorgeschlagen** – keine bereits existierenden ADRs voraussetzen.

## 7. Aktueller Schwerpunkt

SQLite ist die lokale Standarddatenbank. PostgreSQL bleibt über `DATABASE_URL` unterstützt. Neue Spaltenabbildungen gehören nach `crates/ferrum-db/src/sql.rs`. Neue Migrationsregeln gehören nach `crates/ferrum-migrations` und müssen unsichere SQLite-Umbauten ablehnen, statt sie halb auszuführen. Authentifizierung, Admin und zusätzliche REST-Funktionen nur nach ausdrücklicher Beauftragung. Die Roadmap ist keine Freigabe.
