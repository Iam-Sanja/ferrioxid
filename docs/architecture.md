# Architecture – Module Boundaries and Duration

[For overview](index.md) · **Status:** Module boundaries remain valid.] v0.27.1 launches application handler via `WorkerHost` on the same queue. v0.27 places the queue in `ferrioxid-jobs` on the same SeaORM connection. The web server does not start the worker. v0.26 checks this registry before `into_router` if apps are installed or declared in `ferrum.toml`. Field groups remain the same `FerrioxidModel` structure. v0.25 sets the app registry to the same `App` in `ferrioxid-api`. App migrations remain in `_ferrum_migrations`. v0.24 sets settings in `ferrioxid-migrations` and named routes and view helpers in `ferrioxid-api`. `AppState` remains unchanged. There is no second router and no settings singleton. v0.23 places the shipment in `ferrioxid-mail` and the reset in `ferrioxid-auth`. There is no second password or session service. v0.22 hangs pages, actions, filters, widgets and templates to the same `AdminRegistry`. There is no second admin engine. v0.21 attaches the reading migration diagnosis and the upgrade manifest to the same migration engine. There is no second history and no writing diagnostic path. v0.19 adds `Date`, `Time` and `DateTime<Utc>` to the same metadata, migration engine and repository. v0.20 sets model, authentic and audit write operations to the same `DatabaseTransaction`. There is no second transaction engine. See [transactions.md](transactions.md)].

## 1. Architecture principles

1. **Rust is the model source.** No parallel YAML model scheme.
2. **Static generation:** Macros generate implementations; not a dynamic Python-like reflection system.
3. **Explicit model registration:** No automatic scanning of all Crates.
4. **Deny by default:** Without demo release, the generated route 403 answers.
5. **Types at module boundaries:** `ModelRepository::create` receives a Create type and delivers a model; `list` provides `ListPage<Model>`.
6. **parameterize database accesses:** SQL identifiers come only from checked macro attributes. SeaQuery chooses the dialect.
7. **A repository implementation:** Traders and models do not distinguish SQLite and PostgreSQL.
8. **Schema changes run through migrations. *** `myshop --bootstrap` is legacy and does not write tables or history.
9. **Create policies apply before the INSERT.** REST and Admin call the same `can_create` rule. The standard is rejection.
10. **A settings file.** `ferrum.toml` is the project configuration. HTML pages outside the admin renders Tera.

## 2. Actual Workspace (IST)

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

Physische Crates: `crates/ferrioxid/`, `crates/ferrioxid-core/`, `crates/ferrioxid-macros/`, `crates/ferrioxid-db/`, `crates/ferrioxid-migrations/`, `crates/ferrioxid-auth/`, `crates/ferrioxid-api/`, `crates/ferrioxid-cli/`, `crates/ferrioxid-forms/`, `crates/ferrioxid-static/`, `crates/ferrioxid-storage/`, `crates/ferrioxid-middleware/`. `ferrioxid-forms` hängt nicht von Axum ab. `ferrioxid-storage` hängt nicht von `ferrioxid-api` ab. `ferrioxid-auth` hängt nicht von `ferrioxid-api` ab. `Cargo.toml` legt Workspace Edition 2021 fest. Versionsangaben und Features stehen dort, nicht in dieser Dokumentation.

## 3. Compile-time processing (IST)

```text
Product Rust-Struct + #[field(...)] + #[ferrioxid(...)]
  → syn (DeriveInput, Attribute, Named Fields)
  → Prüfung (zulässige Typen, Primärschlüssel, SQL-Identifier, API-Pfad)
  → quote (ProductCreate, FerrioxidModel impl, ModelRepository impl)
  → Rust-Compiler prüft generierten Code
```

Macrocode: `crates/ferrioxid-macros/src/lib.rs`. The model structure must additionally derive `Serialize` + `Deserialize`. The Derive macro creates the necessary Serde Derives for the original model type **not** itself.

## 4. Start and request data flow (IST)

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

The check `allow_anonymous` takes place **when registering the route**. Without release, only 403 traders are registered.

v0.24 hängt benannte Anwendungsrouten über `App::urls` an denselben Router. `UrlSet` lehnt `/admin`, `/auth` und die festen Auth-Pfade ab. Settings und `RouteMap` verlassen `into_router` als `Extension`, nicht als neue Felder von `AppState`. `merge` bleibt für unbenannte Router gültig.

## 5. Contracts between modules (IST)

| Producer | Consumer | Vertrag |File|
|---|---|---|---|
| `ferrioxid-macros` | `ferrioxid-core` | `FerrioxidModel::descriptor/columns/constraints/schema_sql(DatabaseKind)` | `crates/ferrioxid-core/src/lib.rs` |
| `ferrioxid-macros` | `ferrioxid-db` |`ModelRepository` and SeaQuery-CRUD| `crates/ferrioxid-db/src/lib.rs`, `crates/ferrioxid-db/src/sql.rs` |
| `ferrioxid-api` | `ferrioxid-db` | generische `M: ModelRepository` | `crates/ferrioxid-api/src/lib.rs` |
| `myshop` | `ferrioxid` | `prelude`, `db::connect`, `App` | `crates/ferrioxid/src/lib.rs` |

## 6. Architecture for later modules (reference, not implemented)

```text
Rust-Modelle + Makros
 → reichhaltige Model Registry (Fields, Relations, Constraints, Policies)
 → DB / Migrations / Validierung / Auth
 → REST + OpenAPI + Admin-Metadaten
 → serverseitiger Admin (seit v0.13 vorhanden)
 → CLI / Docker-Deployment / Plugins
```

`ModelDescriptor` still does not contain a field catalog. Columns, unique groups, index groups and foreign keys are in `ColumnSpec` and `TableConstraints`. This results in snapshot, SQL and the lazy loading methods. Policies, eager loading and an automatic discovery of all rust structures are not implemented. Schema snapshots remain serializable and deterministic. A runtime registry only has explicitly registered models.

## 7. Security borders and open risks

- `allow_anonymous=true` is intended exclusively for the locally bound demo.
- `BIND_ADDRESS` can make the service available beyond localhost.
- API-JSON rejections of Axum do not use the ferrioxide fault body.
- The HTTP list query limits the response to a maximum of 100 records; No pagination.
- `CREATE TABLE IF NOT EXISTS` does not report schema drift.
- OpenAPI and a general plugin system are not implemented. Auth, migrations and the native admin are available.

## 8. Admin writing in v0.18

`ModelRepository` has `get_tx`, `create_tx`, `update_tx` and `delete_tx` on `&DatabaseTransaction`. The SQL helpers take `&impl ConnectionTrait`. `ManagedModel::update` and `remove` get the `AuthUser`. A successful creation, modification or deletion in the admin opens a transaction, writes the line, writes `_ferrum_admin_audit` and commits. An abandonment rejects both. `ModelRepository::create` without the admin path does not write a history.

Directory events (`user_create`, `user_update`, `password_reset`, `role_assign`, `role_revoke`) occur after the `AuthService` call. You do not share a transaction with the Auth Change. The self-service under `/auth/password-reset` is separate from this: token consumption, password, session revocation and audit are one transaction. The email follows after the commit. See [password-reset.md](password-reset.md)].

Drafts are in `_ferrum_admin_drafts` and refer as text UUID to `_ferrum_sessions.id`. Auth stores session IDs as text. The same text binding applies to audit IDs. The migration is `crates/ferrioxid-auth/migrations/0002_admin_journal.json`. `0001_auth_schema` remains unchanged.

Weiterlesen: [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [security.md](security.md), [admin-audit-log.md](admin-audit-log.md).
