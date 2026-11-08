# Architecture – module boundaries and runtime

[Overview](index.md) · **Implemented:** apps, named routes and views share the `App` builder in `ferrioxid-api`. Settings and migration diagnostics live in `ferrioxid-migrations`; diagnostics are read-only and use the existing migration history. Model date/time fields use the shared metadata, migration engine and repository. Model, directory and audit writes share `DatabaseTransaction`; see [transactions.md](transactions.md). Admin extensions use the existing `AdminRegistry`. Mail delivery is in `ferrioxid-mail`, password reset in `ferrioxid-auth`, and the persistent job queue in `ferrioxid-jobs`. Application workers use `WorkerHost`; the web server does not start a worker.

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
10. **One settings file:** `ferrum.toml` configures the project. Tera renders HTML pages outside the admin.

## 2. Actual Workspace (Implemented)

```text
examples/myshop → ferrioxid (Fassade)
                    ├── ferrioxid-core
                    ├── ferrioxid-macros
                    ├── ferrioxid-db → ferrioxid-core, SeaORM
                    ├── ferrioxid-auth → ferrioxid-core, ferrioxid-db, ferrioxid-migrations
                    ├── ferrioxid-admin → ferrioxid-auth, ferrioxid-core, ferrioxid-db
                    ├── ferrioxid-api → ferrioxid-admin, ferrioxid-auth, ferrioxid-core, ferrioxid-db, ferrioxid-static, ferrioxid-middleware, Axum
                    ├── ferrioxid-forms → ferrioxid-core
                    ├── ferrioxid-static → Axum
                    ├── ferrioxid-storage → ferrioxid-auth, ferrioxid-core
                    └── ferrioxid-middleware → Axum, tracing
ferrioxid-cli → ferrioxid-auth, ferrioxid-db, ferrioxid-migrations
ferrioxid-admin → Tera-HTML und CSS unter `/admin`, JSON unter `/admin/api`
```

Physical crates include `crates/ferrioxid/`, `crates/ferrioxid-core/`, `crates/ferrioxid-macros/`, `crates/ferrioxid-db/`, `crates/ferrioxid-migrations/`, `crates/ferrioxid-auth/`, `crates/ferrioxid-api/`, `crates/ferrioxid-cli/`, `crates/ferrioxid-forms/`, `crates/ferrioxid-static/`, `crates/ferrioxid-storage/` and `crates/ferrioxid-middleware/`. `ferrioxid-forms` does not depend on Axum. Neither `ferrioxid-storage` nor `ferrioxid-auth` depends on `ferrioxid-api`. The workspace uses edition 2021. Package versions and enabled features are defined in `Cargo.toml`.

## 3. Compile-time processing (Implemented)

```text
Product Rust-Struct + #[field(...)] + #[ferrioxid(...)]
  → syn (DeriveInput, Attribute, Named Fields)
  → Prüfung (zulässige Typen, Primärschlüssel, SQL-Identifier, API-Pfad)
  → quote (ProductCreate, FerrioxidModel impl, ModelRepository impl)
  → Rust-Compiler prüft generierten Code
```

Macro implementation: `crates/ferrioxid-macros/src/lib.rs`. The model struct must also derive `Serialize` and `Deserialize`; the model derive macro does not generate those derives for the model itself.

## 4. Start and request data flow (Implemented)

```text
myshop::main
  ├─ DATABASE_URL oder sqlite://db.sqlite3?mode=rwc → ferrioxid::db::connect
  ├─ AuthService::new + App::with_auth
  ├─ App::new(db.clone()).register::<Product>()
  │    ├─ Routen GET, GET /{id}, POST, PATCH, DELETE oder denied
  │    └─ ColumnSpec für den Bootstrap merken
  ├─ --bootstrap? → Abbruch ohne Schemaänderung
  ├─ --print-schema? → Snapshot-JSON der registrierten Modelle
  └─ BIND_ADDRESS lesen → TcpListener → axum::serve

ferrioxid migrate make|up|status|rename-column|rename-table|reseal|baseline
  → ferrum.toml oder --schema-file
  → zuerst Auth-Verzeichnis und Historie `_ferrum_auth_migrations`
  → danach Snapshot, Diff, Datei, Historie `_ferrum_migrations`

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
  → Product::list
  → SELECT ... ORDER BY "id" LIMIT 101
  → höchstens 100 Modelle, truncated als Boolean
```

Route registration reads `allow_anonymous` and the configured permission policy. Models with neither public access nor an authorization policy receive only 403 handlers.

v0.24 mounts named application routes on the same router through `App::urls`. `UrlSet` rejects `/admin`, `/auth` and the reserved auth paths. Settings and `RouteMap` leave `into_router` as `Extension` values rather than new `AppState` fields. `merge` remains available for unnamed routers.

## 5. Contracts between modules (Implemented)

| Producer | Consumer | Contract |File|
|---|---|---|---|
| `ferrioxid-macros` | `ferrioxid-core` | `FerrioxidModel::descriptor/columns/constraints/schema_sql(DatabaseKind)` | `crates/ferrioxid-core/src/lib.rs` |
| `ferrioxid-macros` | `ferrioxid-db` |`ModelRepository` and SeaQuery-CRUD| `crates/ferrioxid-db/src/lib.rs`, `crates/ferrioxid-db/src/sql.rs` |
| `ferrioxid-api` | `ferrioxid-db` | Generic `M: ModelRepository` bound | `crates/ferrioxid-api/src/lib.rs` |
| `myshop` | `ferrioxid` | `prelude`, `db::connect`, `App` | `crates/ferrioxid/src/lib.rs` |

## 6. Architecture for later modules (planned, not implemented)

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
- API-JSON rejections of Axum do not use the Ferrioxid fault body.
- The HTTP list query limits the response to a maximum of 100 records; No pagination.
- `CREATE TABLE IF NOT EXISTS` does not report schema drift.
- OpenAPI and a general plugin system are not implemented. Auth, migrations and the native admin are available.

## 8. Admin writes in v0.18

`ModelRepository` exposes `get_tx`, `create_tx`, `update_tx` and `delete_tx` on `&DatabaseTransaction`. SQL helpers accept `&impl ConnectionTrait`. `ManagedModel::update` and `remove` receive the `AuthUser`. Admin creates, updates and deletes write the record and `_ferrum_admin_audit` in one transaction; a failure rolls back both. Direct repository calls do not write admin audit entries.

Since v0.20, directory events (`user_create`, `user_update`, `password_reset`, `role_assign`, `role_revoke`) share a transaction with the corresponding auth changes. Self-service password reset at `/auth/password-reset` atomically consumes the token, changes the password, revokes sessions and writes the audit entry. Email delivery follows the commit. See [password-reset.md](password-reset.md).

Drafts are in `_ferrum_admin_drafts` and refer as text UUID to `_ferrum_sessions.id`. Auth stores session IDs as text. The same text binding applies to audit IDs. The migration is `crates/ferrioxid-auth/migrations/0002_admin_journal.json`. `0001_auth_schema` remains unchanged.

Further reading: [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [security.md](security.md), [admin-audit-log.md](admin-audit-log.md).
