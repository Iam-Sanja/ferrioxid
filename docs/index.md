# Ferrum – Dokumentationsportal

**Projektstand:** Funktionsstand v0.13. Die Cargo-Version bleibt `0.1.0`. Der Admin ist serverseitiges HTML: [admin-native.md](admin-native.md). SQLite ist die lokale Standarddatenbank, PostgreSQL das optionale Backend. **Dokumentationssprache:** Deutsch

## Wie dieses Handbuch zu lesen ist

Jedes Dokument trennt **IST** von **SOLL (Entwurf)**. SOLL bezeichnet eine Architekturentscheidung oder einen Vorschlag, **keine implementierte Funktion**. Beispiel-Code und CLI-Aufrufe in SOLL-Abschnitten sind nicht ausführbar, solange die Funktion nicht implementiert wurde. Authentifizierung ist v0.7. Der automatische Admin ist v0.8. Query-Scopes, stabiles CSRF und eingebettete Admin-Assets sind v0.8.1. Settings, Tera-Seiten und die vorbereitete Zweisprachigkeit sind v0.10: [settings.md](settings.md), [templates.md](templates.md), [i18n.md](i18n.md). Vollständige Admin-Übersetzung, Fehlercodes und Branding sind v0.11: [localization.md](localization.md), [error-codes.md](error-codes.md), [admin-directory.md](admin-directory.md). Create-Policies, das Admin-Feature und die Entwickler-CLI sind v0.9: [project-generator.md](project-generator.md), [project-templates.md](project-templates.md), [development-server.md](development-server.md), [cli-doctor.md](cli-doctor.md). Formulare, Staticfiles, Uploads und Middleware sind v0.12: [forms.md](forms.md), [forms-validation.md](forms-validation.md), [staticfiles.md](staticfiles.md), [storage.md](storage.md), [uploads.md](uploads.md), [middleware.md](middleware.md), [request-lifecycle.md](request-lifecycle.md). Die Cargo-Version bleibt `0.1.0`.

**Zentrale Regel:** Rust-Struct → `FerrumModel`-Makro → typisierte Repository-Implementierung und Modelldeskriptor → explizite Registrierung → HTTP-Routen. Schemaänderungen laufen über die Migrations-Engine. `myshop --bootstrap` ist Legacy.

## Teilbereiche und ihre Dokumente

| Modul | Zustand | Dokument | Kernverantwortung |
|---|---|---|---|
| Gesamtarchitektur | IST + SOLL | [architecture.md](architecture.md) | Grenzen, Abhängigkeiten und Datenfluss |
| Öffentliche API (`ferrum`) | IST | [facade.md](facade.md) | Re-Exports, Prelude, Stabilitätsgrenze |
| `ferrum-core` | IST + SOLL | [core.md](core.md) | Traits, Metadaten, Fehler |
| `ferrum-macros` | IST + SOLL | [macros.md](macros.md) | Parsing, Validierung, Codegenerierung |
| `ferrum-db` | IST + SOLL | [database.md](database.md) | Typisierte Repositories, Connection, SQL |
| `ferrum-api` | IST + SOLL | [api.md](api.md) | Axum-Routing, DTOs, Fehlerbehandlung |
| `ferrum-cli` | IST | [cli.md](cli.md), [project-generator.md](project-generator.md), [development-server.md](development-server.md), [cli-doctor.md](cli-doctor.md), [project-templates.md](project-templates.md) | Generator, Diagnose, Entwicklungsserver |
| `myshop` | IST | [example-myshop.md](example-myshop.md) | Integrationsbeispiel und Start |
| Konfiguration | IST + SOLL | [configuration.md](configuration.md) | Environment, Bind, Secrets |
| Sicherheit | IST + SOLL | [security.md](security.md) | Access Defaults, Bedrohungen, Checks |
| Datenvalidierung | IST + SOLL | [validation.md](validation.md) | Regeln und Fehlersemantik |
| Tests & CI | IST + SOLL | [testing.md](testing.md) | SQLite-HTTP im Workspace-Test, PostgreSQL optional |
| Migrationen | IST, mit benannten Grenzen | [migrations.md](migrations.md) | Snapshots, Constraints, explizite Umbenennung, geprüfter SQLite-Umbau, Baseline |
| Constraints und Indizes | IST | [constraints.md](constraints.md) | Unique, Index, Foreign Key, SQL und Fehlercodes |
| Modellbeziehungen | IST, Lazy Loading | [relations.md](relations.md) | Belongs-To, Has-Many, N+1 |
| Auth & Rechte | IST | [auth.md](auth.md), [sessions.md](sessions.md), [permissions.md](permissions.md), [policies.md](policies.md), [csrf.md](csrf.md), [query-scopes.md](query-scopes.md) | Benutzer, Sitzungen, RBAC, Objektregeln, Scopes |
| Formulare | IST v0.12 | [forms.md](forms.md), [forms-validation.md](forms-validation.md) | Typisierte Felder, Tera-HTML, gemeinsamer Regelkern |
| Staticfiles | IST v0.12 | [staticfiles.md](staticfiles.md) | Projekt-CSS und JavaScript, getrennt vom Admin |
| Speicher und Uploads | IST v0.12, S3 geplant | [storage.md](storage.md), [uploads.md](uploads.md) | Lokaler Speicher, private Dateien |
| Middleware | IST v0.12 | [middleware.md](middleware.md), [request-lifecycle.md](request-lifecycle.md) | Request-ID, Log, Limits, Header |
| Admin | IST v0.13 | [admin.md](admin.md), [admin-native.md](admin-native.md), [admin-routing.md](admin-routing.md), [admin-templates.md](admin-templates.md), [admin-design-system.md](admin-design-system.md), [admin-backend.md](admin-backend.md), [admin-security.md](admin-security.md), [admin-directory.md](admin-directory.md), [admin-migration-from-react.md](admin-migration-from-react.md) | HTML-Oberfläche, JSON-API, Rechte, Verzeichnis |
| OpenAPI | **nur SOLL** | [openapi.md](openapi.md) | Dokumentation aus DTOs |
| Erweiterungen | **nur SOLL** | [extensions.md](extensions.md) | Hooks, Plugin-Grenzen |
| Deployment | IST SQLite, optionales Docker-PostgreSQL, SOLL App | [deployment.md](deployment.md) | Lokaler Start, Proxy, Produktion |
| Umsetzungsprozess | IST + SOLL | [implementation-guide.md](implementation-guide.md) | Arbeitsaufträge, Abnahme, ADR |
| Roadmap | Planung | [roadmap.md](roadmap.md) | Meilensteine und Reihenfolge |

## Verbindliche Begriffe

- **Model:** Entwicklerseitige, serialisierbare Rust-Struct mit `#[derive(FerrumModel)]`.
- **Deskriptor:** `ModelDescriptor` mit Name, Tabelle, API-Pfad, `allow_anonymous`, `permission` und `requires_object_policy`. Spalten stehen zusätzlich in `ColumnSpec`.
- **Repository:** Generierte typisierte Methoden `list`, `get`, `create`, `update` und `delete`. `update` nimmt `{Model}Update` mit `PatchField<T>`.
- **Migration:** versionierte Datei unter `migrations/`, angewendet über `ferrum migrate up`.
- **Bootstrap:** Legacy-Befehl ohne Schemaänderung. `App::bootstrap_schema` bleibt ein Testhelfer für leere Datenbanken ohne Historie.
- **Registrierung:** `App::new(db).register::<Product>()`, explizit und typisiert.
- **SOLL:** Geplante Funktion; wird nicht als fertiges Feature kommuniziert.

## Verifikationsbasis

Dateien: `crates/*/src/*`, `examples/myshop/src/*`, `examples/myshop/tests/postgres_api.rs`, `Cargo.toml`, `.github/workflows/rust.yml`, `compose.yaml`. Der SQLite-HTTP-Test gehört zu `cargo test --workspace`. Der PostgreSQL-Integrationstest bleibt ignoriert und gilt ohne laufenden Server nicht als bestanden.
