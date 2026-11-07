# Ferrum – Dokumentationsportal

**Projektstand:** Funktionsstand v0.27.1 ergänzt Anwendungs-Worker und verdeckte Job-Payloads: [workers.md](workers.md), [job-security.md](job-security.md). Funktionsstand v0.27 ergänzt Hintergrundjobs: [jobs.md](jobs.md), [workers.md](workers.md), [job-retries.md](job-retries.md), [job-security.md](job-security.md), [job-mail-integration.md](job-mail-integration.md). Funktionsstand v0.26 ergänzt Modellfelder: [models.md](models.md), [model-components.md](model-components.md), [model-timestamps.md](model-timestamps.md), [model-soft-delete.md](model-soft-delete.md), [app-lifecycle.md](app-lifecycle.md). Funktionsstand v0.25 ergänzt Apps: [apps.md](apps.md), [app-registry.md](app-registry.md), [app-migrations.md](app-migrations.md), [reusable-apps.md](reusable-apps.md), [create-app.md](create-app.md). Funktionsstand v0.24 ergänzt Settings, benannte Routen und Views: [settings.md](settings.md), [custom-settings.md](custom-settings.md), [urls.md](urls.md), [routing.md](routing.md), [views.md](views.md), [project-structure.md](project-structure.md). Funktionsstand v0.23 bleibt gültig. E-Mail: [mail.md](mail.md), [mail-smtp.md](mail-smtp.md), [mail-security.md](mail-security.md). Passwort-Reset: [password-reset.md](password-reset.md). Hinweise: [notifications.md](notifications.md). Funktionsstand v0.22 bleibt gültig. Erweiterung: [admin-extension-api.md](admin-extension-api.md), [admin-custom-views.md](admin-custom-views.md), [admin-custom-filters.md](admin-custom-filters.md), [admin-custom-widgets.md](admin-custom-widgets.md). Diagnose: [migration-diagnostics.md](migration-diagnostics.md). Upgrade: [project-upgrade.md](project-upgrade.md). Manifest: [project-manifest.md](project-manifest.md). Legacy: [legacy-projects.md](legacy-projects.md). v0.20 bleibt gültig. Die Cargo-Version bleibt `0.1.0`. Der Admin ist serverseitiges HTML: [admin-native.md](admin-native.md). Transaktionen: [transactions.md](transactions.md). Verzeichnis-Audit: [admin-directory-audit.md](admin-directory-audit.md). Eltern-Kind-Formulare: [admin-transactional-inlines.md](admin-transactional-inlines.md). Entwürfe, Inlines, Sammelaktionen und Historie: [admin-form-drafts.md](admin-form-drafts.md), [admin-inlines.md](admin-inlines.md), [admin-bulk-actions.md](admin-bulk-actions.md), [admin-audit-log.md](admin-audit-log.md), [admin-custom-actions.md](admin-custom-actions.md). Datum und Uhrzeit: [date-time-types.md](date-time-types.md). SQLite ist die lokale Standarddatenbank, PostgreSQL das optionale Backend. **Dokumentationssprache:** Deutsch

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
| `ferrum-cli` | IST | [cli.md](cli.md), [migration-diagnostics.md](migration-diagnostics.md), [project-upgrade.md](project-upgrade.md), [project-generator.md](project-generator.md), [development-server.md](development-server.md), [cli-doctor.md](cli-doctor.md), [project-templates.md](project-templates.md) | Generator, Diagnose, Upgrade, Entwicklungsserver |
| `myshop` | IST | [example-myshop.md](example-myshop.md) | Integrationsbeispiel und Start |
| Konfiguration | IST v0.24 | [settings.md](settings.md), [custom-settings.md](custom-settings.md), [configuration.md](configuration.md) | `ferrum.toml`, Umgebung, eigene Abschnitte |
| URLs und Views | IST v0.24 | [urls.md](urls.md), [routing.md](routing.md), [views.md](views.md), [project-structure.md](project-structure.md) | Benannte Axum-Routen, Antworten, Projektlayout |
| Jobs | IST v0.27.1 | [jobs.md](jobs.md), [workers.md](workers.md), [job-retries.md](job-retries.md), [job-security.md](job-security.md), [job-mail-integration.md](job-mail-integration.md) | Queue, Anwendungs-Worker, verdeckte Payload, Mail ohne Reset-Token |
| Apps | IST v0.26 | [apps.md](apps.md), [app-registry.md](app-registry.md), [app-migrations.md](app-migrations.md), [app-lifecycle.md](app-lifecycle.md), [reusable-apps.md](reusable-apps.md), [create-app.md](create-app.md) | Typisierte Registrierung, geprüfter Start, App-Migrationen |
| Modelle | IST v0.26 | [models.md](models.md), [model-components.md](model-components.md), [model-timestamps.md](model-timestamps.md), [model-soft-delete.md](model-soft-delete.md) | Eine Model-Definition, flache Feldgruppen, verwaltete Zeitstempel |
| Sicherheit | IST + SOLL | [security.md](security.md) | Access Defaults, Bedrohungen, Checks |
| Datenvalidierung | IST + SOLL | [validation.md](validation.md) | Regeln und Fehlersemantik |
| Tests & CI | IST + SOLL | [testing.md](testing.md) | SQLite-HTTP im Workspace-Test, PostgreSQL optional |
| Migrationen | IST, mit benannten Grenzen | [migrations.md](migrations.md) | Snapshots, Constraints, explizite Umbenennung, geprüfter SQLite-Umbau, Baseline |
| Constraints und Indizes | IST | [constraints.md](constraints.md) | Unique, Index, Foreign Key, SQL und Fehlercodes |
| Modellbeziehungen | IST, Lazy Loading | [relations.md](relations.md) | Belongs-To, Has-Many, N+1 |
| Auth & Rechte | IST | [auth.md](auth.md), [password-reset.md](password-reset.md), [sessions.md](sessions.md), [permissions.md](permissions.md), [policies.md](policies.md), [csrf.md](csrf.md), [query-scopes.md](query-scopes.md) | Benutzer, Sitzungen, Reset, RBAC, Objektregeln, Scopes |
| E-Mail | IST v0.23 | [mail.md](mail.md), [mail-smtp.md](mail-smtp.md), [notifications.md](notifications.md), [mail-security.md](mail-security.md) | Versand, Vorlagen, Hinweise, keine Zustellgarantie |
| Formulare | IST v0.12 | [forms.md](forms.md), [forms-validation.md](forms-validation.md) | Typisierte Felder, Tera-HTML, gemeinsamer Regelkern |
| Staticfiles | IST v0.12 | [staticfiles.md](staticfiles.md) | Projekt-CSS und JavaScript, getrennt vom Admin |
| Speicher und Uploads | IST v0.12, S3 geplant | [storage.md](storage.md), [uploads.md](uploads.md) | Lokaler Speicher, private Dateien |
| Middleware | IST v0.12 | [middleware.md](middleware.md), [request-lifecycle.md](request-lifecycle.md) | Request-ID, Log, Limits, Header |
| Admin | IST v0.22 | [admin.md](admin.md), [admin-native.md](admin-native.md), [admin-extension-api.md](admin-extension-api.md), [transactions.md](transactions.md), [admin-directory-audit.md](admin-directory-audit.md), [admin-transactional-inlines.md](admin-transactional-inlines.md), [admin-form-drafts.md](admin-form-drafts.md), [admin-inlines.md](admin-inlines.md), [admin-bulk-actions.md](admin-bulk-actions.md), [admin-audit-log.md](admin-audit-log.md), [admin-custom-actions.md](admin-custom-actions.md), [admin-django-parity.md](admin-django-parity.md) | HTML-Oberfläche, JSON-API, Rechte, Entwürfe, Inlines, Transaktionen, Sammelaktionen, Historie, registrierte Erweiterungen |
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

Dateien: `crates/*/src/*`, `examples/myshop/src/*`, `examples/myshop/tests/postgres_api.rs`, `Cargo.toml`, `.github/workflows/ci.yml`, `compose.yaml`. Der SQLite-HTTP-Test gehört zu `cargo test --workspace`. Der PostgreSQL-Integrationstest bleibt ignoriert und gilt ohne laufenden Server nicht als bestanden.
