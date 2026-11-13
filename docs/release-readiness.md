# Release readiness

Checked on 2026-11-12 from branch `work/api-quality` and extended on 2026-11-13 on `work/release-readiness`. Nothing was published to crates.io.

## Versions

| Name | Value | Meaning |
|---|---|---|
| Crate version | `0.1.0` | Cargo package version for every workspace member. Experimental. |
| HTTP contract | OpenAPI `info.version` = `1` | Generated list, problem JSON and read/write schemas described in [api.md](api.md). |
| Template version | unchanged | `fexu upgrade` does not rewrite an existing project to match a new crate. |

Before 1.0, a new `0.1.x` may change Rust APIs, migration history and HTTP fields. The HTTP document version moves only when a generated request or response changes in a way an existing client cannot ignore. This branch moved the list body from `truncated` to `next` and `has_more`. That is a breaking HTTP change inside contract version `1` because no stable crate has been published. Callers of the old list body need to read `has_more`.

## Crate graph and publication order

Nothing in this branch was uploaded. `cargo publish --dry-run` aborts before the upload. Examples `myshop`, `notes` and `desk` stay `publish = false`.

Internal dependencies keep a path for workspace builds and `version = "0.1.0"` for the manifest Cargo writes into a package. The version matches `[workspace.package]`.

Publish in this order. A crate is not publishable until every crate it depends on is already on crates.io.

1. `fexu-core`, `fexu-macros`, `fexu-cache`, `fexu-static`, `fexu-middleware`
2. `fexu-db` and `fexu-mail` after `fexu-core`
3. `fexu-migrations` after `fexu-db`
4. `fexu-auth` after migrations and mail
5. `fexu-storage` after auth, and `fexu-jobs` after auth
6. `fexu-forms` after storage
7. `fexu-cli` after jobs
8. `fexu-admin` after forms
9. `fexu-api` after admin
10. `fexu` after `fexu-cache` and every crate above

`fexu-storage` uses `fexu-db` only as a dev-dependency. `fexu-jobs` uses `fexu-auth` only as a dev-dependency. Verification still fetches those crates.

## Packaging and dry-run on 2026-11-13

Compiler: `rustc 1.98.1` and `cargo 1.98.1` on Windows. Edition is 2021. `rust-toolchain.toml` selects stable. No `rust-version` key is set. A minimum supported version was not bisected, so 1.98.1 is the compiler that ran these checks, not a measured minimum. Linux packaging was not run. Pull request 102's CI covers the parent commits on Windows and Linux, not this packaging commit.

`cargo package --list` included the crate sources, `README.md`, and for the admin crate the templates, `static/css/admin.css` and `static/js/admin.js`. One census invocation also passed `--allow-dirty` while the tree was already clean, so it did not change the file set. No `.env`, key or certificate was in those lists. `src/password.rs` and `migrations/0004_password_reset.json` are code and schema. They contain column names, not secret values. The SPDX license `MIT` is in each manifest. `LICENSE-MIT` stays at the repository root and is not copied into each crate archive.

| Crate | `cargo publish --dry-run` | Archive |
|---|---|---|
| `fexu-core` | Verified. Upload aborted because this was a dry run. | 12 files, 82.7KiB (23.6KiB compressed) |
| `fexu-macros` | Verified. Upload aborted. | 8 files, 111.5KiB (21.5KiB compressed) |
| `fexu-middleware` | Verified. Upload aborted. | 6 files, 37.5KiB (12.7KiB compressed) |
| `fexu-static` | First verify failed: `tokio::fs` is not in the workspace Tokio features, and an isolated package does not inherit features from other workspace members. After `features = ["fs"]` on that crate, verify passed and the upload was aborted. | 6 files, 36.1KiB (12.1KiB compressed) |
| `fexu-mail` | Passed after declaring Tokio `time`, which `tokio::time::timeout` needs in isolation. Upload aborted. | 12 files, 74.5KiB (21.6KiB compressed) |
| `fexu-db` | Failed before an archive existed. crates.io has no `fexu-core` 0.1.0. | Not produced |
| `fexu-cli` | Failed the same way. crates.io has no `fexu-auth` 0.1.0. | Not produced |
| `myshop` | `cargo publish` refuses it because `publish` is not true. `cargo package` also stops because the example dependency is path-only. | Not produced |

`fexu-jobs` now declares Tokio `time` for the same isolation reason. Its dry-run cannot succeed until `fexu-core`, `fexu-db`, `fexu-mail` and `fexu-migrations` are on crates.io. That failure was not re-run after the mail result, because the missing crates are still the cause.

`cargo install` from a packaged `fexu-cli` archive was not possible: the dry-run never wrote that archive. Install from a checkout remains `cargo install --path crates/fexu-cli`.

`--allow-dirty` was not used for the successful dry-runs. One earlier attempt stopped with "changes that were not yet committed" while the Tokio feature edit was uncommitted. That edit was committed, and the dry-run was repeated on a clean tree.

## Re-check on work/release-readiness-final

The same dry-run, without `--allow-dirty`, was repeated after the cherry-pick onto `26e79f2` and again after merging `5e7525c`. The table is the later measurement. Upload was aborted in every successful run.

| Crate | Result | Archive |
|---|---|---|
| `fexu-core` | Verified | 12 files, 88.5KiB (25.0KiB compressed) |
| `fexu-macros` | Verified | 8 files, 113.4KiB (21.8KiB compressed) |
| `fexu-cache` | Verified | 11 files, 46.1KiB (13.1KiB compressed) |
| `fexu-static` | Verified | 6 files, 36.1KiB (12.1KiB compressed) |
| `fexu-middleware` | Verified | 6 files, 42.1KiB (13.6KiB compressed) |
| `fexu-mail` | Stopped. crates.io has no `fexu-core` 0.1.0. | Not produced |

After the merge of `5e7525c` (#110), the dry-run was repeated. `fexu-cache` is the archive that changed, from 40.3KiB (12.2KiB compressed) to the size above. The other successful archives kept the sizes in the table. `fexu-mail` again stopped because crates.io has no `fexu-core` 0.1.0.

On that merged head, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace`, and `cargo metadata --locked` passed. The same commands passed again after merging `2619268` (#109). The dry-run sizes stayed as listed. The ignored PostgreSQL tests were left to the GitHub PostgreSQL job.

## Release blockers

- Do not run `cargo publish` without an explicit decision. Dry-run is not a publish.
- Crates that depend on other Fexu crates fail dry-run until those crates exist on crates.io, in the order above. On this base `fexu-mail` depends on `fexu-core`, so its dry-run now stops for that reason even though an earlier tree without that dependency verified.
- The SQLite patch below blocks publishing any crate whose graph includes `sqlx-sqlite`.
- MSRV is not measured. `libsqlite3-sys` 0.39 requires Rust 1.88 or newer. That floor is not a measured minimum for the whole workspace.
- The rename to Fexu is planned in [fexu-rename-plan.md](fexu-rename-plan.md) and was not performed. Publish the renamed crates once, not both names.
- This branch is `work/release-readiness-final`. It started from `origin/main` at `26e79f2` (`#101` squashed), cherry-picks only the three release commits from `work/release-readiness`, and then merges `origin/main` through `5e7525c` (`#110`) and `2619268` (`#109`). Pull requests #102 and #105 were not changed.

## SQLite patch for external users

The root `[patch.crates-io]` stays. This branch does not replace `sqlx-sqlite`, SeaORM, or SQLx.

A direct version bump is not sufficient. Published `sqlx-sqlite` 0.8.6 requires `libsqlite3-sys >=0.30.1, <0.38.0`. `fexu-db` requires `libsqlite3-sys` 0.39.1 with `bundled`. Both packages use `links = "sqlite3"`, so Cargo refuses to build them together. A checkout without the root patch fails resolution. It does not fall back to the vulnerable binding. Published `sqlx-sqlite` 0.9.0 still requires `>=0.30.1, <0.38.0`, so SeaORM 2 and SQLx 0.9 do not select the fixed binding and would change APIs. The upstream bound change at `10462fa` is not a published SQLx 0.8 release. Details are in [sqlite-rustsec-2026-0459.md](security/sqlite-rustsec-2026-0459.md).

Cargo applies `[patch]` only from the consuming workspace root. A published library cannot force that patch onto its users. The generator already writes the patch for a new project that points at this checkout. An existing project must add the same table itself.

The release path is:

1. Keep `vendor/sqlx-sqlite` and both patch stanzas until crates.io has a SQLx release that SeaORM can use and whose `sqlx-sqlite` accepts `libsqlite3-sys >=0.39.1, <0.40.0`.
2. Do not publish `fexu-db`, `fexu`, or another crate whose graph includes `sqlx-sqlite` before that release. Users of such a publish would have to vendor this checkout or the build would fail.
3. Remove the framework patch and the generator patch in the same change, then repeat the checks named in the SQLite security document: workspace tests, generated-project SQLite version, disposable PostgreSQL tests, and `cargo audit`. Keep the direct `libsqlite3-sys` 0.39.1 requirement until the upstream constraint enforces it.
4. Do not publish a forked `sqlx-sqlite` from this branch. That would be a new crate name, a provenance review, and a dependency switch. It is not required for step 1 and it is not done here.

## Install

Until a crate is published, install from a checkout. A dry-run of `fexu-cli` does not produce an installable archive, because `fexu-auth` is not on crates.io.

```powershell
cargo install --path crates/fexu-cli
```

```bash
cargo install --path crates/fexu-cli
```

The same command is the Linux and macOS form. `cargo install fexu-cli` is not a supported command yet because the crate is not on crates.io.

## Upgrade

There is no previous crates.io release and no git tag that represents a published `0.1.0`. An upgrade test from an older published crate cannot be run. `fexu upgrade` still only plans project-file changes and does not migrate a database. Schema changes stay `fexu migrate up`, run by hand. Rollback of a migration is `fexu migrate down` where that command already exists, on a disposable database.

## What this branch does not do

- It does not publish, tag or sign release artifacts.
- It does not label the project 1.0 or production-ready.
- It does not revert the squash of pull request 101. Pull requests 97–100 are already in that squash.
- It does not start the Compose stack when Docker is absent.
- It does not replace the vendored SQLite driver.

## Dependencies on pull requests 97–100

Those four pull requests are already in `main` through the squash of pull request 101. This branch does not merge them again and does not revert them. `FEXU_TRUSTED_PROXIES`, grouped migration planning, the audit timeline, and the project-file alias checks from that squash stay as `main` left them.

## PostgreSQL 18.6

The local binary install answered on `127.0.0.1:5432`. `psql` reported version 18.6. `pg_dump` and `pg_restore` are the same 18.6 build. No password was printed or stored.

The run created only databases named `fexu_test_` plus 32 hexadecimal digits. `fexu` stayed at 15 public tables. `fexu_migrations_down_test` stayed at 1 public table. After the run, no `fexu_test_*` database remained.

These ignored tests passed against that disposable catalog:

- `fresh_postgres_initializes_auth_jobs_and_application`
- `postgres_http_roundtrip`
- `postgres_migration_roundtrip`
- `postgres_enforces_cross_app_foreign_keys`
- `postgres_anonymous_session_roundtrip`
- `postgres_decimal_and_choice_roundtrip`
- `postgres_fixture_roundtrip`
- `postgres_reverts_backfill_and_foreign_key` (isolated databases, not `fexu_migrations_down_test`)
- `postgres_filters_and_pages_runtime`
- `postgres_transaction_query_and_rollback_runtime`
- `postgres_keyset_annotations_and_many_to_many`
- `postgres_text_search_relations_count_and_pages`
- `postgres_admin_search_runtime`
- `postgres_temporal_filters_and_keyset`
- `postgres_temporal_fixture_roundtrip`
- `postgres_isolated_databases_do_not_share_rows`
- `postgres_shop_cases`
- `postgres_write_security`
- `postgres_cursor_filters_and_openapi_describe_the_same_list`
- `postgres_dump_and_restore_roundtrip`
- `postgres_same_operations`

`postgres_dump_and_restore_roundtrip` wrote a custom-format dump and restored it with `pg_restore --no-owner` into a second disposable database. The test passed.

`/readyz` returned 200 and `"status":"ready"` while a disposable database was present. After that database was dropped, the same process returned 503.

Docker Compose was not started. Docker is not installed. Playwright was not launched on this Windows machine. The Ubuntu browser job remains the run for that spec. Nothing was published to crates.io.

## Pull request 101

`origin/main` is `26e79f2`, the squash of pull request 101. This branch starts there. The conflict list from `work/release-readiness` described an older unmerged integration branch and does not apply to this head. `FEXU_TRUSTED_PROXIES` and the `FEXU_*` aliases from that squash stay in place.

## Security follow-up on work/integration (2026-11-13)

Pull request 101 now contains the merged work from pull requests 97–104. The notes above describe the earlier `work/api-quality` snapshot. They are not the current integration head.

`FEXU_MFA_KEY` is required in production. The encoding is 64 hexadecimal characters and the value is 32 bytes. It is not derived from `FEXU_CSRF_SECRET`. Existing development checkouts keep working: when the variable is unset and the process is not in production, Fexu reads or creates `.fexu/mfa.key`. Generated projects ignore that file. `fexu check --deploy` reports the variable's presence and shape and does not print it.

A deployment that already stored confirmed TOTP under the old CSRF-derived key can still open that ciphertext. The next successful open re-seals it with the MFA key. Unused recovery codes from that enrollment stay HMAC hashes of the old key. Keep the current CSRF secret until those codes are used. New enrollments do not need the CSRF secret for TOTP or recovery codes.

`RUSTSEC-2023-0071` is still present because `Cargo.lock` records optional `sqlx-mysql`. It is not in the enabled build graph. No feature change removes it without leaving the SQLx 0.8.6 line that the vendored SQLite patch requires. The audit ignore stays limited to that one advisory. See [security-rsa.md](security-rsa.md) and [auth-threat-model.md](auth-threat-model.md).

`AuthConfig::development` draws one random 32-byte MFA key for the process. Session tokens are SHA-256 lookup hashes of 32 random bytes. Passwords are Argon2id. Finishing a pending second factor is `complete_pending_login`, and that result is a session. CodeQL classifies a method name containing the delimited word `mfa` as a password source, so this method name describes the session it returns.

Nothing in this follow-up was published to crates.io. The project was not renamed.
