# Advanced framework progress

This file records the current development pass. An issue is complete only after the code is tested, documented, CI is green, and the change is on `main`. Nothing in this pass is on `main` yet.

## Block 1 branch

`fix/runtime-security-docker`, based on `main` at `53a95aa` (PR #73).

| Issue | Status | Already present | Added in this pass | Tests | Still open |
| --- | --- | --- | --- | --- | --- |
| #74 | implemented locally | Auth and jobs called `apply_directory` on `CARGO_MANIFEST_DIR`. | JSON is embedded with `include_str`. `apply_migrations` uses those bytes. Job primary keys are bound as PostgreSQL `uuid` and as SQLite text. `embedded-migrate` runs from another directory. | SQLite memory test. Release binary copied outside the source tree: SQLite apply, login and enqueue succeeded. Ignored PostgreSQL test applies auth and jobs, logs in, enqueues and reads the job back. CI steps exist and have not run on GitHub. | CI and merge. Docker is not installed on this machine, so the compose file was not booted. |
| #75 | implemented locally | A missing directory was an empty migration set. | Required auth and jobs packages fail when empty, incomplete or corrupt. `inspect_framework` reports auth, jobs and the application separately and does not migrate. An absent app directory stays optional. | Unit tests for empty, corrupt, absent and empty directories. Readiness test checks that history tables are not created. | CI and merge. |
| #76 | implemented locally | Security middleware always wrote `Referrer-Policy: no-referrer`. | The handler's `Referrer-Policy` is kept. The framework default fills a missing value and can be set to a listed policy. `nosniff` and `DENY` are always enforced. | Middleware tests over the full chain. | A browser walk of LAN HTTP and HTTPS is not automated in this pass. Header and origin tests cover the HTTP contract. |
| #77 | implemented locally | `require_browser` rejected `POST /admin/login` before the login CSRF cookie was checked. | A fresh jar can submit the native admin login with its own CSRF cookie. Other writes still need a session cookie. Foreign origins and unverified `Origin: null` stay rejected. | `direct_admin_login_survives_require_browser_writes_from_a_fresh_jar`. | Browser click-through is not a separate automated test. |
| #78 | implemented locally | `0.0.0.0` was rejected. `trust_proxy` accepted `X-Forwarded-For` from every peer. | Wildcard bind is opt-in and is not an origin. Forwarded client keys require both the flag and a listed peer. Compose example keeps PostgreSQL unpublished. | Project tests and the auth proxy test. | `docker` is not installed on this machine, so the compose file was reviewed and not booted. |
| #8 | assessed, not removed from the lockfile | CI ignores `RUSTSEC-2023-0071` only after `cargo tree` shows `rsa` is absent from the enabled graph. | Re-checked with `cargo tree`. See `docs/security-rsa.md`. | The existing CI `cargo tree` gate remains. | The lockfile still contains the optional SQLx MySQL dependency. Removing it would mean changing SQLx itself. The issue stays open until that exception is accepted on `main`. |

## Verification in this pass

Local, not GitHub CI, and not merged:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed. The pre-existing `linker_messages` warning on `ferrioxid-macros` remains and is not denied by `-D warnings`.
- `cargo test --workspace` passed on SQLite. Ignored PostgreSQL tests in that command were not executed.
- `postgres_embedded_auth_and_jobs` passed against a disposable database created from the local `postgres` admin database. The populated `ferrioxid` and `ferrioxid_migrations_down_test` databases were unchanged (15 and 1 public tables).
- The release `embedded-migrate` binary was copied outside the source tree. It applied auth and jobs, logged in, and enqueued a job on a fresh SQLite file and on a disposable PostgreSQL database. That database was dropped.
- `cargo tree --workspace --target all --prefix none --locked -i rsa` and the same query for `sqlx-mysql` both printed nothing.
- Docker is not installed here, so `examples/deploy/compose.yaml` was not started.

## Local commits

Implementation: `d556bcc3bbfa5b624c5d27ef13d2682ee06b5652`.
The earlier progress note is `ad487023d79d9faebd0a711258cdc848e1bb9fa2`.
GitHub CI has not run. This branch is not merged.

## Later blocks

Blocks 2, 3 and 4 exist on `feat/orm-migrations-lifecycle`, `feat/native-admin` and `feat/developer-apps`. They are not contained in this branch. Their issues stay open.
