# Release readiness

Checked on 2026-11-12 from branch `work/api-quality` and extended on 2026-11-13 on `work/release-readiness`. Nothing was published to crates.io.

## Versions

| Name | Value | Meaning |
|---|---|---|
| Crate version | `0.1.0` | Cargo package version for every workspace member. Experimental. |
| HTTP contract | OpenAPI `info.version` = `1` | Generated list, problem JSON and read/write schemas described in [api.md](api.md). |
| Template version | unchanged | `ferrioxid upgrade` does not rewrite an existing project to match a new crate. |

Before 1.0, a new `0.1.x` may change Rust APIs, migration history and HTTP fields. The HTTP document version moves only when a generated request or response changes in a way an existing client cannot ignore. This branch moved the list body from `truncated` to `next` and `has_more`. That is a breaking HTTP change inside contract version `1` because no stable crate has been published. Callers of the old list body need to read `has_more`.

## Crate graph and publication order

Nothing in this branch was uploaded. `cargo publish --dry-run` aborts before the upload. Examples `myshop`, `notes` and `desk` stay `publish = false`.

Internal dependencies keep a path for workspace builds and `version = "0.1.0"` for the manifest Cargo writes into a package. The version matches `[workspace.package]`.

Publish in this order. A crate is not publishable until every crate it depends on is already on crates.io.

1. `ferrioxid-core`, `ferrioxid-macros`
2. `ferrioxid-db` after `ferrioxid-core`
3. `ferrioxid-static`, `ferrioxid-middleware`, `ferrioxid-mail`
4. `ferrioxid-migrations` after `ferrioxid-db`
5. `ferrioxid-auth` after migrations and mail
6. `ferrioxid-storage` after auth, and `ferrioxid-jobs` after auth
7. `ferrioxid-forms` after storage
8. `ferrioxid-cli` after jobs
9. `ferrioxid-admin` after forms
10. `ferrioxid-api` after admin
11. `ferrioxid`

`ferrioxid-storage` uses `ferrioxid-db` only as a dev-dependency. `ferrioxid-jobs` uses `ferrioxid-auth` only as a dev-dependency. Verification still fetches those crates.

## Packaging and dry-run on 2026-11-13

Compiler: `rustc 1.98.1` and `cargo 1.98.1` on Windows. Edition is 2021. `rust-toolchain.toml` selects stable. No `rust-version` key is set. A minimum supported version was not bisected, so 1.98.1 is the compiler that ran these checks, not a measured minimum. Linux packaging was not run. Pull request 102's CI covers the parent commits on Windows and Linux, not this packaging commit.

`cargo package --list` included the crate sources, `README.md`, and for the admin crate the templates, `static/css/admin.css` and `static/js/admin.js`. One census invocation also passed `--allow-dirty` while the tree was already clean, so it did not change the file set. No `.env`, key or certificate was in those lists. `src/password.rs` and `migrations/0004_password_reset.json` are code and schema. They contain column names, not secret values. The SPDX license `MIT` is in each manifest. `LICENSE-MIT` stays at the repository root and is not copied into each crate archive.

| Crate | `cargo publish --dry-run` | Archive |
|---|---|---|
| `ferrioxid-core` | Verified. Upload aborted because this was a dry run. | 12 files, 82.7KiB (23.6KiB compressed) |
| `ferrioxid-macros` | Verified. Upload aborted. | 8 files, 111.5KiB (21.5KiB compressed) |
| `ferrioxid-middleware` | Verified. Upload aborted. | 6 files, 37.5KiB (12.7KiB compressed) |
| `ferrioxid-static` | First verify failed: `tokio::fs` is not in the workspace Tokio features, and an isolated package does not inherit features from other workspace members. After `features = ["fs"]` on that crate, verify passed and the upload was aborted. | 6 files, 36.1KiB (12.1KiB compressed) |
| `ferrioxid-mail` | Passed after declaring Tokio `time`, which `tokio::time::timeout` needs in isolation. Upload aborted. | 12 files, 74.5KiB (21.6KiB compressed) |
| `ferrioxid-db` | Failed before an archive existed. crates.io has no `ferrioxid-core` 0.1.0. | Not produced |
| `ferrioxid-cli` | Failed the same way. crates.io has no `ferrioxid-auth` 0.1.0. | Not produced |
| `myshop` | `cargo publish` refuses it because `publish` is not true. `cargo package` also stops because the example dependency is path-only. | Not produced |

`ferrioxid-jobs` now declares Tokio `time` for the same isolation reason. Its dry-run cannot succeed until `ferrioxid-core`, `ferrioxid-db`, `ferrioxid-mail` and `ferrioxid-migrations` are on crates.io. That failure was not re-run after the mail result, because the missing crates are still the cause.

`cargo install` from a packaged `ferrioxid-cli` archive was not possible: the dry-run never wrote that archive. Install from a checkout remains `cargo install --path crates/ferrioxid-cli`.

`--allow-dirty` was not used for the successful dry-runs. One earlier attempt stopped with "changes that were not yet committed" while the Tokio feature edit was uncommitted. That edit was committed, and the dry-run was repeated on a clean tree.

## Release blockers

- Do not run `cargo publish` without an explicit decision. Dry-run is not a publish.
- Crates that depend on other Ferrioxid crates fail dry-run until those crates exist on crates.io, in the order above.
- The root `[patch.crates-io]` replaces `sqlx-sqlite` with `vendor/sqlx-sqlite`. Cargo does not copy a workspace patch into a published member. A consumer of a published `ferrioxid-db` would resolve upstream `sqlx-sqlite` 0.8.6 and miss the bundled SQLite bound fix. Remove that blocker only when a published SQLx or SeaORM release accepts the patched binding, as `vendor/README.md` already says.
- MSRV is not measured.
- The rename to Fexu is planned in [fexu-rename-plan.md](fexu-rename-plan.md) and was not performed. Publish the renamed crates once, not both names.
- This branch contains the commits from pull request 102 and adds the packaging work. Merge 102 first, or merge this pull request only when 102's commits are accepted with it. Do not publish from either branch as part of that merge.

## Dependencies on pull requests 97–100

## Install

Until a crate is published, install from a checkout. A dry-run of `ferrioxid-cli` does not produce an installable archive, because `ferrioxid-auth` is not on crates.io.

```powershell
cargo install --path crates/ferrioxid-cli
```

```bash
cargo install --path crates/ferrioxid-cli
```

The same command is the Linux and macOS form. `cargo install ferrioxid-cli` is not a supported command yet because the crate is not on crates.io.

## Upgrade

There is no previous crates.io release and no git tag that represents a published `0.1.0`. An upgrade test from an older published crate cannot be run. `ferrioxid upgrade` still only plans project-file changes and does not migrate a database. Schema changes stay `ferrioxid migrate up`, run by hand. Rollback of a migration is `ferrioxid migrate down` where that command already exists, on a disposable database.

## What this branch does not do

- It does not publish, tag or sign release artifacts.
- It does not label the project 1.0 or production-ready.
- It does not merge pull requests 97, 98, 99 or 100. Those branches are independent. See the dependency note below.
- It does not start the Compose stack when Docker is absent.

## Dependencies on pull requests 97–100

| PR | Branch | What this branch needs from it | What this branch does instead |
|---|---|---|---|
| 97 | `fix/runtime-security-docker` | Peer allow-list for `X-Forwarded-For`, embedded auth/job migrations | `check --deploy` fails when `FERRIOXID_TRUST_PROXY=1` because this branch trusts every peer. Migrations are still read from the existing files. |
| 98 | `feat/orm-migrations-lifecycle` | Grouped reports, JSON columns, `migrate plan` | Not included. List filters use the existing `Select`. |
| 99 | `feat/native-admin` | Per-record audit timeline | Not included. The Playwright test only opens `/admin/login`. |
| 100 | `feat/developer-apps` | One project file and conflicting env aliases | Not included. `ferrioxid.toml` behavior stays as on main. |

Do not merge those branches into this one to make the checks pass. The integration order is documented on the runtime branch, not here.

## PostgreSQL 18.6

The local binary install answered on `127.0.0.1:5432`. `psql` reported version 18.6. `pg_dump` and `pg_restore` are the same 18.6 build. No password was printed or stored.

The run created only databases named `ferrioxid_test_` plus 32 hexadecimal digits. `ferrioxid` stayed at 15 public tables. `ferrioxid_migrations_down_test` stayed at 1 public table. After the run, no `ferrioxid_test_*` database remained.

These ignored tests passed against that disposable catalog:

- `fresh_postgres_initializes_auth_jobs_and_application`
- `postgres_http_roundtrip`
- `postgres_migration_roundtrip`
- `postgres_enforces_cross_app_foreign_keys`
- `postgres_anonymous_session_roundtrip`
- `postgres_decimal_and_choice_roundtrip`
- `postgres_fixture_roundtrip`
- `postgres_reverts_backfill_and_foreign_key` (isolated databases, not `ferrioxid_migrations_down_test`)
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

Draft pull request 101 (`work/integration`) already combines pull requests 97–100. This branch does not contain those commits. `git merge-tree` against `origin/work/integration` reports content conflicts in:

- `.github/workflows/ci.yml`
- `CHANGELOG.md`
- `crates/ferrioxid-macros/src/lib.rs`
- `docs/index.md`
- `docs/testing.md`

Git can auto-merge other overlapping files, and those still need a semantic pass before anyone treats the result as correct:

- `crates/ferrioxid-cli/src/diagnose.rs` still fails `check --deploy` when `FERRIOXID_TRUST_PROXY=1` and still says the peer list is unmerged. On pull request 101 the flag does nothing unless `FERRIOXID_TRUSTED_PROXIES` names the immediate peer.
- `crates/ferrioxid-cli/src/main.rs` carries both `check --deploy` and the integration CLI commands.
- `crates/ferrioxid-api/src/lib.rs` and `openapi.rs` carry the cursor contract and the smaller integration edits. OpenAPI has to be checked again after that merge.
- `crates/ferrioxid-middleware/src/lib.rs`, `crates/ferrioxid-migrations/src/project.rs`, `crates/ferrioxid/tests/openapi_client.rs`, `examples/myshop/src/main.rs`, `Cargo.lock`, and the shared docs (`database.md`, `django-capability-matrix.md`) auto-merge today.
- `deploy/` and `examples/deploy/` are different paths. A merge would keep both stacks.

Resolve those conflicts on the integration branch. Do not copy pull request 101 into this branch to clear them.

## Security follow-up on work/integration (2026-11-13)

Pull request 101 now contains the merged work from pull requests 97–104. The notes above describe the earlier `work/api-quality` snapshot. They are not the current integration head.

`FERRIOXID_MFA_KEY` is required in production. The encoding is 64 hexadecimal characters and the value is 32 bytes. It is not derived from `FERRIOXID_CSRF_SECRET` and it has no `FERRUM_` alias. Existing development checkouts keep working: when the variable is unset and the process is not in production, Ferrioxid reads or creates `.ferrioxid/mfa.key`. Generated projects ignore that file. `ferrioxid check --deploy` reports the variable's presence and shape and does not print it.

A deployment that already stored confirmed TOTP under the old CSRF-derived key can still open that ciphertext. The next successful open re-seals it with the MFA key. Unused recovery codes from that enrollment stay HMAC hashes of the old key. Keep the current CSRF secret until those codes are used. New enrollments do not need the CSRF secret for TOTP or recovery codes.

`RUSTSEC-2023-0071` is still present because `Cargo.lock` records optional `sqlx-mysql`. It is not in the enabled build graph. No feature change removes it without leaving the SQLx 0.8.6 line that the vendored SQLite patch requires. The audit ignore stays limited to that one advisory. See [security-rsa.md](security-rsa.md) and [auth-threat-model.md](auth-threat-model.md).

`AuthConfig::development` draws one random 32-byte MFA key for the process. Session tokens are SHA-256 lookup hashes of 32 random bytes. Passwords are Argon2id. Finishing a pending second factor is `complete_pending_login`, and that result is a session. CodeQL classifies a method name containing the delimited word `mfa` as a password source, so this method name describes the session it returns.

Nothing in this follow-up was published to crates.io. The project was not renamed.
