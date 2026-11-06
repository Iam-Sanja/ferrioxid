# Ferrum Roadmap – Status und Umsetzungsvoraussetzungen

[Zur Übersicht](index.md) · **Stand:** CRUD und eine sichere Migrations-Teilmenge auf SQLite. PostgreSQL nutzt denselben Plan und ist nicht gegen einen lokalen Server gelaufen.

| Meilenstein | Status | Ziel | Voraussetzung |
|---|---|---|---|
| v0.1 | Code vorhanden; SQLite ist der lokale Standard und wird im Workspace getestet | Typed Model, Liste, GET, POST, PATCH, DELETE, sichere Migrationen | Optionaler PostgreSQL-Lauf nur mit erreichbarem Server |
| Migrationen | Teilweise vorhanden | Snapshot, Diff, `make`/`up`/`status`, sichere SQLite-Änderungen | OpenAPI, Rename, `down` und Tabellen-Neubau fehlen |
| v0.3 | Geplant | Auth/Policies, Admin-Metadaten und React-Admin | Auth und Schema-Versionen |
| v0.4 | Geplant | Beziehungen, Erweiterungspunkte, Deploy und Performance | belastbare v0.2/0.3-Tests |

## v0.1 enthalten

- Cargo-Workspace mit `ferrum-migrations` und dem Beispiel myshop.
- `FerrumModel`-Derive für `Uuid`, `String`, `i64`, `bool` und `Option` dieser Typen, außer nullable `id`.
- Typisierte Repository-Methoden `list`, `get`, `create`, `update`, `delete`.
- Versionierte Migrationen für neue Tabellen, sichere Spalten und ausdrücklich freigegebene Löschungen. Bootstrap ist Legacy.
- GET/POST/PATCH/DELETE. Listen haben einen `truncated`-Indikator, maximal 100 Elemente. Update-DTOs nutzen `PatchField`.
- Localhost-Demo mit SQLite-Datei. Docker Compose für PostgreSQL ist optional. Der PostgreSQL-HTTP-Test bleibt ignoriert.

## Nicht enthalten

Rename, Tabellen-Neubau, `migrate down`, Baseline, Relations, Auth, RBAC, Admin, OpenAPI, weitere CLI-Scaffolding-Kommandos, Pagination über Limit 100, API-Dockerfile und MySQL. SQLite, PostgreSQL und die sichere Migrations-Teilmenge sind enthalten.

## Reihenfolge für zukünftige Arbeit

1. **v0.1 verifizieren**: Rustfmt, Clippy, Tests, optional DB-HTTP-Test; Fehler zuerst beheben.
2. **Metadaten erweitern**: Felder, Typen, Constraints, Snapshot-Version.
3. **Migrationen erweitern**: Rename, Tabellen-Neubau, `down`, Baseline und Sperren. Die sichere Teilmenge ist vorhanden.
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
