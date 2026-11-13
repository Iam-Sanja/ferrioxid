# SQLite security: RUSTSEC-2026-0459

RUSTSEC-2026-0459 covers SQLite builds before 3.54.1. The old Fexu graph reached that code through `sqlx-sqlite` 0.8.6 and `libsqlite3-sys` 0.30.1.

The current graph does not use `sqlx-sqlite`. SQLite is rusqlite 0.42.0 with its bundled `libsqlite3-sys` 0.40.0, which bundles SQLite 3.54.1. Runtime tests reject a `sqlite_version()` older than 3.54.1. PostgreSQL does not link this crate.

There is no `[patch.crates-io]` table and no `vendor/sqlx-sqlite` tree. A downstream project does not add a patch to build Fexu. Adding published `sqlx-sqlite` 0.8.6 or 0.9.0 next to this workspace fails resolution because those crates also set `links = "sqlite3"` and require an older `libsqlite3-sys`.

The earlier backport under `vendor/sqlx-sqlite` was removed in the database driver change. Its history remains in git. The driver layout and the crates.io consequence are in [database-driver.md](../database-driver.md).
