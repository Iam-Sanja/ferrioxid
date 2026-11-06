# Ferrum Roadmap – Status und Umsetzungsvoraussetzungen

[Zur Übersicht](index.md) · **Stand:** v0.1 mit CRUD. Format, Clippy und Unit-Tests wurden lokal ausgeführt. Der PostgreSQL-Test nicht.

| Meilenstein | Status | Ziel | Voraussetzung |
|---|---|---|---|
| v0.1 | Code vorhanden; Format, Clippy und Unit-Tests lokal grün; PostgreSQL-Test nicht ausgeführt | Typed Model, Liste, GET, POST, PATCH, DELETE, Bootstrap | PostgreSQL-Integrationstest auf einem laufenden Server |
| v0.2 | Geplant | Schema-Metadaten, sichere Migrationen, OpenAPI | stabile Model-/DTO-Verträge |
| v0.3 | Geplant | Auth/Policies, Admin-Metadaten und React-Admin | Auth und Schema-Versionen |
| v0.4 | Geplant | Beziehungen, Erweiterungspunkte, Deploy und Performance | belastbare v0.2/0.3-Tests |

## v0.1 enthalten

- Cargo-Workspace (sechs Framework-Crates, ein Beispiel).
- `FerrumModel`-Derive für `Uuid`, `String`, `i64`, `bool` und `Option` dieser Typen, außer nullable `id`.
- Typisierte Repository-Methoden `list`, `get`, `create`, `update`, `delete`.
- PostgreSQL-Bootstrap über `CREATE TABLE IF NOT EXISTS`.
- GET/POST/PATCH/DELETE. Listen haben einen `truncated`-Indikator, maximal 100 Elemente. Update-DTOs nutzen `PatchField`.
- Localhost-Demo, PostgreSQL-Compose, optionale ignorierte DB-Tests.

## Nicht enthalten

Migrations-Engine, Relations, Auth, RBAC, Admin, OpenAPI, echte CLI-Scaffolding-Kommandos, Pagination über Limit 100, API-Dockerfile oder andere DBs. PATCH, DELETE und GET-by-ID sind im aktuellen Code enthalten.

## Reihenfolge für zukünftige Arbeit

1. **v0.1 verifizieren**: Rustfmt, Clippy, Tests, optional DB-HTTP-Test; Fehler zuerst beheben.
2. **Metadaten erweitern**: Felder, Typen, Constraints, Snapshot-Version.
3. **DB-Migrationen sicher machen**: Diff, Review, Locks, History.
4. **DTO-/HTTP-Operationen erweitern**: PATCH/DELETE, Error Contracts, OpenAPI.
5. **Authentifizierung und Berechtigungen** als harte API-Grenze.
6. **Admin-Schema-Backend und React-Frontend** erst auf abgesicherten APIs.
7. **Erweiterungen und Deployment** zum Schluss production-orientiert testen.

Die einzelnen Ziele und Abnahmen sind in den jeweiligen Teilbereichs-Dokumenten beschrieben. Keine Fertigstellungszeit und keine Build-Erfolge behaupten, solange sie nicht nachgewiesen sind.

## Inkrementeller Ausbau nach v0.1 (Quellcode ergänzt, Verifikation ausstehend)

- Typisiertes `get(UUID)` mit parametrisiertem Select.
- Generierte Detailroute `GET /{id}` und 404-Fehler.
- 403-Sicherheitsprüfungen für nicht freigegebene Listen- und Detailrouten.
- Kein PATCH/DELETE, keine Migrationen, keine Authentifizierung.
