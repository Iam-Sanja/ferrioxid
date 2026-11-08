# Architektur – Modulgrenzen und Laufzeit

[Zur Übersicht](index.md) · **Status:** Modulgrenzen bleiben gültig. v0.27.1 startet Anwendungs-Handler über `WorkerHost` auf derselben Queue. v0.27 legt die Queue in `ferrioxid-jobs` auf dieselbe SeaORM-Verbindung. Der Webserver startet den Worker nicht. v0.26 prüft diese Registry vor `into_router`, wenn Apps installiert oder in `ferrum.toml` deklariert sind. Feldgruppen bleiben dieselbe `FerrioxidModel`-Struct. v0.25 legt die App-Registry auf dieselbe `App` in `ferrioxid-api`. App-Migrationen bleiben in `_ferrum_migrations`. v0.24 legt Settings in `ferrioxid-migrations` und benannte Routen sowie View-Helfer in `ferrioxid-api`. `AppState` bleibt unverändert. Es gibt keinen zweiten Router und kein Settings-Singleton. v0.23 legt den Versand in `ferrioxid-mail` und den Reset in `ferrioxid-auth`. Es gibt keinen zweiten Passwort- oder Sitzungsdienst. v0.22 hängt Seiten, Aktionen, Filter, Widgets und Vorlagen an dieselbe `AdminRegistry`. Es gibt keine zweite Admin-Engine. v0.21 hängt die lesende Migrationsdiagnose und das Upgrade-Manifest an dieselbe Migrations-Engine. Es gibt keine zweite Historie und keinen schreibenden Diagnoseweg. v0.19 ergänzt `Date`, `Time` und `DateTime<Utc>` in denselben Metadaten, derselben Migrations-Engine und demselben Repository. v0.20 legt Modell-, Auth- und Audit-Schreibvorgänge auf dieselbe `DatabaseTransaction`. Es gibt keine zweite Transaktions-Engine. Siehe [transactions.md](transactions.md).

## 1. Architekturprinzipien

1. **Rust ist die Modellquelle.** Kein paralleles YAML-Modellschema.
2. **Statische Generierung:** Makros generieren Implementierungen; kein dynamisches Python-artiges Reflection-System.
3. **Explizite Modellregistrierung:** Kein automatisches Scannen aller Crates.
4. **Deny by default:** Ohne Demo-Freigabe antwortet die generierte Route 403.
5. **Typen an Modulgrenzen:** `ModelRepository::create` erhält einen Create-Typ und liefert ein Model; `list` liefert `ListPage<Model>`.
6. **Datenbankzugriffe parameterisieren:** SQL-Identifier stammen nur aus geprüften Makroattributen. SeaQuery wählt den Dialekt.
7. **Eine Repository-Implementierung:** Handler und Modelle unterscheiden SQLite und PostgreSQL nicht.
8. **Schemaänderungen laufen über Migrationen.** `myshop --bootstrap` ist Legacy und schreibt weder Tabellen noch Historie.
9. **Create-Policies gelten vor dem INSERT.** REST und Admin rufen dieselbe `can_create`-Regel auf. Der Standard ist Ablehnung.
10. **Eine Settings-Datei.** `ferrum.toml` ist die Projektkonfiguration. HTML-Seiten außerhalb des Admins rendert Tera.

## 2. Tatsächlicher Workspace (IST)

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

## 3. Compile-time-Verarbeitung (IST)

```text
Product Rust-Struct + #[field(...)] + #[ferrioxid(...)]
  → syn (DeriveInput, Attribute, Named Fields)
  → Prüfung (zulässige Typen, Primärschlüssel, SQL-Identifier, API-Pfad)
  → quote (ProductCreate, FerrioxidModel impl, ModelRepository impl)
  → Rust-Compiler prüft generierten Code
```

Makrocode: `crates/ferrioxid-macros/src/lib.rs`. Die Modell-Struct muss zusätzlich `Serialize` + `Deserialize` ableiten. Das Derive-Makro erstellt die dafür nötigen Serde-Derives für den ursprünglichen Model-Typ **nicht** selbst.

## 4. Start- und Request-Datenfluss (IST)

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

Die Prüfung `allow_anonymous` geschieht **beim Registrieren der Route**. Ohne Freigabe werden ausschließlich 403-Handler registriert.

v0.24 hängt benannte Anwendungsrouten über `App::urls` an denselben Router. `UrlSet` lehnt `/admin`, `/auth` und die festen Auth-Pfade ab. Settings und `RouteMap` verlassen `into_router` als `Extension`, nicht als neue Felder von `AppState`. `merge` bleibt für unbenannte Router gültig.

## 5. Verträge zwischen Modulen (IST)

| Producer | Consumer | Vertrag | Datei |
|---|---|---|---|
| `ferrioxid-macros` | `ferrioxid-core` | `FerrioxidModel::descriptor/columns/constraints/schema_sql(DatabaseKind)` | `crates/ferrioxid-core/src/lib.rs` |
| `ferrioxid-macros` | `ferrioxid-db` | `ModelRepository` und SeaQuery-CRUD | `crates/ferrioxid-db/src/lib.rs`, `crates/ferrioxid-db/src/sql.rs` |
| `ferrioxid-api` | `ferrioxid-db` | generische `M: ModelRepository` | `crates/ferrioxid-api/src/lib.rs` |
| `myshop` | `ferrioxid` | `prelude`, `db::connect`, `App` | `crates/ferrioxid/src/lib.rs` |

## 6. Architektur für spätere Module (SOLL, nicht implementiert)

```text
Rust-Modelle + Makros
 → reichhaltige Model Registry (Fields, Relations, Constraints, Policies)
 → DB / Migrations / Validierung / Auth
 → REST + OpenAPI + Admin-Metadaten
 → serverseitiger Admin (seit v0.13 vorhanden)
 → CLI / Docker-Deployment / Plugins
```

`ModelDescriptor` enthält weiterhin keinen Feldkatalog. Spalten, Unique-Gruppen, Index-Gruppen und Foreign Keys stehen in `ColumnSpec` und `TableConstraints`. Daraus entstehen Snapshot, SQL und die Lazy-Loading-Methoden. Policies, Eager Loading und eine automatische Entdeckung aller Rust-Structs sind nicht implementiert. Schema-Snapshots bleiben serialisierbar und deterministisch. Eine Laufzeit-Registry führt nur explizit registrierte Modelle.

## 7. Sicherheitsgrenzen und offene Risiken

- `allow_anonymous=true` ist ausschließlich für das lokal gebundene Demo gedacht.
- `BIND_ADDRESS` kann den Dienst über localhost hinaus verfügbar machen.
- API-JSON-Rejections von Axum verwenden nicht den Ferrioxid-Fehlerkörper.
- Die HTTP-Listenabfrage beschränkt die Antwort auf maximal 100 Datensätze; keine Pagination.
- `CREATE TABLE IF NOT EXISTS` meldet keinen Schema-Drift.
- OpenAPI und ein allgemeines Plugin-System sind nicht implementiert. Auth, Migrationen und der native Admin sind vorhanden.

## 8. Admin-Schreiben in v0.18

`ModelRepository` hat `get_tx`, `create_tx`, `update_tx` und `delete_tx` auf `&DatabaseTransaction`. Die SQL-Helfer nehmen `&impl ConnectionTrait`. `ManagedModel::update` und `remove` erhalten den `AuthUser`. Ein erfolgreiches Anlegen, Ändern oder Löschen im Admin öffnet eine Transaktion, schreibt die Zeile, schreibt `_ferrum_admin_audit` und committet. Ein Abbruch verwirft beides. `ModelRepository::create` ohne den Admin-Pfad schreibt keine Historie.

Verzeichnisereignisse (`user_create`, `user_update`, `password_reset`, `role_assign`, `role_revoke`) entstehen nach dem `AuthService`-Aufruf. Sie teilen sich keine Transaktion mit der Auth-Änderung. Der Self-Service unter `/auth/password-reset` ist davon getrennt: Token-Verbrauch, Passwort, Sitzungswiderruf und Audit sind eine Transaktion. Die E-Mail folgt nach dem Commit. Siehe [password-reset.md](password-reset.md).

Entwürfe liegen in `_ferrum_admin_drafts` und verweisen als Text-UUID auf `_ferrum_sessions.id`. Auth speichert Sitzungs-IDs als Text. Dieselbe Textbindung gilt für Audit-IDs. Die Migration ist `crates/ferrioxid-auth/migrations/0002_admin_journal.json`. `0001_auth_schema` bleibt unverändert.

Weiterlesen: [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [security.md](security.md), [admin-audit-log.md](admin-audit-log.md).
