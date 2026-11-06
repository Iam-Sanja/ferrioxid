# Architektur – Modulgrenzen und Laufzeit

[Zur Übersicht](index.md) · **Status:** Beschreibung v0.1 + ausdrücklich getrennte Zukunftsarchitektur.

## 1. Architekturprinzipien

1. **Rust ist die Modellquelle.** Kein paralleles YAML-Modellschema.
2. **Statische Generierung:** Makros generieren Implementierungen; kein dynamisches Python-artiges Reflection-System.
3. **Explizite Modellregistrierung:** Kein automatisches Scannen aller Crates.
4. **Deny by default:** Ohne Demo-Freigabe antwortet die generierte Route 403.
5. **Typen an Modulgrenzen:** `ModelRepository::create` erhält einen Create-Typ und liefert ein Model; `list` liefert `ListPage<Model>`.
6. **Datenbankzugriffe parameterisieren:** SQL-Identifier stammen nur aus geprüften Makroattributen. SeaQuery wählt den Dialekt.
7. **Eine Repository-Implementierung:** Handler und Modelle unterscheiden SQLite und PostgreSQL nicht.
8. **Schemaänderungen laufen über Migrationen.** `myshop --bootstrap` ist Legacy und schreibt weder Tabellen noch Historie.

## 2. Tatsächlicher Workspace (IST)

```text
examples/myshop → ferrum (Fassade)
                    ├── ferrum-core
                    ├── ferrum-macros
                    ├── ferrum-db → ferrum-core, SeaORM
                    ├── ferrum-auth → ferrum-core, ferrum-db, ferrum-migrations
                    ├── ferrum-admin → ferrum-auth, ferrum-core, ferrum-db
                    └── ferrum-api → ferrum-admin, ferrum-auth, ferrum-core, ferrum-db, Axum
ferrum-cli → ferrum-auth, ferrum-db, ferrum-migrations
apps/admin-ui → gebautes Verzeichnis, vom Server unter `/admin` ausgeliefert
```

Physische Crates: `crates/ferrum/`, `crates/ferrum-core/`, `crates/ferrum-macros/`, `crates/ferrum-db/`, `crates/ferrum-migrations/`, `crates/ferrum-auth/`, `crates/ferrum-api/`, `crates/ferrum-cli/`. `ferrum-auth` hängt nicht von `ferrum-api` ab. `Cargo.toml` legt Workspace Edition 2021 fest. Versionsangaben und Features stehen dort, nicht in dieser Dokumentation.

## 3. Compile-time-Verarbeitung (IST)

```text
Product Rust-Struct + #[field(...)] + #[ferrum(...)]
  → syn (DeriveInput, Attribute, Named Fields)
  → Prüfung (zulässige Typen, Primärschlüssel, SQL-Identifier, API-Pfad)
  → quote (ProductCreate, FerrumModel impl, ModelRepository impl)
  → Rust-Compiler prüft generierten Code
```

Makrocode: `crates/ferrum-macros/src/lib.rs`. Die Modell-Struct muss zusätzlich `Serialize` + `Deserialize` ableiten. Das Derive-Makro erstellt die dafür nötigen Serde-Derives für den ursprünglichen Model-Typ **nicht** selbst.

## 4. Start- und Request-Datenfluss (IST)

```text
myshop::main
  ├─ DATABASE_URL oder sqlite://db.sqlite3?mode=rwc → ferrum::db::connect
  ├─ AuthService::new + App::with_auth
  ├─ App::new(db.clone()).register::<Product>()
  │    ├─ Routen GET, GET /{id}, POST, PATCH, DELETE oder denied
  │    └─ ColumnSpec für den Bootstrap merken
  ├─ --bootstrap? → Abbruch ohne Schemaänderung
  ├─ --print-schema? → Snapshot-JSON der registrierten Modelle
  └─ BIND_ADDRESS lesen → TcpListener → axum::serve

ferrum migrate make|up|status|rename-column|rename-table|reseal|baseline
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

Die Prüfung `allow_anonymous` geschieht **beim Registrieren der Route**. Ohne Freigabe werden ausschließlich 403-Handler registriert. Es existieren keine Identitäten oder RBAC-Policies.

## 5. Verträge zwischen Modulen (IST)

| Producer | Consumer | Vertrag | Datei |
|---|---|---|---|
| `ferrum-macros` | `ferrum-core` | `FerrumModel::descriptor/columns/constraints/schema_sql(DatabaseKind)` | `crates/ferrum-core/src/lib.rs` |
| `ferrum-macros` | `ferrum-db` | `ModelRepository` und SeaQuery-CRUD | `crates/ferrum-db/src/lib.rs`, `crates/ferrum-db/src/sql.rs` |
| `ferrum-api` | `ferrum-db` | generische `M: ModelRepository` | `crates/ferrum-api/src/lib.rs` |
| `myshop` | `ferrum` | `prelude`, `db::connect`, `App` | `crates/ferrum/src/lib.rs` |

## 6. Architektur für spätere Module (SOLL, nicht implementiert)

```text
Rust-Modelle + Makros
 → reichhaltige Model Registry (Fields, Relations, Constraints, Policies)
 → DB / Migrations / Validierung / Auth
 → REST + OpenAPI + Admin-Metadaten
 → React Admin (schema-driven)
 → CLI / Docker-Deployment / Plugins
```

`ModelDescriptor` enthält weiterhin keinen Feldkatalog. Spalten, Unique-Gruppen, Index-Gruppen und Foreign Keys stehen in `ColumnSpec` und `TableConstraints`. Daraus entstehen Snapshot, SQL und die Lazy-Loading-Methoden. Policies, Eager Loading und eine automatische Entdeckung aller Rust-Structs sind nicht implementiert. Schema-Snapshots bleiben serialisierbar und deterministisch. Eine Laufzeit-Registry führt nur explizit registrierte Modelle.

## 7. Sicherheitsgrenzen und offene Risiken

- `allow_anonymous=true` ist ausschließlich für das lokal gebundene Demo gedacht.
- `BIND_ADDRESS` kann den Dienst über localhost hinaus verfügbar machen.
- API-JSON-Rejections von Axum verwenden nicht den Ferrum-Fehlerkörper.
- Die HTTP-Listenabfrage beschränkt die Antwort auf maximal 100 Datensätze; keine Pagination.
- `CREATE TABLE IF NOT EXISTS` meldet keinen Schema-Drift.
- Keine Authentication, Migration Engine, Admin-UI, OpenAPI, sichere Produktionskonfiguration.

Weiterlesen: [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [security.md](security.md).
