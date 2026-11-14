# Advanced framework services

[Overview](index.md)

**Historical integration note:** this page records `work/framework-services` before the later PR integrations into `main`. Branch-specific pending merge instructions below are archival, not tasks currently awaiting completion. For current capabilities see [roadmap](roadmap.md), [auth providers](auth-providers.md), and [release readiness](release-readiness.md). It covers authentication providers, jobs, cache and storage. They cooperate only through explicit calls. Auth does not enqueue jobs, cache sessions, or delete files.

## What is implemented

- Password login, roles, object permissions and CSRF stay in place. Confirmed TOTP gates session issuance. See [auth-providers.md](auth-providers.md) and [auth-threat-model.md](auth-threat-model.md).
- Jobs stay at-least-once, with cron, backoff, dead letters as `failed`, and transactional enqueue. See [jobs.md](jobs.md) and [job-scheduling.md](job-scheduling.md).
- Cache is optional and process-local. See [cache.md](cache.md).
- Storage keeps local files, adds lifecycle and signed URLs, and exposes an S3 configuration shape without a network client. See [storage.md](storage.md) and [storage-remote.md](storage-remote.md).

## Merge notes for PR #101

PR #101 (`work/integration`) embeds auth migrations `0001` through `0005` and job migration `0001`. This branch does not take those changes. A later integration has to keep both histories in order:

1. Add `0006_account_security.json` after `0005_anonymous_sessions` in `EMBEDDED_MIGRATIONS` and `EXPECTED_MIGRATIONS`.
2. Add `0002_job_schedule.json` after `0001_jobs` in the job embed list.
3. `apply_migrations` on this branch still reads the migration directory. PR #101 calls `apply_files_with_history` on the embedded list. The merged function must apply the embedded files, including `0006` and `0002`, and must not skip a file because the directory helper and the embed list disagree.
4. `framework_names` on this branch expects 6 auth migrations, 2 job migrations and 25 tables, including `_fexu_mfa_totp`, `_fexu_recovery_codes` and `_fexu_login_challenges`.

Files changed on both branches, so the merge will need a manual resolution:

| File | This branch | PR #101 |
|---|---|---|
| `fexu-auth` `schema.rs`, `lib.rs` | MFA migration `0006` and modules | Embedded migration list stops at `0005` |
| `fexu-auth` `service.rs` | Login returns `MfaPending` instead of a session | Other auth service edits |
| `fexu-jobs` `schema.rs`, `lib.rs` | Migration `0002` and `table_ready` checks `cron_expression` | Embed list contains only `0001_jobs` |
| `fexu-jobs` `queue.rs` | Cron columns, transactional enqueue, and UUID binding for job ids | PostgreSQL UUID binding via `id_expr`. Keep one helper |
| `fexu-migrations` `lib.rs` | Exports `column_exists` | Embedded migration helpers |
| `fexu-cli` `diagnose.rs` | Job migration warning mentions `0002` | `aliased_env` for CSRF and SMTP |
| `docs/auth.md`, `docs/index.md`, `Cargo.lock` | Service documentation and new crates | Integration documentation and lockfile |

Do not depend on `aliased_env`. S3 credentials stay on the explicit names `FEXU_S3_ACCESS_KEY` and `FEXU_S3_SECRET_KEY`. Admin permission checks and generated projects in PR #101 do not call the MFA routes. After the merge, a password login for a user with confirmed TOTP must still return `auth.mfa_required` and no session cookie.

## Issues #97–#100

Those issues live on PR #101. This branch does not need their APIs and does not merge them.

| Issue | Why it is not a dependency |
|---|---|
| #97 embed, CSRF, proxy | Session and CSRF behavior used here is already on this branch. `column_exists` is exported from `fexu-migrations` so job readiness can see `cron_expression` |
| #98 ORM reports | No report API is called |
| #99 admin timeline | Account lifecycle does not write the admin journal |
| #100 config aliases | S3 environment names are the constants in `fexu-storage`. Code does not call an alias helper |

Later merges can conflict in auth configuration, the CLI diagnose text, `fexu-core`, and this documentation. Resolve those conflicts in the merge. Do not copy unreviewed APIs in early.

PostgreSQL tests for the new auth and job paths are `#[ignore]` and use `DATABASE_URL`. They are not part of the default SQLite run.
