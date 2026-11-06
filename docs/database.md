# ferrum-db – Eine Repository-Schicht für SQLite und PostgreSQL

[Zur Übersicht](index.md) · **IST-Dateien:** `crates/ferrum-db/src/lib.rs`, `crates/ferrum-db/src/sql.rs`. Die Repository-Implementierung erzeugt `crates/ferrum-macros/src/lib.rs`.

## Auth-Tabellen (v0.7)

`_ferrum_users`, `_ferrum_sessions`, `_ferrum_roles`, `_ferrum_permissions`, `_ferrum_user_roles` und `_ferrum_role_permissions` liegen in `ferrum-auth` und nutzen SeaQuery auf derselben Verbindung. Sie sind keine `FerrumModel`-CRUD-Modelle, damit `password_hash` nicht als REST-Ressource erscheint. Zeitstempel sind `i64`. UUID ist in SQLite Text und in PostgreSQL der UUID-Typ. Die Introspektion übergeht Tabellennamen mit Präfix `_ferrum_`.

## Verantwortung (IST)

- `connect(url)` öffnet SeaORM. URLs mit `sqlite:` setzen den Pool auf eine Verbindung und aktivieren `PRAGMA foreign_keys=ON`.
- Ohne gesetzte `DATABASE_URL` verwendet myshop `DEFAULT_SQLITE_URL`: `sqlite://db.sqlite3?mode=rwc`. Der Pfad ist relativ zum Arbeitsverzeichnis des Prozesses.
- `ModelRepository` bleibt die öffentliche Grenze für `list`, `get`, `create`, `update` und `delete`. Handler kennen den Dialekt nicht.
- Es gibt eine Abfrageimplementierung. SeaQuery rendert SQLite oder PostgreSQL. MySQL wird abgelehnt.

```rust
#[async_trait::async_trait]
pub trait ModelRepository: FerrumModel + Serialize + DeserializeOwned {
    type Create: DeserializeOwned + Send;
    type Update: DeserializeOwned + Send;
    async fn list(db: &DatabaseConnection) -> Result<ListPage<Self>, FerrumError>;
    async fn get(db: &DatabaseConnection, id: uuid::Uuid) -> Result<Option<Self>, FerrumError>;
    async fn create(db: &DatabaseConnection, input: Self::Create) -> Result<Self, FerrumError>;
    async fn update(db: &DatabaseConnection, id: uuid::Uuid, input: Self::Update) -> Result<Self, FerrumError>;
    async fn delete(db: &DatabaseConnection, id: uuid::Uuid) -> Result<DeleteOutcome, FerrumError>;
}
```

`ListPage<T>` enthält `data: Vec<T>`, `limit: usize`, `truncated: bool`. `DeleteOutcome` ist `Deleted` oder `Missing`. Öffentliche Methoden liefern das Model, kein `serde_json::Value`. Die JSON-Abbildung bleibt innerhalb von `ferrum-db`.

## Dialekt und Typen (IST)

`DatabaseKind` ist `Sqlite` oder `Postgres`. `ColumnSpec` beschreibt Name, `ColumnKind`, Nullbarkeit, Primärschlüssel und ein optionales Bool-Default. Das Makro legt diese Metadaten fest. `render_schema` baut daraus `CREATE TABLE IF NOT EXISTS`.

| Modelltyp | PostgreSQL | SQLite | JSON-API |
|---|---|---|---|
| `Uuid` | `uuid` | `text` | String |
| `String` | `text` | `text` | String |
| `i64` | `bigint` | `bigint` | Zahl |
| `bool` | `bool`, Default `TRUE`/`FALSE` | `integer`, Default `1`/`0` | `true`/`false` |
| `Option<T>` | Spalte ohne `NOT NULL` | Spalte ohne `NOT NULL` | Wert oder `null` |

`DateTime`, `Decimal`, JSON-Spalten und Fremdschlüsselbeziehungen sind nicht implementiert. SeaQuery bestimmt die genaue Schreibweise der Schlüsselwörter. Der Vertrag ist das Typ-Mapping, nicht eine feste DDL-Zeichenkette.

Parameter kommen von SeaQuery: PostgreSQL `$1`, `$2`, SQLite `?`. Bezeichner stammen nur aus geprüften Makroattributen und werden gequotet. Rückgaben von Insert und Update nutzen `RETURNING`. Die gebündelte SQLite-Bibliothek von sqlx unterstützt das. Zeilen werden spaltenweise gelesen und danach in das Model deserialisiert. Ein leeres PATCH führt kein Update aus, sondern liest die Zeile. Ein nicht leeres PATCH schreibt nur geänderte Spalten.

## Abfrageverhalten (IST)

### Create

1. DTO aus HTTP oder direktem Rust-Aufruf.
2. Generierte `required`-, Längen- und `min`-Prüfung.
3. UUID mit `Uuid::new_v4()`.
4. SeaQuery-`INSERT` mit gebundenen Werten und `RETURNING`.
5. Interne Deserialisierung zum Model. Fehler werden `FerrumError::Database`.

### Update

1. `PatchField::Missing` lässt die Spalte unverändert.
2. `PatchField::Null` ist bei `NOT NULL` ein Validierungsfehler und schreibt bei nullable Spalten SQL `NULL`.
3. `PatchField::Value` prüft und speichert den Originalwert.
4. Keine geänderte Spalte: `get`. Sonst ein `UPDATE ... WHERE "id" = ... RETURNING`.
5. Keine Zeile ergibt `NotFound`. Es gibt kein Versionsfeld. Die zuletzt festgeschriebene Anweisung gewinnt.

### Delete, Get, List

`DELETE` filtert ausschließlich über die gebundene UUID. `rows_affected() == 0` wird `Missing`. `get` liefert `Ok(None)`, wenn keine Zeile existiert. `list` sortiert nach `"id"` und liest 101 Zeilen. Die Antwort enthält höchstens 100 Objekte. `truncated` ist wahr, wenn mehr Zeilen vorhanden waren. Die UUID-Reihenfolge ist nicht die Erstellungszeit. Es gibt keine Pagination.

## Einrichtung

SQLite ist der lokale Standard. Eine Installation und Docker sind dafür nicht nötig. sqlx bringt SQLite mit. Schemaänderungen für Anwendungen laufen über [migrations.md](migrations.md). `render_schema` bleibt der Legacy-Bootstrap mit `IF NOT EXISTS` für leere Testdatenbanken. Migrationen verwenden `CREATE TABLE` ohne `IF NOT EXISTS`.

```text
sqlite://db.sqlite3?mode=rwc
sqlite::memory:
postgres://user:password@localhost:5432/ferrum
```

`sqlite::memory:` ist eine eigene Datenbank je Verbindung. Ferrum begrenzt SQLite-Pools auf eine Verbindung, damit Tests und die Anwendung dieselbe Datenbank sehen. Die Datei `db.sqlite3` entsteht im Arbeitsverzeichnis und ist in `.gitignore` enthalten. PostgreSQL bleibt ein unterstütztes Backend und wird über `DATABASE_URL` gewählt. `compose.yaml` kann den Server optional starten. Bootstrap führt nur `CREATE TABLE IF NOT EXISTS` aus, löscht vorhandene Zeilen nicht und ändert bestehende Tabellen nicht. Das ist keine Migrations-Engine.

## Fehler und Sicherheit (IST)

Deserialisierungsfehler und unbekannte SQL-Fehler werden `FerrumError::Database`. Die HTTP-Schicht antwortet dann mit 500 und einem generischen Text. Das Datenbankdetail geht ins Tracing-Log, nicht in die Antwort. `DATABASE_URL` wird nicht protokolliert. `sqlx::ErrorKind::UniqueViolation` und `ForeignKeyViolation` werden `Validation` und damit 422. SQLite meldet `ON DELETE RESTRICT` als erweiterten Code `1811` mit der festen Meldung `FOREIGN KEY constraint failed`. Auch das wird 422. Fehlende Zeilen bleiben 404, nicht freigegebene Routen 403. SQLite-Fremdschlüssel sind eingeschaltet. Modelle können Unique Indexes, sekundäre Indexes und Foreign Keys erzeugen. Beziehungsmethoden stehen in [relations.md](relations.md). `find_by_column` lädt typisierte Kindzeilen. SQLite prüft Typen schwächer als PostgreSQL. Ferrum prüft die unterstützten Werte vor dem Schreiben in Rust.

## Nicht implementiert (SOLL)

- `migrate down` und eine Transaktions-API für mehrere Modelle außerhalb einer einzelnen Migration. Explizite Umbenennung, der geprüfte SQLite-Umbau und die SQLite-Baseline sind in [migrations.md](migrations.md) beschrieben.
- Eigene Fehlercodes außer den beiden festen Validation-Texten für Unique und Foreign Key.
- `DateTime`, JSON-Spalten, Many-to-Many, Eager Loading, MySQL.
- Pool-Größen und Timeouts als Anwendungskonfiguration außer dem festen SQLite-Limit von einer Verbindung.
- Optimistisches Sperren.

## Tests

`cargo test --workspace` führt `sqlite_http_roundtrip` gegen `sqlite::memory:` aus. Der Test verändert `db.sqlite3` nicht. `postgres_http_roundtrip` bleibt `#[ignore]` und verlangt eine gesetzte `DATABASE_URL`. Ohne laufenden PostgreSQL-Server ist er kein Erfolgsnachweis. Die CI startet keinen Datenbankserver. Siehe [testing.md](testing.md).

## Admin-Listen

`query_admin_page` in `crates/ferrum-db/src/sql.rs` nutzt dieselbe SeaQuery-Schicht. Spaltennamen kommen aus dem Deskriptor, Werte sind Parameter. Die Suche ist `LIKE` mit `ESCAPE` und läuft unter SQLite. PostgreSQL erhält denselben Ausdruck, keine dialektspezifische Eingabe aus der Oberfläche. Das öffentliche Listenlimit 100 bleibt. Die Admin-Seite ist höchstens 100 Zeilen, plus eine Zeile zur Erkennung von `has_more`.
