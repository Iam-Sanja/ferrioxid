# Ferrum – Dokumentationsportal

**Projektstand:** v0.1 Source-Prototyp · **Dokumentationssprache:** Deutsch · **Verifikation:** aus Quellcode abgeleitet, nicht lokal gebaut.

## Wie dieses Handbuch zu lesen ist

Jedes Dokument trennt **IST (v0.1)** von **SOLL (Entwurf)**. SOLL bezeichnet eine Architekturentscheidung oder einen Vorschlag, **keine implementierte Funktion**. Beispiel-Code und CLI-Aufrufe in SOLL-Abschnitten sind nicht ausführbar, solange die Funktion nicht implementiert wurde.

**Zentrale Regel:** Rust-Struct → `FerrumModel`-Makro → typisierte Repository-Implementierung und Modelldeskriptor → explizite Registrierung → HTTP-Routen und SQL-Bootstrap. Nur diese schmale Kette ist in v0.1 im Quellcode vorhanden.

## Teilbereiche und ihre Dokumente

| Modul | Zustand | Dokument | Kernverantwortung |
|---|---|---|---|
| Gesamtarchitektur | IST + SOLL | [architecture.md](architecture.md) | Grenzen, Abhängigkeiten und Datenfluss |
| Öffentliche API (`ferrum`) | IST | [facade.md](facade.md) | Re-Exports, Prelude, Stabilitätsgrenze |
| `ferrum-core` | IST + SOLL | [core.md](core.md) | Traits, Metadaten, Fehler |
| `ferrum-macros` | IST + SOLL | [macros.md](macros.md) | Parsing, Validierung, Codegenerierung |
| `ferrum-db` | IST + SOLL | [database.md](database.md) | Typisierte Repositories, Connection, SQL |
| `ferrum-api` | IST + SOLL | [api.md](api.md) | Axum-Routing, DTOs, Fehlerbehandlung |
| `ferrum-cli` | IST + SOLL | [cli.md](cli.md) | Entwicklerbefehle und Kommandoverträge |
| `myshop` | IST | [example-myshop.md](example-myshop.md) | Integrationsbeispiel und Start |
| Konfiguration | IST + SOLL | [configuration.md](configuration.md) | Environment, Bind, Secrets |
| Sicherheit | IST + SOLL | [security.md](security.md) | Access Defaults, Bedrohungen, Checks |
| Datenvalidierung | IST + SOLL | [validation.md](validation.md) | Regeln und Fehlersemantik |
| Tests & CI | IST + SOLL | [testing.md](testing.md) | Testmatrix, reale PostgreSQL-Prüfung |
| Migrationen | **nur SOLL** | [migrations.md](migrations.md) | Snapshots, Diffs, sichere Ausführung |
| Auth & Rechte | **nur SOLL** | [auth.md](auth.md) | Sessions, Policies, Deny by default |
| Admin-Backend & UI | **nur SOLL** | [admin.md](admin.md) | Berechtigte Metadaten, React-UI |
| OpenAPI | **nur SOLL** | [openapi.md](openapi.md) | Dokumentation aus DTOs |
| Erweiterungen | **nur SOLL** | [extensions.md](extensions.md) | Hooks, Plugin-Grenzen |
| Deployment | IST Docker-DB + SOLL App | [deployment.md](deployment.md) | Docker, Proxy, Produktion |
| Umsetzungsprozess | IST + SOLL | [implementation-guide.md](implementation-guide.md) | Arbeitsaufträge, Abnahme, ADR |
| Roadmap | Planung | [roadmap.md](roadmap.md) | Meilensteine und Reihenfolge |

## Verbindliche Begriffe

- **Model:** Entwicklerseitige, serialisierbare Rust-Struct mit `#[derive(FerrumModel)]`.
- **Deskriptor:** Derzeit `ModelDescriptor` mit Name, Tabelle, API-Pfad und `allow_anonymous`; kein Feldkatalog.
- **Repository:** Generierte typisierte Methoden `list`, `get`, `create`, `update` und `delete`. `update` nimmt `{Model}Update` mit `PatchField<T>`.
- **Bootstrap:** `CREATE TABLE IF NOT EXISTS`; kein Schema-Diff, keine Migration.
- **Registrierung:** `App::new(db).register::<Product>()`, explizit und typisiert.
- **SOLL:** Geplante Funktion; wird nicht als fertiges Feature kommuniziert.

## Verifikationsbasis

Dateien: `crates/*/src/*`, `examples/myshop/src/*`, `examples/myshop/tests/postgres_api.rs`, `Cargo.toml`, `.github/workflows/rust.yml`, `compose.yaml`. Der letzte lokale Lauf von Format, Clippy und `cargo test --workspace` ist in [testing.md](testing.md) festgehalten. Der PostgreSQL-Integrationstest bleibt davon getrennt und gilt ohne laufende Datenbank nicht als bestanden.
