# Architecture – module boundaries and runtime

[Overview](index.md) · **Implemented:** apps, named routes and views share the `App` builder in `fexu-api`. Settings and migration diagnostics live in `fexu-migrations`; diagnostics are read-only and use the existing migration history. Model date/time fields use the shared metadata, migration engine and repository. Model, directory and audit writes share `DatabaseTransaction`; see [transactions.md](transactions.md). Admin extensions use the existing `AdminRegistry`. Mail delivery is in `fexu-mail`, password reset in `fexu-auth`, and the persistent job queue in `fexu-jobs`. Application workers use `WorkerHost`; the web server does not start a worker.

## 1. Architecture principles

1. **Rust is the model source.** No parallel YAML model schema.
2. **Static generation:** Macros generate implementations; not a dynamic Python-like reflection system.
3. **Explicit model registration:** No automatic scanning of all Crates.
4. **Deny by default:** generated routes return 403 unless anonymous access or a supported authorization policy is configured.
5. **Types at module boundaries:** `ModelRepository::create` receives a Create type and delivers a model; `list` provides `ListPage<Model>`.
6. **Parameterized database access:** SQL identifiers come only from validated macro attributes. SeaQuery selects the dialect.
7. **One repository implementation:** handlers and models do not distinguish between SQLite and PostgreSQL.
8. **Schema changes run through migrations.** `myshop --bootstrap` is legacy and does not write tables or history.
9. **Create policies run before INSERT.** REST and admin call the same `can_create` rule, which denies access by default.
10. **One settings file:** `fexu.toml` configures the project. Tera renders HTML pages outside the admin.

## 2. Actual Workspace (Implemented)

```text
examples/myshop → fexu (Fassade)
                    ├── fexu-core
                    ├── fexu-macros
                    ├── fexu-db → fexu-core, SeaQuery, rusqlite (SQLite), sqlx-postgres/sqlx-core (PostgreSQL)
                    ├── fexu-auth → fexu-core, fexu-db, fexu-migrations
                    ├── fexu-admin → fexu-auth, fexu-core, fexu-db
                    ├── fexu-api → fexu-admin, fexu-auth, fexu-core, fexu-db, fexu-static, fexu-middleware, Axum
                    ├── fexu-forms → fexu-core
                    ├── fexu-static → Axum
                    ├── fexu-storage → fexu-auth, fexu-core
                    └── fexu-middleware → Axum, tracing
fexu-cli → fexu-auth, fexu-db, fexu-migrations
fexu-admin → Tera-HTML und CSS unter `/admin`, JSON unter `/admin/api`
```

Physical crates include `crates/fexu/`, `crates/fexu-core/`, `crates/fexu-macros/`, `crates/fexu-db/`, `crates/fexu-migrations/`, `crates/fexu-auth/`, `crates/fexu-api/`, `crates/fexu-cli/`, `crates/fexu-forms/`, `crates/fexu-static/`, `crates/fexu-storage/` and `crates/fexu-middleware/`. `fexu-forms` does not depend on Axum. Neither `fexu-storage` nor `fexu-auth` depends on `fexu-api`. The workspace uses edition 2021. Package versions and enabled features are defined in `Cargo.toml`.

## 3. Compile-time processing (Implemented)

```text
Product Rust-Struct + #[field(...)] + #[fexu(...)]
  → syn (DeriveInput, Attribute, Named Fields)
  → Prüfung (zulässige Typen, Primärschlüssel, SQL-Identifier, API-Pfad)
  → quote (ProductCreate, FexuModel impl, ModelRepository impl)
  → Rust-Compiler prüft generierten Code
```

Macro implementation: `crates/fexu-macros/src/lib.rs`. The model struct must also derive `Serialize` and `Deserialize`; the model derive macro does not generate those derives for the model itself.

## 4. Start and request data flow (Implemented)

```text
myshop::main
  ├─ DATABASE_URL oder sqlite://db.sqlite3?mode=rwc → fexu::db::connect
  ├─ AuthService::new + App::with_auth
  ├─ App::new(db.clone()).register::<Product>()
  │    ├─ Routen GET, GET /{id}, POST, PATCH, DELETE oder denied
  │    └─ ColumnSpec für den Bootstrap merken
  ├─ --bootstrap? → Abbruch ohne Schemaänderung
  ├─ --print-schema? → Snapshot-JSON der registrierten Modelle
  └─ BIND_ADDRESS lesen → TcpListener → axum::serve

fexu migrate make|up|status|rename-column|rename-table|reseal|baseline
  → fexu.toml oder --schema-file
  → zuerst Auth-Verzeichnis und Historie `_fexu_auth_migrations`
  → danach Snapshot, Diff, Datei, Historie `_fexu_migrations`

POST /api/v1/products
  → Axum Json<ProductCreate>
  → create::<Product>
  → Product::create
  → Validierung
  → SeaQuery INSERT ... RETURNING, Dialekt aus der Verbindung
  → interne Deserialisierung zum Model
  → HTTP 201 + Json<Product>

PATCH /api/v1/products/{id}
  → Path<Uuid> + Json<ProductUpdate>
  → leeres Patch: SELECT; sonst UPDATE nur geänderter Spalten WHERE "id"
  → HTTP 200, 404 oder 422

DELETE /api/v1/products/{id}
  → DELETE WHERE "id" mit gebundenem Wert
  → eine Zeile: HTTP 204 ohne Körper
  → keine Zeile: HTTP 404

GET /api/v1/products
  → Product::query Keyset
  → SELECT ... ORDER BY "id" LIMIT (limit + 1)
  → JSON data, limit, next, has_more
```

Route registration reads `allow_anonymous` and the configured permission policy. Models with neither public access nor an authorization policy receive only 403 handlers.

v0.24 mounts named application routes on the same router through `App::urls`. `UrlSet` rejects `/admin`, `/auth` and the reserved auth paths. Settings and `RouteMap` leave `into_router` as `Extension` values rather than new `AppState` fields. `merge` remains available for unnamed routers.

## 5. Contracts between modules (Implemented)

| Producer | Consumer | Contract |File|
|---|---|---|---|
| `fexu-macros` | `fexu-core` | `FexuModel::descriptor/columns/constraints/schema_sql(DatabaseKind)` | `crates/fexu-core/src/lib.rs` |
| `fexu-macros` | `fexu-db` |`ModelRepository` and SeaQuery-CRUD| `crates/fexu-db/src/lib.rs`, `crates/fexu-db/src/sql.rs` |
| `fexu-api` | `fexu-db` | Generic `M: ModelRepository` bound | `crates/fexu-api/src/lib.rs` |
| `myshop` | `fexu` | `prelude`, `db::connect`, `App` | `crates/fexu/src/lib.rs` |

## 6. Historical architecture proposals (check actual implementation below)

```text
Rust-Modelle + Makros
 → reichhaltige Model Registry (Fields, Relations, Constraints, Policies)
 → DB / Migrations / Validierung / Auth
 → REST + OpenAPI + Admin-Metadaten
 → serverseitiger Admin (seit v0.13 vorhanden)
 → CLI / Docker-Deployment / Plugins
```

`ModelDescriptor` does not contain a field catalog. `ColumnSpec` and `TableConstraints` provide columns, unique groups, index groups and foreign keys for snapshots, SQL generation and lazy relationship methods. Object policies are registered separately. Eager loading and automatic discovery of Rust structs are not implemented. Schema snapshots are serializable and deterministic; runtime registries contain only explicitly registered models.

## 7. Security borders and open risks

- `allow_anonymous=true` is intended exclusively for the locally bound demo.
- `BIND_ADDRESS` can make the service available beyond localhost.
- API-JSON rejections of Axum do not use the Fexu fault body.
- The HTTP list query limits the response to a maximum of 100 records; No pagination.
- `CREATE TABLE IF NOT EXISTS` does not report schema drift.
- OpenAPI 3.1 is generated from registered model metadata. A general plugin system is not implemented. Auth, migrations and the native admin are available.

## 8. Admin writes (implemented; historical milestone v0.18)

`ModelRepository` exposes `get_tx`, `create_tx`, `update_tx` and `delete_tx` on `&DatabaseTransaction`. SQL helpers accept `&impl ConnectionTrait`. `ManagedModel::update` and `remove` receive the `AuthUser`. Admin creates, updates and deletes write the record and `_fexu_admin_audit` in one transaction; a failure rolls back both. Direct repository calls do not write admin audit entries.

Since v0.20, directory events (`user_create`, `user_update`, `password_reset`, `role_assign`, `role_revoke`) share a transaction with the corresponding auth changes. Self-service password reset at `/auth/password-reset` atomically consumes the token, changes the password, revokes sessions and writes the audit entry. Email delivery follows the commit. See [password-reset.md](password-reset.md).

Drafts are in `_fexu_admin_drafts` and refer as text UUID to `_fexu_sessions.id`. Auth stores session IDs as text. The same text binding applies to audit IDs. The migration is `crates/fexu-auth/migrations/0002_admin_journal.json`. `0001_auth_schema` remains unchanged.

Further reading: [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [security.md](security.md), [admin-audit-log.md](admin-audit-log.md).
