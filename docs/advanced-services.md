# Advanced framework services

[Overview](index.md)

This note covers the services on `work/framework-services`: authentication providers, jobs, cache and storage. They cooperate only through explicit calls. Auth does not enqueue jobs, cache sessions, or delete files.

## What is implemented

- Password login, roles, object permissions and CSRF stay in place. Confirmed TOTP gates session issuance. See [auth-providers.md](auth-providers.md) and [auth-threat-model.md](auth-threat-model.md).
- Jobs stay at-least-once, with cron, backoff, dead letters as `failed`, and transactional enqueue. See [jobs.md](jobs.md) and [job-scheduling.md](job-scheduling.md).
- Cache is optional and process-local. See [cache.md](cache.md).
- Storage keeps local files, adds lifecycle and signed URLs, and exposes an S3 configuration shape without a network client. See [storage.md](storage.md) and [storage-remote.md](storage-remote.md).

## Issues #97–#100

Those issues live on other branches. This branch does not need their APIs and does not merge them.

| Issue | Why it is not a dependency |
|---|---|
| #97 embed, CSRF, proxy | Session and CSRF behavior used here is already on this branch. `column_exists` is exported from `ferrioxid-migrations` so job readiness can see `cron_expression` |
| #98 ORM reports | No report API is called |
| #99 admin timeline | Account lifecycle does not write the admin journal |
| #100 config aliases | S3 environment names are the constants in `ferrioxid-storage`. Code does not call an alias helper |

Later merges can conflict in auth configuration, the CLI diagnose text, `ferrioxid-core`, and this documentation. Resolve those conflicts in the merge. Do not copy unreviewed APIs in early.

PostgreSQL tests for the new auth and job paths are `#[ignore]` and use `DATABASE_URL`. They are not part of the default SQLite run.
