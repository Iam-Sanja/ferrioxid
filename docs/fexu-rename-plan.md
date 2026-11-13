# Fexu rename

The framework is Fexu. This checkout no longer reads Ferrioxid or Ferrum names.

Nothing has been published to crates.io. Generated projects and their databases do not need to keep working under the old names, so this rename does not add a second configuration file, a second environment prefix, old cookie names, or a migration of existing production databases.

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

`fexu.dev` is the intended documentation host. This repository does not claim that the domain is registered or that it serves a site. The Cargo `repository` and `homepage` fields stay on `https://github.com/Iam-Sanja/ferrioxid` until that GitHub repository is renamed.

The planned GitHub repository name is `fexu-rs`. The rename on GitHub is a separate step and has not been done.

## What was removed

`FERRIOXID_*`, `FERRUM_*`, `ferrioxid.toml`, `ferrum.toml`, `.ferrioxid/`, `_ferrioxid_*`, and the `ferrioxid_*` cookies are not read. A project that still has those names does not start as a Fexu project. Create `fexu.toml` and set `FEXU_*`. New databases use `_fexu_*` tables only. Fexu does not copy an old database.

`DATABASE_URL` and `BIND_ADDRESS` are unchanged. They are not framework brand names.

## Migration files

Embedded auth and job migrations keep their IDs (`0001_auth_schema` through `0006_account_security`, `0001_jobs`, `0002_job_schedule`). Table names inside those files are `_fexu_*`. Their checksums are recalculated from the canonical body so the file, the checksum, and the embedded list describe the same migration. Applied history in an old database is not rewritten and is not a supported upgrade.

## Tool

`scripts/rename_to_fexu.py` performs the mechanical rename. A run without `--apply` only reports. A second `--apply` changes nothing. The script leaves this plan's historical companions, the CodeQL evidence, `vendor/`, and live `Iam-Sanja/ferrioxid` URLs untouched.

## Later GitHub rename

1. Confirm `main` contains this rename and the crates are still unpublished.
2. Rename `Iam-Sanja/ferrioxid` to `fexu-rs` in GitHub settings. GitHub redirects the old repository URL.
3. Update the Cargo `repository` and `homepage` fields, the README badges, and the workflow badge URLs to the new repository.
4. Only then describe `fexu.dev` as a live site, and only after the domain actually serves the documentation.

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
