# Changelog

## Unreleased — Stabilität, Sicherheit und Auslieferung (v0.8.1)

- `GET /auth/csrf` berechnet ein HMAC-Token aus Sitzungs-ID, Session-Hash und stabilem Schlüssel. Ein zweiter Tab ersetzt das Token nicht.
- Objektregeln liefern `QueryScope`. REST und Admin filtern Listen, Zähler, Details, PATCH und DELETE mit derselben SQL-Bedingung. Ohne ausdrücklichen Scope bleibt `None`.
- Das Admin-Frontend wird beim Rust-Build aus `apps/admin-ui/dist` eingebettet. `FERRUM_ADMIN_DIST` bleibt ein Entwicklungs-Override. Node.js läuft zur Laufzeit nicht.
- Admin-Listen enthalten `total`. Ungebundene Objektregeln bleiben 403. Fehlerantworten nutzen weiterhin Problem-Details und geben keine Datenbanktexte, Hashes oder Tokens aus.
- Nicht enthalten: OAuth, MFA, GraphQL, WebSockets, Plugins, Mandanten, JWT und ein neues Migrationsformat. `0001`, `0002` und `0003` bleiben unverändert. Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Automatischer Admin

- Neue Crate `ferrum-admin` und React-Anwendung `apps/admin-ui`. Modelle mit `admin = true` erscheinen nach expliziter `AdminRegistry`-Registrierung.
- Admin-Metadaten hängen am `FerrumModel`-Trait und ändern `ColumnSpec` nicht. Bestehende Migrationen bleiben gültig.
- HTTP unter `/admin/api`. Die Oberfläche wird aus `apps/admin-ui/dist` oder `FERRUM_ADMIN_DIST` ausgeliefert. Ohne Build antwortet `/admin` mit 503.
- `GET /auth/csrf` rotiert das sitzungsgebundene CSRF-Token, ohne die Sitzung zu ersetzen.
- Benutzer- und Rollenverzeichnis nur für Staff-Superuser. Superuser-Vergabe ist blockiert. Passwort-Hashes verlassen den Server nicht.
- Objektregeln blockieren Admin-Listen und Fremdschlüssel-Auswahlen, bis ein SQL-Scope existiert.
- myshop registriert Produkt, Kategorie und interne Notiz im Admin. `0001`, `0002` und `0003` sind unverändert.
- Nicht enthalten: OAuth, MFA, Plugin-System, Drag-and-Drop-Dashboard, GraphQL, WebSockets, Mandanten.

## Unreleased — Authentifizierung, Sitzungen und Berechtigungen

- Neue Crate `ferrum-auth` mit Argon2id, serverseitigen Sitzungen, HttpOnly-Cookie `ferrum_session`, CSRF-Token und prozesslokalem Login-Limit.
- Tabellen `_ferrum_users`, `_ferrum_sessions`, `_ferrum_roles`, `_ferrum_permissions`, `_ferrum_user_roles` und `_ferrum_role_permissions`. Historie `_ferrum_auth_migrations`, Datei `crates/ferrum-auth/migrations/0001_auth_schema.json`.
- HTTP: `POST /auth/login`, `POST /auth/logout`, `GET /auth/me`, `POST /auth/change-password`.
- Modelle: `permissions` und `object_policy`. Standard bleibt Verweigerung. `allow_anonymous = true` bleibt die öffentliche Demo-Freigabe.
- Objektregeln ohne SQL-Scope blockieren Listen. Ein abgelehnter Einzeldatensatz antwortet mit 404.
- CLI: `admin create-user`, `auth list-users` und Befehle für Rollen. `migrate up` wendet Auth-Migrationen zuerst an. Kein voreingestelltes Admin-Passwort.
- myshop: geschützte `InternalNote` und `0003_internal_notes.json`. Produkte und Kategorien bleiben anonyme Demo-Routen.
- `0001_initial.json` ist unverändert. Die Cargo-Version bleibt `0.1.0`.
- Nicht enthalten: OAuth, MFA, JWT für externe APIs, E-Mail-Reset. Der Admin folgt in v0.8.

## Unreleased — Constraints, Foreign Keys und typisierte Beziehungen

- `#[field(unique)]` und `#[ferrum(unique_together = "a,b")]` erzeugen einen benannten Unique Index. Mehrere `NULL`-Werte bleiben erlaubt.
- `#[field(index)]` und `#[ferrum(index_together = "a,b")]` erzeugen sekundäre Indizes. Ein Primärschlüssel oder Unique Constraint wird nicht doppelt indiziert.
- `#[field(foreign_key = "tabelle.spalte", on_delete = "restrict|cascade|set_null")]` erzeugt einen echten Foreign Key. `ON UPDATE` bleibt `NO ACTION`. `set_null` verlangt eine nullable Spalte.
- Belongs-To und Has-Many laden typisierte Modelle. Die Gegenrichtung wird explizit deklariert. Eager Loading fehlt.
- Snapshots der Formatversion 1 nehmen die neuen Objekte auf, lassen leere Listen weg und lesen alte Dateien weiter. `0001_initial.json` behält die Checksum `06393cd03c3d2afb2e4fbe111d0cf8430caf2ef0eaeda3c503d21edaf3f7a026`.
- Neue Operationen: `add_unique_constraint`, `drop_unique_constraint`, `add_index`, `drop_index`. `add_foreign_key` und `drop_foreign_key` werden erkannt und blockiert. Ein Foreign Key auf einer neuen Spalte hängt an `add_column`.
- Unique- und Foreign-Key-Verletzungen werden HTTP 422. SQLite-`RESTRICT` nutzt den erweiterten Code 1811 mit der festen Meldung `FOREIGN KEY constraint failed`.
- myshop erhält `Category`, ein optionales `Product.category_id` und `0002_categories_and_product_relations.json`.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Explizite Umbenennung, SQLite-Umbau und Baseline

- `RenameColumn` und `RenameTable` bleiben Daten erhalten und werden nicht aus einem Drop plus Add erraten.
- SQLite baut unterstützte Spaltenänderungen als transaktionale Kopie um. Geprüft werden `string` ↔ `i64`, Nullbarkeit und Bool-Defaults. `NULL` bei einer `NOT NULL`-Verschärfung blockiert ohne Ersatzwert.
- PostgreSQL führt dieselben unterstützten Änderungen per `ALTER COLUMN` aus. Der SQLite-Umbau läuft dort nicht.
- `ferrum migrate baseline` schreibt nach `--confirm` nur Historieneinträge, wenn das SQLite-Schema zum letzten Migrationsziel passt. Destruktive oder typändernde Ketten werden nicht markiert. PostgreSQL-Baseline bricht ab.
- `ferrum migrate reseal` erneuert die Checksum einer noch nicht angewendeten Datei.

## Unreleased — Versionierte Migrationen

- Neue Crate `ferrum-migrations` mit Snapshot, Diff, Checksum, Historie und Runner für SQLite und PostgreSQL.
- CLI: `ferrum migrate make`, `ferrum migrate up`, `ferrum migrate status`. Modelle kommen über `ferrum.toml` und `myshop --print-schema`, nicht über Quelltextanalyse.
- Sichere Operationen: neue Tabelle, nullable Spalte, boolesche NOT-NULL-Spalte mit Default. Löschungen nur mit `--allow-destructive`.
- Primärschlüsseländerungen und nicht geprüfte Typänderungen werden abgelehnt. Umbenennung und der begrenzte Umbau stehen im neueren Eintrag.
- `myshop --bootstrap` legt nichts mehr an und markiert vorhandene Tabellen nicht als migriert.
- Breaking Change: der bisherige Bootstrap-Start erzeugt kein Schema mehr. Neue Datenbanken brauchen `ferrum migrate up`.

## Unreleased — SQLite als lokale Standarddatenbank

- Ohne `DATABASE_URL` verwendet myshop `sqlite://db.sqlite3?mode=rwc` im Arbeitsverzeichnis.
- PostgreSQL bleibt über `DATABASE_URL` unterstützt. Docker ist optional.
- DDL und CRUD laufen über eine SeaQuery-Schicht in `crates/ferrum-db/src/sql.rs`. Handler bleiben dialektunabhängig.
- SQLite speichert UUID als Text und Bool als Integer `0`/`1`. Die JSON-API bleibt für beide Backends gleich.
- SQLite-Verbindungen nutzen eine Pool-Verbindung und `PRAGMA foreign_keys=ON`. `sqlite::memory:` bleibt dadurch eine Datenbank.
- `sqlite_http_roundtrip` läuft in `cargo test --workspace` und fasst die lokale Datei nicht an. `postgres_http_roundtrip` bleibt ignoriert.
- Breaking Change: `FerrumModel::schema_sql` erwartet `DatabaseKind` und liefert `String`. Das bisherige myshop-Default war eine PostgreSQL-URL.

## Unreleased — CRUD mit PATCH und DELETE

- Jedes registrierte Modell erhält `PATCH /{id}` und `DELETE /{id}` zusätzlich zu Liste, Einzelabruf und POST.
- Das Makro erzeugt `{Model}Update`. Jedes änderbare Feld ist ein `PatchField<T>` mit den Zuständen `Missing`, `Null` und `Value`.
- Fehlende PATCH-Schlüssel behalten den gespeicherten Wert. `{}` liefert den Datensatz unverändert mit HTTP 200.
- JSON-`null` auf einer `NOT NULL`-Spalte ergibt HTTP 422. JSON-`null` auf `Option<T>` schreibt SQL `NULL`.
- `#[field(readonly)]` und der Primärschlüssel sind nicht im Update-DTO. Unbekannte Felder lehnt Serde ab.
- `DELETE` ist an `WHERE "id" = $1` gebunden. HTTP 204 hat keinen Körper. Eine zweite Löschung derselben UUID ergibt HTTP 404.
- Nullable Spalten sind `Option<String>`, `Option<i64>`, `Option<bool>` oder `Option<Uuid>`. `id` bleibt Pflicht und nicht null.
- Breaking Change: `ModelRepository` verlangt jetzt die zugehörigen Typen `Update` sowie die Methoden `update` und `delete`.
- Der PostgreSQL-HTTP-Test deckt PATCH, DELETE, 403 und Nullable-Felder ab, bleibt `#[ignore]` und wurde hier nicht gegen eine Datenbank ausgeführt.

## Unreleased — API detail endpoint and documentation

- Added generated, typed `ModelRepository::get(UUID)` and `GET /{id}` routes.
- Missing records return HTTP 404 with the shared Ferrum error body.
- Detail routes remain denied (403) for models without `allow_anonymous = true`.
- Tightened the PostgreSQL integration-test precondition; no silent success without `DATABASE_URL`.
- Documented the actual required Serde derives on model structs.

## 0.1.0 — source prototype

- Cargo workspace, FerrumModel macro, SeaORM PostgreSQL repository, Axum GET/POST routes.
- Typed create DTOs and typed model reads; 100-row list cap with `truncated` indicator.
- Required and trimmed string-length validation and Boolean SQL `DEFAULT TRUE/FALSE`.
- Shared error response for Ferrum validation and permission errors.
- Optional PostgreSQL HTTP integration test, demo app, Docker Compose, and modular documentation.
- Compilation and runtime verification of subsequent changes remain outstanding in this environment.
