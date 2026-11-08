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

## SQLite

`ferrioxid_db::connect` sets `max_connections(1)` for SQLite. Requesting another connection from that pool while a write transaction is open will wait indefinitely. Hash passwords, validate forms and run the parent’s `ensure_relations` before `begin`. Inside the transaction, use only methods that accept the transaction connection.

## PostgreSQL

The interfaces are `ConnectionTrait` and `DatabaseTransaction`. They do not contain SQLite-SQL. The PostgreSQL runtime tests `postgres_migration_roundtrip` and `postgres_http_roundtrip` remain ignored as long as no server is reachable. A green workspace run is not a PostgreSQL proof.

## Errors

A database error becomes `FerrioxidError::Database`. SQL text does not go into the HTML response. `conclude` maps the commit error to the transferred mapper. A failed partial operation does not generate a `303`.
