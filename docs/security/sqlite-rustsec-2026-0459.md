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

## Active graph

With the root patch, `cargo tree -i libsqlite3-sys --locked` and `cargo tree -i sqlx-postgres --locked -p fexu-db` resolve:

| Edge | Resolved crate | Requirement |
|---|---|---|
| Direct | `fexu-db` → `libsqlite3-sys` 0.39.1, `bundled` | `^0.39.1` |
| SQLite | SeaORM 1.1.20 → `sqlx` 0.8.6 → vendored `sqlx-sqlite` 0.8.6 → `libsqlite3-sys` 0.39.1 | patched `>=0.39.1, <0.40.0` |
| SQLite binder | `sea-query-binder` 0.7.0 → the same `sqlx` 0.8.6 | `sqlx-sqlite` feature |
| PostgreSQL | SeaORM 1.1.20 → `sqlx` 0.8.6 → registry `sqlx-postgres` 0.8.6 | no `libsqlite3-sys` |

`libsqlite3-sys` 0.39.1 bundles SQLite 3.54.1. The runtime test requires `sqlite_version()` ≥ 3.54.1. PostgreSQL does not link this crate, but the default Fexu features enable both drivers, so a default build still resolves SQLite.

## Crates.io recheck on 2026-11-13

Queried from the crates.io API and the launchbadge/sqlx and rusqlite tags. Nothing below is a git dependency.

| Crate | Latest published | `libsqlite3-sys` requirement | Usable for Fexu 0.1.0 |
|---|---|---|---|
| `sqlx-sqlite` 0.8.6 | 2025-05-19, still current on the 0.8 line | `0.30.1` (`^0.30.1`, `<0.31.0`) | No. Empty intersection with `^0.39.1`. |
| `sqlx` / `sqlx-sqlite` 0.9.0 | 2026-05-21, latest crates.io release | `>=0.30.1, <0.38.0` | No. Excludes 0.39 and 0.40. |
| SeaORM 1.1.20 | latest 1.1, 2026-03-31 | optional `sqlx ^0.8.4` | Keeps the 0.8 driver. No newer 1.1 exists. |
| SeaORM 2.0.4 | latest stable, 2026-09-27 | optional `sqlx ^0.9.0` | Pulls SQLx 0.9.0, which still rejects 0.39.1, and changes the ORM API. |
| `libsqlite3-sys` 0.39.1 | 2026-10-25 | bundles SQLite 3.54.1 | The reviewed fixed binding. |
| `libsqlite3-sys` 0.40.0 | 2026-11-11, rusqlite 0.42.0 | bundles SQLite 3.54.1 | Patched SQLite, but a new bindings series. Unpublished SQLx still caps at `<0.40.0`. Not adopted. |

Unpublished and therefore not a crates.io solution:

- [`10462fa`](https://github.com/launchbadge/sqlx/commit/10462fa4414c6c7f1ed1d39bb31678e3b726878c) raises only the upper bound on `main`, from `<0.39.0` to `<0.40.0`. The lower bound remains `>=0.30.1`.
- Branch `release/0.9.1` sets `version = "0.9.1"` and includes that commit. crates.io still serves 0.9.0. There is no `v0.9.1` tag. The branch is 57 commits ahead of `v0.9.0`.
- No `sqlx` 0.8.7 and no 0.8 release branch contain the bound change.

Cargo applies `[patch]` only in the root manifest of the package being built. A published Fexu crate cannot carry `vendor/sqlx-sqlite` into a downstream build. Publishing this repository's patched sources as `sqlx-sqlite` 0.8.6 or 0.8.7 would impersonate the upstream crate. That is not done.

### Upstream dependency plan

1. Preferred release: SQLx publishes `0.8.7` with [`sqlx-0.8.6-rustsec-2026-0459.patch`](../../vendor/sqlx-0.8.6-rustsec-2026-0459.patch) applied to tag `v0.8.6`. SeaORM 1.1.20 already accepts `sqlx ^0.8.4`. The patch raises the floor to `>=0.39.1`, so the driver itself cannot select SQLite before 3.54.0, and stops before `0.40.0`.
2. If the 0.8 patch release only widens the range to `>=0.30.1, <0.40.0`, Fexu's direct `libsqlite3-sys ^0.39.1` dependency still forces the intersection `>=0.39.1, <0.40.0`. Keep that direct dependency until the upstream floor itself excludes the vulnerable releases.
3. After either release is on crates.io, delete the root patch, the generator patch, and `vendor/sqlx-sqlite` in one change. Re-run the workspace tests, a generated project with no patch, the disposable PostgreSQL tests, and `cargo audit`.
4. SQLx 0.9.1, once actually published, still allows `libsqlite3-sys` 0.30.1. Using it also requires SeaORM 2. That is a later migration, not this publishing fix.
5. Do not add the git revision as a dependency. `cargo publish` rejects git dependencies, and the revision is not a release.

### Feature gating

SeaORM's `sqlx-sqlite` feature is optional. An isolated SeaORM 1.1.20 crate with only `sqlx-postgres` and `runtime-tokio-rustls` resolves `sqlx` 0.8.6 and `sqlx-postgres` 0.8.6. `cargo tree -i libsqlite3-sys` and `cargo tree -i sqlx-sqlite` find neither package. That does not make default Fexu installable. The workspace enables `sqlx-sqlite` and `sqlx-postgres` together, `fexu-db` depends on `libsqlite3-sys` 0.39.1 directly, and SQLite statement builders are compiled into `fexu-db` unconditionally. Leaving SQLite in the default feature set preserves today's behavior and still fails resolution for `cargo add fexu`. Removing it from the default would drop the default database. This branch does not add that feature gate.

A downstream project outside this workspace that path-depends on `crates/fexu`, with no `[patch]` and no `vendor/sqlx-sqlite` dependency, fails `cargo metadata`. Cargo selects registry `sqlx-sqlite` 0.8.6, whose `libsqlite3-sys` requirement `^0.30.1` only matches 0.30.1, and that candidate conflicts with `fexu-db`'s `libsqlite3-sys` 0.39.1 because both use `links = "sqlite3"`. `cargo check` and `cargo build` do not start. The resolver does not select 0.30.1. The same failure is what a crates.io consumer would see after the Fexu crates themselves are published. Until then, `cargo publish --dry-run` for any crate that depends on another unpublished Fexu crate stops first because that dependency is absent from crates.io.

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

## Publishing recheck on 2026-11-13

Windows, Rust/Cargo 1.98.1, PostgreSQL 18.6. RustSec database `4ef2df8e806a65e2f1ecc14f56f51249062b23d6`. Linux was not executed on this machine. The unchanged GitHub workflows remain the Linux run. Nothing was uploaded to crates.io.

`cargo test --workspace` passed: 365 tests, 0 failed, 32 ignored. The ignored PostgreSQL tests from the CI job, plus `postgres_account_security_roundtrip` and `postgres_job_claim_and_rollback`, passed against new `fexu_test_<32 hex>` databases. `ferrioxid` stayed at 15 public tables, `ferrioxid_migrations_down_test` stayed at 1, and `postgres` stayed at 0. Those catalogs and the pre-existing `ferrioxid_test_*` databases were not dropped. No `fexu_test_*` database remained.

`cargo tree --workspace --target all --prefix none --locked -i rsa` and the same command for `sqlx-mysql` print nothing. Unfiltered `cargo audit` reports only `RUSTSEC-2023-0071`. `cargo audit --ignore RUSTSEC-2023-0071` passes. `RUSTSEC-2026-0459` is absent. The runtime SQLite check in the workspace suite still requires at least 3.54.1.

An isolated SeaORM 1.1.20 crate with only `sqlx-postgres` and `runtime-tokio-rustls` compiles. Its tree contains `sqlx-postgres` 0.8.6 and neither `sqlx-sqlite` nor `libsqlite3-sys`.

Generated API and fullstack projects outside this workspace still receive the root patch. With that patch, `cargo metadata`, `cargo check`, `cargo build`, and a runtime query report `sqlite_version=3.54.1`. `cargo tree -i libsqlite3-sys` shows 0.39.1 through vendored `sqlx-sqlite` 0.8.6. Their audits match the workspace: only `RUSTSEC-2023-0071`, and the gated audit passes. Removing the patch makes `cargo metadata` fail on `links = "sqlite3"` between `libsqlite3-sys` 0.30.1 and 0.39.1. `cargo check` and `cargo build` do not start, so there is no patched runtime without the patch. That is the crates.io-equivalent result. The patched generator run is not a substitute for it.

`cargo publish --dry-run` verifies and then aborts the upload for `fexu-core`, `fexu-macros`, `fexu-cache`, `fexu-static`, and `fexu-middleware`. Every later crate stops because a Fexu dependency is not on crates.io. `fexu-db` stops on missing `fexu-core` 0.1.0 before Cargo can show the SQLx patch gap. The external no-patch project is the evidence for that gap. Examples stay unpublished.
