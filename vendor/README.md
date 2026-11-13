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

## Publishing status on 2026-11-13

No crates.io release accepts `libsqlite3-sys >=0.39.1`. The latest published crates are SeaORM 1.1.20 (`sqlx ^0.8.4`) and SeaORM 2.0.4 (`sqlx ^0.9.0`), SQLx/sqlx-sqlite 0.8.6 and 0.9.0, and `libsqlite3-sys` 0.39.1 / 0.40.0. Published `sqlx-sqlite` 0.8.6 requires `libsqlite3-sys` version `0.30.1` (Cargo caret `^0.30.1`, so `<0.31.0`). Published `sqlx-sqlite` 0.9.0 requires `>=0.30.1, <0.38.0`. Both ranges exclude 0.39.1.

`libsqlite3-sys` 0.39.1 bundles SQLite 3.54.1. `libsqlite3-sys` 0.40.0, published 2026-11-11 with rusqlite 0.42.0, also bundles SQLite 3.54.1. This backport does not adopt 0.40.0: unpublished SQLx `main` and `release/0.9.1` still cap the binding at `<0.40.0`, and 0.40 is a new bindings series.

Commit [`10462fa`](https://github.com/launchbadge/sqlx/commit/10462fa4414c6c7f1ed1d39bb31678e3b726878c) (`#4502`, merged 2026-11-08 onto `main`) only changes the unpublished 0.9 line from `<0.39.0` to `<0.40.0`. The lower bound stays `>=0.30.1`, so that commit still allows vulnerable releases. Branch `release/0.9.1` contains that commit and `version = "0.9.1"`, but crates.io has no `0.9.1` and there is no `v0.9.1` tag. A git revision is not a crates.io dependency. Cargo does not publish `[patch]`, and this repository does not publish a renamed or renumbered `sqlx-sqlite`.

[`sqlx-0.8.6-rustsec-2026-0459.patch`](sqlx-0.8.6-rustsec-2026-0459.patch) is the same bound change against SQLx tag `v0.8.6` (`sqlx-sqlite/Cargo.toml`). SeaORM 1.1.20 can consume an official `sqlx` 0.8.7 that contains it without an API migration. The vendored tree is that patch applied to the published 0.8.6 archive. The dependency plan is in [SQLite security](../docs/security/sqlite-rustsec-2026-0459.md).
