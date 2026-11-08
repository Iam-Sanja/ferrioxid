# ferrioxide-db – A repository layer for SQLite and PostgreSQL

[For overview](index.md) · **IST files:** `crates/ferrioxid-db/src/lib.rs`, `crates/ferrioxid-db/src/sql.rs`.] The repository implementation generates `crates/ferrioxid-macros/src/lib.rs`.

## Auth-Tabellen (v0.7)

`_ferrum_users`, `_ferrum_sessions`, `_ferrum_roles`, `_ferrum_permissions`, `_ferrum_user_roles` and `_ferrum_role_permissions` are in `ferrioxid-auth` and use SeaQuery on the same connection. They are not `FerrioxidModel` CRUD models so that `password_hash` does not appear as a REST resource. Time stamps are `i64`. UUID is the UUID type in SQLite Text and PostgreSQL. The introspection passes table names with prefix `_ferrum_`.

SQLite uses `max_connections(1)`. During a write transaction, no call may request a second connection from the same pool. `conclude` and the `*_in` methods are in [transactions.md](transactions.md)].

## Responsibility (IST)

- `connect(url)` opens SeaORM. URLs with `sqlite:` set the pool to a connection and activate `PRAGMA foreign_keys=ON`.
- Without set `DATABASE_URL`, myshop uses `DEFAULT_SQLITE_URL`: `sqlite://db.sqlite3?mode=rwc`. The path is relative to the working directory of the process.
- `ModelRepository` remains the public limit for `list`, `get`, `create`, `update` and `delete`. The same methods exist with `_scoped` and a `QueryScope`. Traders do not know the dialect. See [query-scopes.md](query-scopes.md)].
- There is a query implementation. SeaQuery renders SQLite or PostgreSQL. MySQL is rejected.

```rust
#[async_trait::async_trait]
pub trait ModelRepository: FerrioxidModel + Serialize + DeserializeOwned {
    type Create: DeserializeOwned + Send;
    type Update: DeserializeOwned + Send;
    async fn list(db: &DatabaseConnection) -> Result<ListPage<Self>, FerrioxidError>;
    async fn get(db: &DatabaseConnection, id: uuid::Uuid) -> Result<Option<Self>, FerrioxidError>;
    async fn create(db: &DatabaseConnection, input: Self::Create) -> Result<Self, FerrioxidError>;
    async fn update(db: &DatabaseConnection, id: uuid::Uuid, input: Self::Update) -> Result<Self, FerrioxidError>;
    async fn delete(db: &DatabaseConnection, id: uuid::Uuid) -> Result<DeleteOutcome, FerrioxidError>;
}
```

`ListPage<T>` contains `data: Vec<T>`, `limit: usize`, `truncated: bool`. `DeleteOutcome` is `Deleted` or `Missing`. Public methods provide the model, not `serde_json::Value`. The JSON image remains within `ferrioxid-db`.

## Dialect and Types (IST)

`DatabaseKind` ist `Sqlite` oder `Postgres`. `ColumnSpec` beschreibt Name, `ColumnKind`, Nullbarkeit, Primärschlüssel und ein optionales Bool-Default. `ColumnKind` umfasst `Uuid`, `Text`, `BigInt`, `Boolean`, `Date`, `Time` und `DateTime`. SQLite legt die drei Zeittypen als Text ab. PostgreSQL nutzt `DATE`, `TIME` und `TIMESTAMPTZ`. Das Makro legt diese Metadaten fest. `render_schema` baut daraus `CREATE TABLE IF NOT EXISTS`. Details stehen in [date-time-types.md](date-time-types.md).

| Modelltyp | PostgreSQL | SQLite | JSON-API |
|---|---|---|---|
| `Uuid` | `uuid` | `text` | String |
| `String` | `text` | `text` | String |
| `i64` | `bigint` | `bigint` | Zahl |
| `bool` | `bool`, Default `TRUE`/`FALSE` | `integer`, Default `1`/`0` | `true`/`false` |
| `Option<T>` |Column without `NOT NULL`|Column without `NOT NULL`|Value or `null`|

`DateTime`, `Decimal`, JSON columns and foreign key relationships are not implemented. SeaQuery determines the exact spelling of the keywords. The contract is type mapping, not a fixed DDL string.

Parameters come from SeaQuery: PostgreSQL `$1`, `$2`, SQLite `?`. Identifiers come only from checked macro attributes and are quoted. Returns of insert and update use `RETURNING`. The bundled SQLite library of sqlx supports this. Lines are read column by column and then deserialized into the model. An empty PATCH does not perform an update, but reads the line. A non-empty PATCH only writes modified columns.

## Interrogation behaviour (IST)

### Create

1. DTO from HTTP or direct Rust call.
2. Generated `required`, Length and `min` test.
3. UUID with `Uuid::new_v4()`.
4. SeaQuery-`INSERT` with bound values and `RETURNING`.
5. Internal deserialization to the model. Errors become `FerrioxidError::Database`.

### Update

1. `PatchField::Missing` leaves the column unchanged.
2. `PatchField::Null` is a validation error for `NOT NULL` and writes SQL `NULL` for nullable columns.
3. `PatchField::Value` checks and stores the original value.
4. No amended column: `get`. Otherwise, a `UPDATE ... WHERE "id" = ... RETURNING`.
5. No line gives `NotFound`. There is no version field. The last written instruction wins.

### Delete, Get, List

`DELETE` filters exclusively via the bound UUID. `rows_affected() == 0` becomes `Missing`. `get` provides `Ok(None)` if no line exists. `list` sorted by `"id"` and reads 101 lines. The answer contains a maximum of 100 objects. `truncated` is true when there were more lines. The UUID order is not the creation time. There is no pagination.

## Einrichtung

SQLite is the local standard. An installation and Docker are not necessary for this. sqlx brings SQLite. Schema changes for applications run through [migrations.md](migrations.md)]. `render_schema` remains the legacy bootstrap with `IF NOT EXISTS` for empty test databases. Migrations use `CREATE TABLE` without `IF NOT EXISTS`.

```text
sqlite://db.sqlite3?mode=rwc
sqlite::memory:
postgres://user:password@localhost:5432/ferrioxid
```

`sqlite::memory:` is a separate database for each connection. Ferrioxide limits SQLite pools to one connection so that tests and the application see the same database. The file `db.sqlite3` is created in the working directory and is contained in `.gitignore`. PostgreSQL remains a supported backend and is dialed via `DATABASE_URL`. `compose.yaml` can optionally start the server. Bootstrap only runs `CREATE TABLE IF NOT EXISTS`, does not delete existing rows and does not change existing tables. This is not a migration engine.

## Error and security (IST)

Deserialization errors and unknown SQL errors become `FerrioxidError::Database`. The HTTP layer then responds with 500 and a generic text. The database detail goes to the tracing log, not the answer. `DATABASE_URL` is not logged. `sqlx::ErrorKind::UniqueViolation` and `ForeignKeyViolation` are `Validation` and thus 422. SQLite reports `ON DELETE RESTRICT` as extended code `1811` with the fixed message `FOREIGN KEY constraint failed`. This will also be 422. Missing lines remain 404, unreleased routes 403. SQLite foreign keys are on. Models can generate unique indexes, secondary indexes and foreign keys. Relationship methods are in [relations.md](relations.md)]. `find_by_column` loads typed child lines. SQLite checks types weaker than PostgreSQL. Ferrioxide checks the supported values before writing in Rust.

## Not implemented (reference)

- `migrate down` and a transaction API for multiple models outside of a single migration. Explicit renaming, the tested SQLite conversion and the SQLite baseline are described in [migrations.md](migrations.md)].
- Own error codes apart from the two fixed validation texts for Unique and Foreign Key.
- `DateTime`, JSON-Spalten, Many-to-Many, Eager Loading, MySQL.
- Pool sizes and timeouts as an application configuration except for the fixed SQLite limit of a connection.
- Optimistic blocking.

## Tests

`cargo test --workspace` führt `sqlite_http_roundtrip` gegen `sqlite::memory:` aus. Der Test verändert `db.sqlite3` nicht. `postgres_http_roundtrip` bleibt `#[ignore]` und verlangt eine gesetzte `DATABASE_URL`. Ohne laufenden PostgreSQL-Server ist er kein Erfolgsnachweis. `.github/workflows/ci.yml` enthält einen separaten PostgreSQL-Job für die ignorierten Tests. Ein lokaler Workspace-Lauf startet diesen Server nicht. Siehe [testing.md](testing.md).

## Admin-Listen

`query_admin_page` in `crates/ferrioxid-db/src/sql.rs` uses the same SeaQuery layer. Column names come from the descriptor, values are parameters. The search is `LIKE` with `ESCAPE` and runs under SQLite. PostgreSQL gets the same expression, no dialect-specific input from the interface. The public list limit remains 100. The admin page is at most 100 lines, plus one line to detect `has_more`.
