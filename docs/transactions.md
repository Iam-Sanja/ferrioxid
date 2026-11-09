# Transactions

[Overview](index.md) · Database: [database.md](database.md) Directory: [admin-directory-audit.md](admin-directory-audit.md) Parent-child forms: [admin-transactional-inlines.md](admin-transactional-inlines.md)

**Milestone:** v0.20. The Cargo version remains `0.1.0`.

A write operation that changes several rows uses a `DatabaseTransaction`. `ferrioxid_db::conclude` commits only on `Ok` and rolls back on `Err`. There is no separate transaction engine.

## Limitations

- Admin model CRUD through `ManagedModel::create`, `update` and `remove` opens a transaction and writes its audit entry there. Do not call these methods inside another transaction.
- `ManagedModel::read_in`, `create_in`, `update_in` and `delete_in` run on the transferred transaction. They do not write an audit and do not open another transaction.
- Auth methods with the suffix `_on` take `&impl ConnectionTrait`. They check the actor and the target state on this connection and do not commit.
- The directory in `ferrioxid-admin` writes auth changes and audit entries in the same transaction. See [admin-directory-audit.md](admin-directory-audit.md).
- A form with `form` in `admin_inline` stores parent object, children and audit in one transaction. See [admin-transactional-inlines.md](admin-transactional-inlines.md).

`query_admin_page` takes `&impl ConnectionTrait`. A child list within the write transaction uses the same connection.

`Product::query()`, `read_model`, `load_by_ids`, `load_children` and `find_by_column` also take `&impl ConnectionTrait`. Pass the open transaction. Do not call `list`, `get`, `create`, `update` or `delete` there: those methods take `&DatabaseConnection` and would check out another pool connection. On SQLite that wait does not end. Use `get_tx`, `create_tx`, `update_tx`, `delete_tx`, or the `*_in` methods.

```rust
use ferrioxid::db::{col, load_by_ids, ModelQuery};

// 6. Read, insert and update share the order transaction.
let tx = db.begin().await?;
let available = Product::query()
    .filter(col("active").eq(true))
    .filter(col("stock").gt(0_i64))
    .all(&tx)
    .await?;
let product_ids: Vec<_> = available.iter().map(|product| product.id).collect();
let products = load_by_ids::<Product>(&tx, &product_ids, &scope).await?;
let order = Order::create_tx(&tx, order_input).await?;
OrderLine::create_tx(&tx, line_input).await?;
let _ = (order, products);
tx.commit().await?;
```

`scope` is the policy scope. A rollback drops the insert and the update. The SQLite test `sqlite_transaction_reads_writes_and_rolls_back_on_one_connection` opens a pool with one connection, runs a filter, `find_by_column`, an insert and an update on that transaction, then rolls back. It fails if the work is still running after five seconds, which is the deadlock from a second checkout. `postgres_transaction_query_and_rollback_runtime` is ignored unless `DATABASE_URL` points at a disposable PostgreSQL database. The local workspace run does not mark it passed.

## SQLite

`ferrioxid_db::connect` sets `max_connections(1)` for SQLite. Requesting another connection from that pool while a write transaction is open will wait indefinitely. Hash passwords, validate forms and run the parent’s `ensure_relations` before `begin`. Inside the transaction, use only methods that accept the transaction connection.

## PostgreSQL

The interfaces are `ConnectionTrait` and `DatabaseTransaction`. They do not contain SQLite-SQL. The PostgreSQL runtime tests `postgres_migration_roundtrip` and `postgres_http_roundtrip` remain ignored as long as no server is reachable. A green workspace run is not a PostgreSQL proof.

## Errors

A database error becomes `FerrioxidError::Database`. SQL text does not go into the HTML response. `conclude` maps the commit error to the transferred mapper. A failed partial operation does not generate a `303`.
