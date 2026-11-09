# Historical names and the breaking Ferrioxid cleanup

This page records the removal of the old Ferrum naming contracts. Ferrioxid has no production installations or databases to preserve. The compatibility policy from [PR #18](ferrioxid-rename-plan.md) is superseded: current code recognizes and generates only the names in the right-hand column.

## Name mapping

| Historical name | Current name |
|---|---|
| `ferrum.toml` | `ferrioxid.toml` |
| `.ferrum/project.json` | `.ferrioxid/project.json` |
| `.ferrum/csrf.key` | `.ferrioxid/csrf.key` |
| `.ferrum/backup/` | `.ferrioxid/backup/` |
| `FERRUM_*` | `FERRIOXID_*` |
| `ferrum_session` | `ferrioxid_session` |
| `_ferrum_*` | `_ferrioxid_*` |
| `__FERRUM_COLUMNS_*` | `__FERRIOXID_COLUMNS_*` |

There are no fallback aliases or precedence rules for the historical names. `DATABASE_URL`, `BIND_ADDRESS`, `RUST_LOG`, application model names, CLI subcommands and the existing `ferrioxid:*` source markers are unchanged. The Cargo and project template versions are unchanged.

## Fresh projects and secrets

Run `ferrioxid new project` outside the framework workspace as shown in the [README](../README.md). Both API and fullstack templates create `ferrioxid.toml` and `.ferrioxid/project.json`. The configuration discovery code searches for the current TOML filename in the working directory and its ancestors.

Generation creates no CSRF key. Authentication creates or reads `.ferrioxid/csrf.key` only when runtime configuration needs a persistent key and `FERRIOXID_CSRF_SECRET` is not supplied. The generated `.gitignore` excludes this key, `.env` and `.env.*` while allowing `.env.example` and the non-secret project manifest. The model generator continues to use the markers from PR #24.

All framework-specific environment variables, including CLI test controls, mail/SMTP, production/debug, session lifetime, allowed origins, proxy trust and worker project selection, now use `FERRIOXID_`. No secrets belong in the TOML file, CLI arguments, Git or logs.

## Fresh databases

Auth and reset tables, roles and permissions, sessions, admin drafts/audits, the mail outbox and the job queue use `_ferrioxid_*`. The history tables are `_ferrioxid_auth_migrations`, `_ferrioxid_migrations` and `_ferrioxid_job_migrations`. Foreign keys, indexes, metadata and all readers/writers use the same names.

The four built-in auth migrations and the job migration were resealed with the existing migration engine after renaming their snapshots and operations. Their IDs stay `0001_auth_schema`, `0002_admin_journal`, `0003_audit_operation`, `0004_password_reset` and `0001_jobs`. Application migration IDs and app-package namespaces are unchanged.

This is an intentional schema and checksum break. Existing histories cannot be reused; use a fresh disposable SQLite or PostgreSQL database and run `ferrioxid migrate up`. No automatic conversion, copying, deleting or baselining of old data is performed. Normal migration checksum, ordering, validation, authorization and transaction checks remain enabled.

## Verification and historical references

Workspace tests cover discovery, both generated templates, model insertion, authentication, sessions, CSRF, permissions, jobs, workers and migrations. A fresh-schema integration test initializes auth, jobs and all myshop migrations twice to check both names and idempotency. Its PostgreSQL variant requires an empty disposable database and runs first in the dedicated PostgreSQL CI job.

Old names remain only in clearly historical documentation, changelog entries, the original MIT copyright attribution and negative regression assertions. Generated files and operational code contain no historical aliases. PostgreSQL and browser checks count as passed only when actually executed; local and CI results are recorded in the pull request.
