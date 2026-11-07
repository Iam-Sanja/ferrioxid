# Implementierungsleitfaden für ausführende Coding-KIs

[Zur Übersicht](index.md) · **Status:** Arbeitskonventionen und prüfbare Zielkriterien. Funktionsstand v0.24 ergänzt [settings.md](settings.md), [urls.md](urls.md) und [views.md](views.md). Eine bestehende `main.rs` wird dafür nicht umgeschrieben. Funktionsstand v0.23 bleibt gültig. E-Mail und Reset: [mail.md](mail.md), [password-reset.md](password-reset.md), [notifications.md](notifications.md). Funktionsstand v0.22 bleibt gültig. Admin-Erweiterungen: [admin-extension-api.md](admin-extension-api.md). Diagnose und Upgrade: [migration-diagnostics.md](migration-diagnostics.md), [project-upgrade.md](project-upgrade.md). v0.20 bleibt gültig. Datum und Uhrzeit sind in [date-time-types.md](date-time-types.md) beschrieben. Transaktionen: [transactions.md](transactions.md). Bestehende angewendete Migrationen werden nicht umgeschrieben.

## 1. Erst den vorhandenen Code verstehen

Lies in dieser Reihenfolge `Cargo.toml`, [architecture.md](architecture.md), [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [auth.md](auth.md), [csrf.md](csrf.md), [query-scopes.md](query-scopes.md), [i18n.md](i18n.md), [error-codes.md](error-codes.md), [testing.md](testing.md), bevor du die betroffene Crate änderst. **Behaupte keine Feature-Vollständigkeit auf Basis einer SOLL-Dokumentation.** Auth-Tabellen laufen über `apply_directory_with_history` und `_ferrum_auth_migrations`. `0001_initial.json` wird nicht geändert. v0.12 ergänzt Formulare, Staticfiles, lokalen Speicher und Middleware. Upload-Metadaten bleiben Sidecar-JSON, damit die Migrationsprüfsummen gelten. S3 und ein Virenscanner sind nicht als fertig zu beschreiben.

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

## 7. Nächste Admin-Phasen

Lies [roadmap.md](roadmap.md), bevor du den Admin erweiterst. v0.14 bis v0.18 sind umgesetzt. Datum und Uhrzeit sind bewusst nicht ergänzt.

Impeccable, ein Cloudflare-Audit-Skill, ub-mode und Ponytail sind im Arbeitsbaum nicht als Skill hinterlegt. Installiere sie nicht nebenbei. Lies einen Skill erst nach ausdrücklicher Zustimmung und nur für die Phase, die ihn nennt. Kein Skill und kein Hook erzeugt Commits oder Pushes.

Ein Admin-Auftrag, der nur die Roadmap erweitert, ändert keine Handler und keine Templates außer der Dokumentation.

## 8. Aktueller Schwerpunkt

SQLite ist die lokale Standarddatenbank. PostgreSQL bleibt über `DATABASE_URL` unterstützt. Neue Spaltenabbildungen gehören nach `crates/ferrum-db/src/sql.rs`. Neue Migrationsregeln gehören nach `crates/ferrum-migrations`. Ein SQLite-Umbau darf nur laufen, wenn die geänderte Spalte vorher geprüft ist und die Transaktion bei Fehler alles zurücknimmt. Foreign Keys auf bestehenden Spalten werden nicht umgebaut. Authentifizierung ist vorhanden. Der Admin ist in v0.8 vorhanden und seit v0.13 serverseitiges HTML, beschrieben in [admin-native.md](admin-native.md). v0.9 ergänzt `can_create`, das Feature `admin-ui` und die Befehle `new`, `dev`, `doctor`, `check`, `generate model` und `build`. v0.10 ergänzt Settings und Tera-Seiten. Die Cargo-Version bleibt `0.1.0`. Many-to-Many und Eager Loading nur nach ausdrücklicher Beauftragung. Die Roadmap ist keine Freigabe. Constraints und Beziehungen sind in [constraints.md](constraints.md) und [relations.md](relations.md) beschrieben.
