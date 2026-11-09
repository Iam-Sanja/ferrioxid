# Ferrioxid-db – A repository layer for SQLite and PostgreSQL

[Overview](index.md) · **Implemented files:** `crates/ferrioxid-db/src/lib.rs`, `crates/ferrioxid-db/src/query.rs`, `crates/ferrioxid-db/src/sql.rs`. The repository implementation generates `crates/ferrioxid-macros/src/lib.rs`.

## Auth tables (v0.7)

`_ferrioxid_users`, `_ferrioxid_sessions`, `_ferrioxid_roles`, `_ferrioxid_permissions`, `_ferrioxid_user_roles` and `_ferrioxid_role_permissions` are in `ferrioxid-auth` and use SeaQuery on the same connection. They are not `FerrioxidModel` CRUD models so that `password_hash` does not appear as a REST resource. Time stamps are `i64`. UUID is the UUID type in SQLite Text and PostgreSQL. The introspection passes table names with prefix `_ferrioxid_`.

SQLite uses `max_connections(1)`. During a write transaction, no call may request a second connection from the same pool. `conclude` and the `*_in` methods are in [transactions.md](transactions.md).

## Responsibility (Implemented)

- `connect(url)` opens SeaORM. URLs with `sqlite:` set the pool to a connection and activate `PRAGMA foreign_keys=ON`.
- Without set `DATABASE_URL`, myshop uses `DEFAULT_SQLITE_URL`: `sqlite://db.sqlite3?mode=rwc`. The path is relative to the working directory of the process.
- `ModelRepository` is the public interface for `list`, `get`, `create`, `update` and `delete`. Each method also has a `_scoped` variant accepting a `QueryScope`. Handlers do not need to know the SQL dialect. See [query-scopes.md](query-scopes.md).
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

## Dialect and Types (Implemented)

`DatabaseKind` is `Sqlite` or `Postgres`. `ColumnSpec` describes the name, `ColumnKind`, nullability, primary-key status and optional Boolean default. `ColumnKind` includes `Uuid`, `Text`, `BigInt`, `Boolean`, `Date`, `Time` and `DateTime`. SQLite stores all three temporal types as text; PostgreSQL uses `DATE`, `TIME` and `TIMESTAMPTZ`. The macro supplies this metadata, and `render_schema` uses it to build `CREATE TABLE IF NOT EXISTS`. See [date-time-types.md](date-time-types.md).

| Model type | PostgreSQL | SQLite | JSON-API |
|---|---|---|---|
| `Uuid` | `uuid` | `text` | String |
| `String` | `text` | `text` | String |
| `i64` | `bigint` | `bigint` | Number |
| `bool` | `bool`, Default `TRUE`/`FALSE` | `integer`, Default `1`/`0` | `true`/`false` |
| `Option<T>` |Column without `NOT NULL`|Column without `NOT NULL`|Value or `null`|

`Decimal` and JSON model columns are not supported. Date/time types and single-column foreign keys are supported as described above and in [relations.md](relations.md). SeaQuery determines exact SQL keyword spelling; the contract is the type mapping rather than a fixed DDL string.

SeaQuery binds values using PostgreSQL `$1`, `$2` or SQLite `?` placeholders. Identifiers come only from validated macro attributes and are quoted. Inserts and updates use `RETURNING`, supported by sqlx’s bundled SQLite library. Rows are read column by column and deserialized into models. An empty PATCH reads the current row without updating it; a non-empty PATCH writes only changed columns.

## Query behavior (implemented)

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
4. With no changed columns, call `get`; otherwise, execute `UPDATE ... WHERE "id" = ... RETURNING`.
5. A missing row returns `NotFound`. There is no version field; the last write wins.

### Delete, Get and List

`DELETE` filters by the bound UUID. Zero affected rows produce `Missing`; `get` returns `Ok(None)` when no row exists. `list` orders by `"id"` and reads 101 rows, returning at most 100 objects and setting `truncated` if more exist. UUID ordering does not represent creation time. The generated REST `list` response is still not a page API. Bounded pages are `Model::query()`, described below.

## Setup

SQLite is the local standard. An installation and Docker are not necessary for this. sqlx brings SQLite. Schema changes for applications run through [migrations.md](migrations.md). `render_schema` remains the legacy bootstrap with `IF NOT EXISTS` for empty test databases. Migrations use `CREATE TABLE` without `IF NOT EXISTS`.

```text
sqlite://db.sqlite3?mode=rwc
sqlite::memory:
postgres://user:password@localhost:5432/ferrioxid
```

`sqlite::memory:` is a separate database for each connection. Ferrioxid limits SQLite pools to one connection so that tests and the application see the same database. The file `db.sqlite3` is created in the working directory and is contained in `.gitignore`. PostgreSQL remains a supported backend and is dialed via `DATABASE_URL`. `compose.yaml` can optionally start the server. Bootstrap only runs `CREATE TABLE IF NOT EXISTS`, does not delete existing rows and does not change existing tables. This is not a migration engine.

## Error and security (Implemented)

Deserialization errors and unknown SQL errors become `FerrioxidError::Database`. The HTTP layer then responds with 500 and a generic text. The database detail goes to the tracing log, not the answer. `DATABASE_URL` is not logged. `sqlx::ErrorKind::UniqueViolation` and `ForeignKeyViolation` are `Validation` and thus 422. SQLite reports `ON DELETE RESTRICT` as extended code `1811` with the fixed message `FOREIGN KEY constraint failed`. This will also be 422. Missing lines remain 404, unreleased routes 403. SQLite foreign keys are on. Models can generate unique indexes, secondary indexes and foreign keys. Relationship methods are in [relations.md](relations.md). `find_by_column` loads typed child lines. SQLite checks types weaker than PostgreSQL. Ferrioxid checks the supported values before writing in Rust.

## Composable queries

`Product::query()` returns a `Select<Product>`. Import `ModelQuery` so the method is in scope. The query accepts `&DatabaseConnection` and any other `&impl ConnectionTrait`, including an open `DatabaseTransaction`. It does not take a second connection from the pool.

`list` and `list_scoped` are unchanged.

```rust
use ferrioxid::core::QueryScope;
use ferrioxid::db::{any_of, col, not, Direction, ModelQuery};

// 1. Active products with stock greater than zero, ordered by price.
let available = Product::query()
    .filter(col("active").eq(true))
    .filter(col("stock").gt(0_i64))
    .order_by("price_cents", Direction::Asc)
    .all(&db)
    .await?;

// 2. The same order with the stable id tie-break appended by the query.
let by_price = Product::query()
    .order_by("price_cents", Direction::Asc)
    .order_by("name", Direction::Asc)
    .all(&db)
    .await?;

// 3. AND, OR and NOT. Values are bound parameters, not SQL text.
// The scope comes from the object policy; this shows an equality scope.
let matched = Product::query()
    .filter(col("active").eq(true))
    .filter(any_of([
        col("name").eq("bolt"),
        col("price_cents").lte(500_i64),
    ]))
    .filter(not(col("name").is_null()))
    .scope(QueryScope::Equals {
        column: "category_id",
        value: ferrioxid::core::ScopeValue::Uuid(category_id),
    })
    .all(&db)
    .await?;

// 4. Walk more than 100 rows. Each call returns at most 100.
let mut offset = 0_u64;
loop {
    let page = Product::query()
        .order_by("price_cents", Direction::Asc)
        .limit(40)
        .offset(offset)
        .page(&db)
        .await?;
    // `page.data` is this window. `page.total` counts the filtered rows.
    if !page.has_more {
        break;
    }
    offset += page.data.len() as u64;
}
```

`policy_scope` is a `QueryScope` from the object policy. The default scope is `QueryScope::All`, the same default as `list`. `QueryScope::None` matches no row. `QueryScope::Equals` is AND-ed with the filters. A query does not invent a policy.

| Call | Window | Result |
|---|---|---|
| `all` | `limit` and `offset` | At most `limit` models. Default limit 100. |
| `page` | same window, plus a count | `QueryPage` with `data`, `limit`, `offset`, `total`, `has_more`. |
| `count` | ignored | Filtered row count, including rows outside the page. |
| `exists` | ignored | Whether the filter matches at least one row. |

`count` and `exists` still reject an invalid `limit` or `offset` on that builder.

Supported comparisons are `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `is_in`, `is_not_in`, `is_null` and `is_not_null`. `filter` combines with AND. `all_of`, `any_of` and `not` nest AND, OR and NOT. An empty OR matches nothing. Boolean and UUID columns accept equality, inequality, `IN`, `NOT IN` and null checks. `i64`, text, date, time and datetime also accept ranges. A comparison whose value type does not match the column is `Validation`.

Column names must be columns of that model and must match `[A-Za-z_][A-Za-z0-9_]*`. Unknown or unsafe names are `Validation` and are not sent to the database. Values go through SeaQuery parameters (`?` on SQLite, `$1` on PostgreSQL). `limit` and `offset` are validated integers; SeaQuery may emit them as numeric literals.

Ordering can list several columns. If `id` is absent, the query appends `id ASC`. Pages of an unchanged filtered set do not overlap or skip when walked with a constant page size. An insert, delete or sort-key update between two requests can move offset pages. This is not keyset pagination and not a snapshot across requests. Read both pages inside one transaction when the set must stay still.

### Limits

- Page size is 1..=100 (`PAGE_LIMIT`). Offset is at most 1_000_000 (`OFFSET_LIMIT`).
- One `IN` or `NOT IN` list holds 1..=100 values (`IN_LIMIT`). An empty list is an error, not `IN ()`.
- `all` never scans the whole table. A result larger than 100 rows requires further pages.
- There is no `SUM`, grouping, annotation or arbitrary SQL expression.
- MySQL is rejected.
- SQLite behavior is covered by `cargo test -p ferrioxid-db`. PostgreSQL SQL generation is asserted in the same crate. `postgres_filters_and_pages_runtime` is `#[ignore]` and runs only with `DATABASE_URL` and a disposable PostgreSQL database. A local workspace run does not mark that test as passed.

## Not implemented (planned)

- `migrate down`. Explicit renames, SQLite table rebuilds and SQLite baselining are described in [migrations.md](migrations.md). Multi-model transactions are described in [transactions.md](transactions.md).
- Own error codes apart from the two fixed validation texts for Unique and Foreign Key.
- JSON columns, many-to-many relations, eager loading and MySQL.
- Pool sizes and timeouts as an application configuration except for the fixed SQLite limit of a connection.
- Optimistic locking.

## Tests

`cargo test --workspace` runs `sqlite_http_roundtrip` against `sqlite::memory:` without changing `db.sqlite3`. `postgres_http_roundtrip` remains `#[ignore]` and requires `DATABASE_URL` plus a running PostgreSQL server. `.github/workflows/ci.yml` provides a separate PostgreSQL job for the ignored tests; a local workspace test run does not start that server. See [testing.md](testing.md).

## Admin lists

`query_admin_page` in `crates/ferrioxid-db/src/sql.rs` uses the same SeaQuery layer. Column names come from the descriptor, values are parameters. The search is `LIKE` with `ESCAPE` and runs under SQLite. PostgreSQL gets the same expression, no dialect-specific input from the interface. The public list limit remains 100. The admin page is at most 100 lines, plus one line to detect `has_more`.
