# Architektur – Modulgrenzen und Laufzeit

[Zur Übersicht](index.md) · **Status:** Modulgrenzen bleiben gültig. v0.24 legt Settings in `ferrum-migrations` und benannte Routen sowie View-Helfer in `ferrum-api`. `AppState` bleibt unverändert. Es gibt keinen zweiten Router und kein Settings-Singleton. v0.23 legt den Versand in `ferrum-mail` und den Reset in `ferrum-auth`. Es gibt keinen zweiten Passwort- oder Sitzungsdienst. v0.22 hängt Seiten, Aktionen, Filter, Widgets und Vorlagen an dieselbe `AdminRegistry`. Es gibt keine zweite Admin-Engine. v0.21 hängt die lesende Migrationsdiagnose und das Upgrade-Manifest an dieselbe Migrations-Engine. Es gibt keine zweite Historie und keinen schreibenden Diagnoseweg. v0.19 ergänzt `Date`, `Time` und `DateTime<Utc>` in denselben Metadaten, derselben Migrations-Engine und demselben Repository. v0.20 legt Modell-, Auth- und Audit-Schreibvorgänge auf dieselbe `DatabaseTransaction`. Es gibt keine zweite Transaktions-Engine. Siehe [transactions.md](transactions.md).

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
examples/myshop → ferrum (Fassade)
                    ├── ferrum-core
                    ├── ferrum-macros
                    ├── ferrum-db → ferrum-core, SeaORM
                    ├── ferrum-auth → ferrum-core, ferrum-db, ferrum-migrations
                    ├── ferrum-admin → ferrum-auth, ferrum-core, ferrum-db
                    ├── ferrum-api → ferrum-admin, ferrum-auth, ferrum-core, ferrum-db, ferrum-static, ferrum-middleware, Axum
                    ├── ferrum-forms → ferrum-core
                    ├── ferrum-static → Axum
                    ├── ferrum-storage → ferrum-auth, ferrum-core
                    └── ferrum-middleware → Axum, tracing
ferrum-cli → ferrum-auth, ferrum-db, ferrum-migrations
ferrum-admin → Tera-HTML und CSS unter `/admin`, JSON unter `/admin/api`
```

Physische Crates: `crates/ferrum/`, `crates/ferrum-core/`, `crates/ferrum-macros/`, `crates/ferrum-db/`, `crates/ferrum-migrations/`, `crates/ferrum-auth/`, `crates/ferrum-api/`, `crates/ferrum-cli/`, `crates/ferrum-forms/`, `crates/ferrum-static/`, `crates/ferrum-storage/`, `crates/ferrum-middleware/`. `ferrum-forms` hängt nicht von Axum ab. `ferrum-storage` hängt nicht von `ferrum-api` ab. `ferrum-auth` hängt nicht von `ferrum-api` ab. `Cargo.toml` legt Workspace Edition 2021 fest. Versionsangaben und Features stehen dort, nicht in dieser Dokumentation.

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

Die Prüfung `allow_anonymous` geschieht **beim Registrieren der Route**. Ohne Freigabe werden ausschließlich 403-Handler registriert.

v0.24 hängt benannte Anwendungsrouten über `App::urls` an denselben Router. `UrlSet` lehnt `/admin`, `/auth` und die festen Auth-Pfade ab. Settings und `RouteMap` verlassen `into_router` als `Extension`, nicht als neue Felder von `AppState`. `merge` bleibt für unbenannte Router gültig.

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
 → serverseitiger Admin (seit v0.13 vorhanden)
 → CLI / Docker-Deployment / Plugins
```

`ModelDescriptor` enthält weiterhin keinen Feldkatalog. Spalten, Unique-Gruppen, Index-Gruppen und Foreign Keys stehen in `ColumnSpec` und `TableConstraints`. Daraus entstehen Snapshot, SQL und die Lazy-Loading-Methoden. Policies, Eager Loading und eine automatische Entdeckung aller Rust-Structs sind nicht implementiert. Schema-Snapshots bleiben serialisierbar und deterministisch. Eine Laufzeit-Registry führt nur explizit registrierte Modelle.

## 7. Sicherheitsgrenzen und offene Risiken

- `allow_anonymous=true` ist ausschließlich für das lokal gebundene Demo gedacht.
- `BIND_ADDRESS` kann den Dienst über localhost hinaus verfügbar machen.
- API-JSON-Rejections von Axum verwenden nicht den Ferrum-Fehlerkörper.
- Die HTTP-Listenabfrage beschränkt die Antwort auf maximal 100 Datensätze; keine Pagination.
- `CREATE TABLE IF NOT EXISTS` meldet keinen Schema-Drift.
- OpenAPI und ein allgemeines Plugin-System sind nicht implementiert. Auth, Migrationen und der native Admin sind vorhanden.

## 8. Admin-Schreiben in v0.18

`ModelRepository` hat `get_tx`, `create_tx`, `update_tx` und `delete_tx` auf `&DatabaseTransaction`. Die SQL-Helfer nehmen `&impl ConnectionTrait`. `ManagedModel::update` und `remove` erhalten den `AuthUser`. Ein erfolgreiches Anlegen, Ändern oder Löschen im Admin öffnet eine Transaktion, schreibt die Zeile, schreibt `_ferrum_admin_audit` und committet. Ein Abbruch verwirft beides. `ModelRepository::create` ohne den Admin-Pfad schreibt keine Historie.

Verzeichnisereignisse (`user_create`, `user_update`, `password_reset`, `role_assign`, `role_revoke`) entstehen nach dem `AuthService`-Aufruf. Sie teilen sich keine Transaktion mit der Auth-Änderung. Der Self-Service unter `/auth/password-reset` ist davon getrennt: Token-Verbrauch, Passwort, Sitzungswiderruf und Audit sind eine Transaktion. Die E-Mail folgt nach dem Commit. Siehe [password-reset.md](password-reset.md).

Entwürfe liegen in `_ferrum_admin_drafts` und verweisen als Text-UUID auf `_ferrum_sessions.id`. Auth speichert Sitzungs-IDs als Text. Dieselbe Textbindung gilt für Audit-IDs. Die Migration ist `crates/ferrum-auth/migrations/0002_admin_journal.json`. `0001_auth_schema` bleibt unverändert.

Weiterlesen: [core.md](core.md), [macros.md](macros.md), [database.md](database.md), [api.md](api.md), [security.md](security.md), [admin-audit-log.md](admin-audit-log.md).
