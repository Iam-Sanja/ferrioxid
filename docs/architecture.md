# Architektur – Modulgrenzen und Laufzeit

[Zur Übersicht](index.md) · **Status:** Beschreibung v0.1 + ausdrücklich getrennte Zukunftsarchitektur.

## 1. Architekturprinzipien

1. **Rust ist die Modellquelle.** Kein paralleles YAML-Modellschema.
2. **Statische Generierung:** Makros generieren Implementierungen; kein dynamisches Python-artiges Reflection-System.
3. **Explizite Modellregistrierung:** Kein automatisches Scannen aller Crates.
4. **Deny by default:** Ohne Demo-Freigabe antwortet die generierte Route 403.
5. **Typen an Modulgrenzen:** `ModelRepository::create` erhält einen Create-Typ und liefert ein Model; `list` liefert `ListPage<Model>`.
6. **Datenbankzugriffe parameterisieren:** SQL-Identifier stammen nur aus geprüften Makroattributen.
7. **Bootstrap ist keine Migration.** Produktschemaänderungen dürfen nicht still per `CREATE TABLE IF NOT EXISTS` simuliert werden.

## 2. Tatsächlicher Workspace (IST)

```text
examples/myshop → ferrum (Fassade)
                    ├── ferrum-core
                    ├── ferrum-macros
                    ├── ferrum-db → ferrum-core, SeaORM
                    └── ferrum-api → ferrum-core, ferrum-db, Axum, SeaORM
ferrum-cli (separate Binary)
```

Physische Crates: `crates/ferrum/`, `crates/ferrum-core/`, `crates/ferrum-macros/`, `crates/ferrum-db/`, `crates/ferrum-api/`, `crates/ferrum-cli/`. `Cargo.toml` legt Workspace Edition 2021 fest. Versionsangaben und Features stehen dort, nicht in dieser Dokumentation.

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
  ├─ DATABASE_URL lesen → ferrum::db::connect
  ├─ App::new(db.clone()).register::<Product>()
  │    ├─ Register Route GET/POST (oder denied)
  │    └─ schema_sql() in Bootstrap-Liste
  ├─ --bootstrap? → CREATE TABLE IF NOT EXISTS → exit
  └─ BIND_ADDRESS lesen → TcpListener → axum::serve

POST /api/v1/products
  → Axum Json<ProductCreate> (Axum-Rejection bei ungültigem JSON)
  → Ferrum-Handler create::<Product>
  → Product::create(db, ProductCreate)
  → Validierung der Makro-generierten Regeln
  → Parametrisiertes INSERT ... RETURNING to_jsonb(...)
  → serde_json::from_value<Product>
  → HTTP 201 + Json<Product>

PATCH /api/v1/products/{id}
  → Path<Uuid> + Json<ProductUpdate>
  → Product::update mit einem UPDATE ... WHERE "id" = $n
  → HTTP 200 + Json<Product>, 404 wenn keine Zeile, 422 bei PatchField::Null auf NOT NULL

DELETE /api/v1/products/{id}
  → DELETE FROM "products" WHERE "id" = $1
  → rows_affected 1: HTTP 204 ohne Körper
  → rows_affected 0: HTTP 404

GET /api/v1/products
  → Product::list(db)
  → SELECT ... ORDER BY id LIMIT 101
  → deserialize records into Product
  → return {data:[max 100],limit:100,truncated:boolean}
```

Die Prüfung `allow_anonymous` geschieht **beim Registrieren der Route**. Ohne Freigabe werden ausschließlich 403-Handler registriert. Es existieren keine Identitäten oder RBAC-Policies.

## 5. Verträge zwischen Modulen (IST)

| Producer | Consumer | Vertrag | Datei |
|---|---|---|---|
| `ferrum-macros` | `ferrum-core` | `FerrumModel::descriptor/schema_sql` | `crates/ferrum-core/src/lib.rs` |
| `ferrum-macros` | `ferrum-db` | `ModelRepository::Create/list/create` | `crates/ferrum-db/src/lib.rs` |
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

**Wichtiger Entwurfspunkt:** Der aktuelle `ModelDescriptor` enthält **keinen** Feldkatalog. Für automatische Migrationen und Admin müssen Metadaten erweitert werden, ohne vorhandene Model-Implementierungen still zu brechen. Schema-Snapshots müssen serialisierbar und deterministisch sein. Eine Laufzeit-Registry kann explizit registrierte Deskriptoren führen, aber nicht ungefragt alle Rust-Structs entdecken.

## 7. Sicherheitsgrenzen und offene Risiken

- `allow_anonymous=true` ist ausschließlich für das lokal gebundene Demo gedacht.
- `BIND_ADDRESS` kann den Dienst über localhost hinaus verfügbar machen.
- API-JSON-Rejections von Axum verwenden nicht den Ferrum-Fehlerkörper.
- Die HTTP-Listenabfrage beschränkt die Antwort auf maximal 100 Datensätze; keine Pagination.
- `CREATE TABLE IF NOT EXISTS` meldet keinen Schema-Drift.
- Keine Authentication, Migration Engine, Admin-UI, OpenAPI, sichere Produktionskonfiguration.

Weiterlesen: [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [security.md](security.md).
