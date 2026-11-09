# SQLx SQLite security backport

`sqlx-sqlite/` is the complete published `sqlx-sqlite 0.8.6` source archive. Its Rust source, features and dual MIT/Apache-2.0 licenses are unchanged. Only the normalized `Cargo.toml` dependency constraint for `libsqlite3-sys` is changed from `0.30.1` to `>=0.39.1, <0.40.0`. `Cargo.toml.orig` is retained as upstream provenance, not the active manifest.

- Archive: https://static.crates.io/crates/sqlx-sqlite/sqlx-sqlite-0.8.6.crate
- SHA-256: `c2d12fe70b2c1b4401038055f90f151b78208de1f9f89a7dbfd41587a10c3eea`
- Original VCS commit: `bab1b022bd56a64f9a08b46b36b97c5cff19d77e`
- Upstream bound change: https://github.com/transact-rs/sqlx/commit/10462fa4414c6c7f1ed1d39bb31678e3b726878c
- Advisory: https://rustsec.org/advisories/RUSTSEC-2026-0459
- Maintenance and consumer instructions: [SQLite security](../docs/security/sqlite-rustsec-2026-0459.md)

This is a dependency-bound backport, not a replacement SQLite implementation. Cargo resolves the real registry `libsqlite3-sys 0.39.1` with bundled SQLite 3.54.1. No dependency is relabeled and no advisory is suppressed. The lower bound deliberately excludes all affected versions; the upper bound requires a review before the next bindings series.

To verify provenance, download the archive, verify its SHA-256, extract it and compare every file. Only the above manifest stanza may differ. Do not edit the vendored Rust source when updating framework formatting. Keep both upstream license files.

Remove this backport only when a published SQLx/SeaORM combination accepts patched SQLite, and after the workspace, generated projects, runtime version tests, PostgreSQL tests and audit pass without it. Remove the root and generated-project patch stanzas together. Existing external projects need their root patch removed/updated explicitly.
