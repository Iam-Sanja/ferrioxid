# SQLite security: RUSTSEC-2026-0459

[Overview](../index.md) · [Issue #40](https://github.com/Iam-Sanja/ferrioxid/issues/40) · [Vendored-source provenance](../../vendor/README.md)

## Finding and dependency chain

The original resolved graph is `SeaORM 1.1.20 -> SQLx 0.8.6 -> sqlx-sqlite 0.8.6 -> libsqlite3-sys 0.30.1`. SQLx enables bundled SQLite. This is an active SQLite dependency, unlike the unrelated optional MySQL/RSA lockfile finding.

[RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459) / CVE-2026-119375 concerns a database and sibling WAL file with different page sizes. Queries can expose uninitialized process memory if a less trusted party can supply those files. SQLite before 3.54.0 is affected. This is a real dependency vulnerability, not a false positive; application authentication does not fix the native-library flaw.

## Compatible remediation

Keep the SeaORM 1.1 / SQLx 0.8 APIs and PostgreSQL driver. A root `[patch.crates-io]` selects the complete published `sqlx-sqlite 0.8.6` source under `vendor/sqlx-sqlite/`. Its Rust source and features are unchanged; only its `libsqlite3-sys` bound becomes `>=0.39.1, <0.40.0`. Cargo resolves the genuine registry `libsqlite3-sys 0.39.1`, which bundles patched SQLite 3.54.1. `cargo update -p libsqlite3-sys --precise 0.39.1` generated the lockfile changes.

The upstream SQLx [bound update](https://github.com/transact-rs/sqlx/commit/10462fa4414c6c7f1ed1d39bb31678e3b726878c) accepts the 0.39 series. This backport deliberately raises the lower bound as well, so an affected version cannot be selected. The published `sqlx-sqlite 0.9.0` manifest still requires `>=0.30.1, <0.38.0`; upgrading to SeaORM 2 / SQLx 0.9 alone would introduce API changes without resolving the advisory. The upstream development branch's bound change is not a published SQLx 0.8 security release.

`fexu-db` also requires `libsqlite3-sys ^0.39.1` with `bundled`. This propagates the safe minimum to every consumer and makes omitted root-level driver patches a dependency-resolution error. Cargo's single `links = "sqlite3"` requirement prevents simultaneously using a second, old SQLite binding. A direct pin alone cannot repair SQLx's incompatible constraint.

SQLite bindings 0.39 require Rust 1.88 or newer; the full workspace is validated with the configured stable toolchain, not claimed to support Rust 1.88 as a whole. SeaORM and SQLx themselves remain unchanged. The bundled build requires a C compiler, as before. No system SQLite/SQLCipher substitution, version relabeling, build-environment bypass or audit ignore is part of this fix.

## Independent projects

Cargo honors `[patch]` only in the consuming workspace/project root. New API/fullstack projects therefore receive a patch pointing at the selected framework checkout. Existing projects are not edited automatically.

For an existing independent project, add this to its root `Cargo.toml`, adjusting the path to this fixed Fexu checkout:

```toml
[patch.crates-io]
sqlx-sqlite = { path = "../fexu/vendor/sqlx-sqlite" }
```

For a larger Cargo workspace, put it in that workspace's root manifest. Merge it with any existing patch table rather than adding a duplicate table. Then run:

```sh
cargo update -p libsqlite3-sys --precise 0.39.1
cargo tree -i libsqlite3-sys
cargo test
cargo audit --ignore RUSTSEC-2023-0071
```

The RSA exception above is the repository's existing narrowly guarded exception, not an instruction to suppress other advisories. In other projects, reassess whether that exception applies at all.

Verify that the tree contains `libsqlite3-sys 0.39.1` or a later reviewed fixed version and the selected vendored driver. Keep the vendor directory with the framework checkout. Existing applications must be rebuilt and redeployed; an already running executable still contains its old SQLite engine. Keep normal deployment backups. This update does not delete, reset, baseline, rename or reseal any database or migration.

## Validation and maintenance

The database regression test checks `SELECT sqlite_version()` through the actual SeaORM/SQLx connection, requires at least SQLite 3.54.1, and checks WAL reads, transaction rollback, persistent rows and migration-history checksums across reopen. The API/fullstack generator integration tests assert the loaded SQLite version in projects outside the framework workspace. Existing migration, authentication, admin, jobs and PostgreSQL tests remain enabled.

Run formatting, Clippy, the full workspace suite, generator release builds, the three ignored PostgreSQL tests against a disposable database, and the unmodified security workflow. The unfiltered audit must show no RUSTSEC-2026-0459 finding; the existing RSA reachability guard and audit gate remain unchanged. An explicitly overridden system SQLite or SQLCipher build needs separate security assessment and is not validated by the bundled-library guarantee.

The vendored driver is excluded from workspace membership, like a normal dependency; it is still built and used by all SQLite integration tests. Its original licenses remain in its directory. [Provenance and removal conditions](../../vendor/README.md) document the archive checksum and upstream commits. Review/remove this backport when an official compatible SQLx release accepts the fixed binding. Remove the framework and generator patch stanzas together only after the runtime, consumer-resolution, data-preservation and audit checks pass. Do not remove the safe minimum before the upstream dependency constraints enforce it.

The runtime-version checks detect dependency/build regressions; they do not claim exhaustive fuzzing of SQLite or reproduction of every malformed WAL exploit. Published SQLite source fixes and RustSec version ranges provide the vulnerability assessment.

## Local results (2026-11-09)

Validated on Windows with Rust/Cargo 1.98.1 and PostgreSQL 16.15. Linux and Windows GitHub checks run on the PR using the unchanged workflows.

| Check | Result |
|---|---|
| Format / Clippy `--workspace --all-targets -- -D warnings` / locked workspace build | Passed |
| `cargo test --workspace` | 185 passed; 0 failed; 3 PostgreSQL tests ignored in the normal suite |
| Three ignored PostgreSQL CI tests, explicitly selected against a fresh disposable database | All passed |
| API/fullstack generated projects | Compile, runtime SQLite >=3.54.1, app/model migrations, HTTP, dev, Clippy and release builds passed |
| Generated API/fullstack lockfile audit gates | Both passed |
| SQLite regression | WAL reads, rollback, rows and history checksum preserved across reopen |
| Existing database created with SQLite 3.49.1 | Two updated-engine migration runs preserved rows/history; `PRAGMA integrity_check` returned `ok` |
| Consumer without the root patch | Expected SQLite `links`/version resolution failure, rather than vulnerable fallback |
| CLI installation from the checkout | `cargo install --path crates/fexu-cli --debug --locked` passed in an isolated install root |
| Vendor archive comparison | All 55 non-manifest files byte-identical; only the documented dependency stanza differs |
| Current RustSec DB `899077d9d471b9fd3dbf4b2faea8ef26e83692e8` | Original lockfile reports SQLite + RSA; updated lockfile reports only the pre-existing optional RSA entry |
| Existing RSA graph guard / audit gate | RSA absent on all enabled targets; existing gate passes with no new ignore and no advisory warnings |

The normal Windows linker emits its pre-existing import-library informational warning. No source warning, test or security check was disabled. The disposable PostgreSQL server was stopped after validation; the external Shop project and other PR branches were not changed.

Framework changes pass `git diff --check`. The archived upstream `src/connection/serialize.rs` retains three existing rustdoc Markdown hard-break lines with trailing spaces; these are documented upstream formatting, not security changes, and preserving them keeps the source comparison byte-identical.
