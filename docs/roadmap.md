# Ferrum Roadmap – Status und Umsetzungsvoraussetzungen

[Zur Übersicht](index.md) · **Stand:** CRUD und eine sichere Migrations-Teilmenge auf SQLite. PostgreSQL nutzt denselben Plan und ist nicht gegen einen lokalen Server gelaufen.

| Meilenstein | Status | Ziel | Voraussetzung |
|---|---|---|---|
| v0.1 | Code vorhanden; SQLite ist der lokale Standard und wird im Workspace getestet | Typed Model, Liste, GET, POST, PATCH, DELETE, sichere Migrationen | Optionaler PostgreSQL-Lauf nur mit erreichbarem Server |
| v0.6 | Implementiert und mit SQLite-Tests geprüft | Unique, zusammengesetzte Unique Constraints, Indizes, Foreign Keys, `restrict`/`cascade`/`set_null`, Belongs-To, Has-Many, Snapshot und Diff | Cargo-Version bleibt `0.1.0`. Many-to-Many, Eager Loading, `down` und PostgreSQL-Baseline fehlen |
| v0.7 | Implementiert und mit SQLite-Tests geprüft | Argon2id, serverseitige Sitzungen, Cookies, CSRF, RBAC, Modellrechte, blockierte Objektlisten | Cargo-Version bleibt `0.1.0`. OAuth, MFA, JWT und Passwort-Reset fehlen |
| v0.8 | Implementiert und mit SQLite- sowie Vitest-Tests geprüft | Admin-Metadaten, `/admin/api`, React-Oberfläche, CSRF-Bootstrap, Staff-Zugang | Cargo-Version bleibt `0.1.0`. Kein SQL-Scope für Objektregeln, keine Superuser-Vergabe im Admin |

## v0.1 enthalten

- Cargo-Workspace mit `ferrum-migrations` und dem Beispiel myshop.
- `FerrumModel`-Derive für `Uuid`, `String`, `i64`, `bool` und `Option` dieser Typen, außer nullable `id`.
- Typisierte Repository-Methoden `list`, `get`, `create`, `update`, `delete`.
- Versionierte Migrationen für neue Tabellen, sichere Spalten, explizite Umbenennungen, geprüfte `string`/`i64`- und Nullbarkeitsänderungen, SQLite-Baseline und ausdrücklich freigegebene Löschungen. Bootstrap ist Legacy.
- GET/POST/PATCH/DELETE. Listen haben einen `truncated`-Indikator, maximal 100 Elemente. Update-DTOs nutzen `PatchField`.
- Localhost-Demo mit SQLite-Datei. Docker Compose für PostgreSQL ist optional. Der PostgreSQL-HTTP-Test bleibt ignoriert.

## Nicht enthalten

`migrate down`, Baseline für PostgreSQL, Many-to-Many, Eager Loading, Auth, RBAC, Admin, OpenAPI, weitere CLI-Scaffolding-Kommandos, Pagination über Limit 100, API-Dockerfile und MySQL. Einspaltige Foreign Keys, Unique Constraints, Indizes und Lazy-Loading-Beziehungen sind enthalten. Eine Foreign-Key-Änderung an einer bestehenden Spalte bleibt blockiert.

## Reihenfolge für zukünftige Arbeit

1. **v0.1 verifizieren**: Rustfmt, Clippy, Tests, optional DB-HTTP-Test; Fehler zuerst beheben.
2. **Metadaten erweitern**: Felder, Typen, Constraints, Snapshot-Version.
3. **Migrationen erweitern**: `down`, Sperren und eine PostgreSQL-Baseline. Explizite Umbenennung, der geprüfte SQLite-Umbau und die SQLite-Baseline sind vorhanden.
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
