# Transaktionen

[For overview](index.md) · Database: [database.md](database.md)] Directory: [admin-directory-audit.md](admin-directory-audit.md)] Parent-child forms: [admin-transactional-inlines.md](admin-transactional-inlines.md)]

**Stand:** v0.20. The cargo version remains `0.1.0`.

Eine Schreiboperation, die mehrere Zeilen ändern soll, benutzt eine `DatabaseTransaction`. `ferrioxid_db::conclude` committet nur bei `Ok` und rollt bei `Err` zurück. Es gibt keine zweite Transaktions-Engine.

## Grenze

- Model CRUD via `ModelRepository::create`, `update` and `delete` opens the own transaction and writes the model audit therein. These methods must not be invoked as long as a transaction is already open.
- `ManagedModel::read_in`, `create_in`, `update_in` and `delete_in` run on the transferred transaction. They do not write an audit and do not open another transaction.
- Auth methods with the suffix `_on` take `&impl ConnectionTrait`. They check the actor and the target state on this connection and do not commit.
- The directory in `ferrioxid-admin` writes authenticity change and audit line in the same transaction. See [admin-directory-audit.md](admin-directory-audit.md)].
- A form with `form` in `admin_inline` stores parent object, children and audit in one transaction. See [admin-transactional-inlines.md](admin-transactional-inlines.md)].

`query_admin_page` takes `&impl ConnectionTrait`. A child list within the write transaction uses the same connection.

## SQLite

`ferrioxid_db::connect` sets for SQLite `max_connections(1)`. A second query requesting a connection from the same pool waits for the open write transaction and is stuck. Before `begin` therefore belong password hashes, form checking and `ensure_relations` of the parent object. Within the transaction only methods that accept the transaction itself.

## PostgreSQL

The interfaces are `ConnectionTrait` and `DatabaseTransaction`. They do not contain SQLite-SQL. The PostgreSQL runtime tests `postgres_migration_roundtrip` and `postgres_http_roundtrip` remain ignored as long as no server is reachable. A green workspace run is not a PostgreSQL proof.

## Fehler

A database error becomes `FerrioxidError::Database`. SQL text does not go into the HTML response. `conclude` maps the commit error to the transferred mapper. A failed partial operation does not generate a `303`.
