# Release readiness

Checked on 2026-11-12 from branch `work/api-quality`. Nothing in this branch was published to crates.io.

## Versions

| Name | Value | Meaning |
|---|---|---|
| Crate version | `0.1.0` | Cargo package version for every workspace member. Experimental. |
| HTTP contract | OpenAPI `info.version` = `1` | Generated list, problem JSON and read/write schemas described in [api.md](api.md). |
| Template version | unchanged | `ferrioxid upgrade` does not rewrite an existing project to match a new crate. |

Before 1.0, a new `0.1.x` may change Rust APIs, migration history and HTTP fields. The HTTP document version moves only when a generated request or response changes in a way an existing client cannot ignore. This branch moved the list body from `truncated` to `next` and `has_more`. That is a breaking HTTP change inside contract version `1` because no stable crate has been published. Callers of the old list body need to read `has_more`.

## Crate graph and publication order

Publication has to follow the path dependencies. A later crate cannot be published before the crates it uses.

1. `ferrioxid-core`
2. `ferrioxid-macros`
3. `ferrioxid-db`
4. `ferrioxid-static`, `ferrioxid-middleware`, `ferrioxid-storage`, `ferrioxid-mail`
5. `ferrioxid-migrations`, `ferrioxid-forms`, `ferrioxid-jobs`, `ferrioxid-auth`
6. `ferrioxid-admin`
7. `ferrioxid-api`
8. `ferrioxid`
9. `ferrioxid-cli`

`examples/myshop`, `examples/notes` and `examples/desk` are `publish = false`. They keep path dependencies.

Every framework crate still depends on its siblings with `path`. `cargo publish` from this workspace cannot succeed until those dependencies are version requirements for crates that already exist on crates.io. That rewrite is a release commit. It is not in this branch. `cargo package` was not treated as a green publish check for that reason.

Workspace metadata now sets `description`, `repository` and `homepage`. License is MIT. There is no `rust-version` key. The toolchain file selects stable Rust. A minimum supported version was not measured here.

## Install

Until a crate is published, install from a checkout:

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

Nothing in this follow-up was published to crates.io. The project was not renamed.
