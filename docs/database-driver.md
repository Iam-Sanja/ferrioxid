# Database driver

Fexu generates SQL with sea-query 0.32.7. PostgreSQL and SQLite each have one execution adapter. The public names under `fexu::sea_orm` are `fexu_db::driver`. That module is not the `sea-orm` crate.

| Piece | Crate | Where it is used |
| --- | --- | --- |
| SQL | sea-query 0.32.7 | `crates/fexu-db/src/sql.rs` and the query builders |
| PostgreSQL | sqlx-postgres 0.8.6 and sqlx-core 0.8.6 | `crates/fexu-db/src/postgres.rs` only |
| SQLite | rusqlite 0.42.0, bundled, which links libsqlite3-sys 0.40.0 | `crates/fexu-db/src/sqlite.rs` |
| Connection, transaction, rows | this repository | `crates/fexu-db/src/database.rs`, `rows.rs`, `error.rs` |

The workspace does not depend on the `sqlx` facade, `sqlx-sqlite`, `sea-orm`, or `sea-query-binder`. `libsqlite3-sys` appears once, at 0.40.0. Published `sqlx-sqlite` 0.8.6 cannot be added beside this graph: both crates set `links = "sqlite3"` and their version ranges do not overlap.

## PostgreSQL adapter

`postgres.rs` is the only module allowed to name `sqlx_postgres` or `sqlx_core`. It uses the published items `PgPool`, `PgPoolOptions`, `PgConnectOptions`, `PgSslMode`, `PgArguments` through the `Arguments` trait, `query_with`, `PgRow`, `TypeInfo::name`, `Column::name`, `ErrorKind`, and `Transaction::commit` / `rollback`. It does not use the query macros or the SQLite driver.

`sqlx-postgres` and `sqlx-core` say they are not the supported application API. The exact versions are pinned with `=0.8.6`. A later driver release is an edit to this one module and its tests, not a workspace-wide SeaORM migration. Do not enable the `sqlx` facade features `json`, `chrono`, `uuid`, or `time`: those features also activate `sqlx-sqlite` and the resolver then rejects the build.

`PgTransaction` in 0.8.6 does not provide an `Executor` implementation that a higher-ranked async trait can call with `raw_sql`. Unprepared PostgreSQL text therefore goes through `query_with` and an empty argument list. That runs one statement. SQLite `execute_unprepared` still uses `execute_batch` and can run several statements. The pool does not set `statement_timeout`, so a migration is not cancelled by an idle or statement limit. Acquire timeout is 30 seconds and idle timeout is 10 minutes. `sslmode` is `Prefer`: a server without SSL accepts a clear connection. That mode is not a test of TLS.

## SQLite adapter

Each session owns one rusqlite connection on a thread named `fexu-sqlite`. Tokio sends the work and waits. rusqlite is not called on a Tokio worker. The session sets `PRAGMA foreign_keys = ON`, a 5 second busy timeout, and WAL for a file database. `connect` uses one SQLite connection so an in-memory database stays one database. A memory URL with a larger pool is rejected.

A failed rollback drops that connection and opens a new one on the same file. The failed transaction is aborted with the connection. If the new open fails, the session stores the error and later calls return it until the process creates another session. `close` sends shutdown, rolls back an open transaction, drops the connection, and joins the thread. Drop from another thread also joins, so the WAL checkpoint finishes before the caller reads the file. Drop on the SQLite thread only queues shutdown, because joining that same thread would deadlock. The cleanup job stores its own failure before it releases the last session; a shutdown error that remains after `Drop` is written to stderr, since `Drop` has no caller to return it to.

`Value::Uuid` is bound as a 16-byte blob, matching the previous model queries. Callers that store a UUID as text pass a string. `Value::Decimal` is rejected: Fexu decimal columns are 16-byte blobs on SQLite and `bytea` on PostgreSQL. `Value::Decimal` is only the PostgreSQL `NUMERIC` bind. `render_create_table` still emits `blob` / `bytea` for `ColumnKind::Decimal` and does not emit `numeric`.

## crates.io

`cargo publish --dry-run --allow-dirty` packaged these crates and then aborted the upload: `fexu-core`, `fexu-macros`, `fexu-cache`, `fexu-static`, and `fexu-middleware`. They do not depend on another Fexu crate.

`fexu-mail` and `fexu-db` stop with `no matching package named fexu-core` because that crate is not on crates.io yet. The same stop applies to every other Fexu crate that depends on a sibling. The dry-run does not stop because a consumer would need `vendor/sqlx-sqlite`. Nothing in this change publishes a crate.
