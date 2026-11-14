# Fexu rename

The framework is Fexu. This checkout no longer reads Ferrioxid or Ferrum names.

**Historical rename decision:** no Fexu crate had been published at the time of the rename. Initial Fexu `0.1.0` components began publishing on 2026-11-14. The rename does not add a second configuration file, environment prefix, old cookie names, or an automatic conversion of existing application databases.

## Names

| Item | Name |
|---|---|
| Framework | Fexu |
| Main crate | `fexu` |
| CLI crate and binary | `fexu-cli`, command `fexu` |
| Other crates | `fexu-core`, `fexu-macros`, `fexu-db`, `fexu-migrations`, `fexu-auth`, `fexu-cache`, `fexu-admin`, `fexu-api`, `fexu-forms`, `fexu-static`, `fexu-storage`, `fexu-middleware`, `fexu-mail`, `fexu-jobs` |
| Rust imports | `fexu`, `fexu_core`, and the same pattern for the other crates |
| Project file | `fexu.toml` |
| Local directory | `.fexu/` |
| Environment | `FEXU_*` |
| Internal tables | `_fexu_*` |
| Cookies | `fexu_session`, `fexu_anonymous`, `fexu_login_csrf`, `fexu_reset`, `fexu_reset_csrf` |
| Documentation host | `fexu.dev` |

`fexu.dev` is the intended documentation host. This repository does not claim that the domain is registered or that it serves a site. The GitHub repository is [`Iam-Sanja/fexu-rs`](https://github.com/Iam-Sanja/fexu-rs). Cargo `repository` and `homepage` use that URL. GitHub redirects the previous repository URL.

## What was removed

`FERRIOXID_*`, `FERRUM_*`, `ferrioxid.toml`, `ferrum.toml`, `.ferrioxid/`, `_ferrioxid_*`, and the `ferrioxid_*` cookies are not read. A project that still has those names does not start as a Fexu project. Create `fexu.toml` and set `FEXU_*`. New databases use `_fexu_*` tables only. Fexu does not copy an old database.

`DATABASE_URL` and `BIND_ADDRESS` are unchanged. They are not framework brand names.

## Migration files

Embedded auth and job migrations keep their IDs (`0001_auth_schema` through `0006_account_security`, `0001_jobs`, `0002_job_schedule`). Table names inside those files are `_fexu_*`. Their checksums are recalculated from the canonical body so the file, the checksum, and the embedded list describe the same migration. Applied history in an old database is not rewritten and is not a supported upgrade.

## Tool

`scripts/rename_to_fexu.py` finished the mechanical rename and is not a maintained command. Its last version remains in the Git history of the rename. Do not restore or run it: historical documents, local catalog names, and numbered pull-request, issue and commit links intentionally keep the old repository spelling.

## GitHub repository

The repository was renamed from `Iam-Sanja/ferrioxid` to `Iam-Sanja/fexu-rs`. GitHub redirects the old URL. Numbered pull requests, issues, commits and the CodeQL snapshot keep the URLs written for that history. Describe `fexu.dev` as a live site only after the domain actually serves the documentation.

## Local PostgreSQL catalogs

This rename does not create, rename, migrate, or delete local PostgreSQL databases. Disposable tests create only names that start with `fexu_test_` and drop only those names. The GitHub Actions database is ephemeral and uses `fexu` there.

Existing local catalogs stay under their previous names. Do not point an in-place migration test at them, and do not drop them:

- `ferrioxid` (15 public tables)
- `ferrioxid_migrations_down_test` (1 public table)
- `ferrioxid_test_services_anon_20261112233529`
- `ferrioxid_test_services_auth_20261112233529`
- `ferrioxid_test_services_jobs_20261112233529`
- `ferrioxid_test_services_names_20261112233529`
- `ferrioxid_test_reliability_20261113030821`
- `ferrioxid_test_reliability_final_20261113105605`

Older notes in this repository that mention those catalogs under the new brand were rewritten by the mechanical pass. The catalogs on disk were not.
