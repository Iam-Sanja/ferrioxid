# Ferrioxid to Fexu rename plan

This is a plan. No identifier in the repository was renamed for it. The crates in this branch stay `ferrioxid` and `ferrioxid-*`. Nothing has been published to crates.io, so the rename should happen before the first publish. A published crate name cannot be renamed in place.

Checked from `work/release-readiness-final` on 2026-11-13. That branch starts at the squash of pull request 101, so `FERRUM_*` aliases and `FERRIOXID_TRUSTED_PROXIES` are already in the tree. Nothing here was renamed.

## Target names

| Item | Today | Target |
|---|---|---|
| GitHub repository | `Iam-Sanja/ferrioxid` | `fexu-rs` under an owner that is chosen at rename time |
| Main crate | `ferrioxid` | `fexu` |
| CLI crate | `ferrioxid-cli` | `fexu-cli` |
| CLI binary and Clap command | `ferrioxid` | `fexu` |
| Domain | none; homepage is the GitHub repository | `fexu.dev` |
| Other crates | `ferrioxid-core`, `ferrioxid-macros`, `ferrioxid-db`, `ferrioxid-migrations`, `ferrioxid-auth`, `ferrioxid-cache`, `ferrioxid-admin`, `ferrioxid-api`, `ferrioxid-forms`, `ferrioxid-static`, `ferrioxid-storage`, `ferrioxid-middleware`, `ferrioxid-mail`, `ferrioxid-jobs` | The same suffixes on `fexu-` |

Rust library crate names become `fexu_core` and so on because Cargo turns hyphens into underscores. `use ferrioxid::` becomes `use fexu::`. Proc-macro derives stay on the facade crate unless a later design splits them.

Examples `myshop`, `notes` and `desk` are `publish = false`. Their package names can stay. Their dependency on the facade changes with the facade name.

## Do this before crates.io

Publish the renamed crates once. Do not publish `ferrioxid-*` 0.1.0 and then add `fexu-*` as a second line. Two published names would leave generated projects and the SQLite patch instructions pointing at a name that the project no longer builds.

The workspace `[patch.crates-io]` for `sqlx-sqlite` does not travel inside a published crate. That blocker is in [release-readiness.md](release-readiness.md) and is independent of the rename.

## What a generated project contains today

`ferrioxid new` writes:

- `ferrioxid.toml`
- `.ferrioxid/` for local metadata and `csrf.key`
- `Cargo.toml` with `ferrioxid = { path = ... }` and a `[patch.crates-io]` entry for the vendored SQLite driver
- Rust sources that call `ferrioxid::prelude`, `ferrioxid::migrations`, `ferrioxid::apps`, `ferrioxid::urls`, `ferrioxid::views` and `ferrioxid::with_timestamps!`
- `.env.example` comments that name `FERRIOXID_CSRF_SECRET`, `FERRIOXID_PASSWORD`, `FERRIOXID_SMTP_PASSWORD` and `FERRIOXID_PUBLIC_ORIGIN`

An existing project also has a database. The framework creates these names itself and does not rewrite them on startup:

| Name | Role |
|---|---|
| `_ferrioxid_migrations` | Application migration history |
| `_ferrioxid_auth_migrations` | Embedded auth history |
| `_ferrioxid_job_migrations` | Embedded job history |
| `_ferrioxid_users`, `_ferrioxid_sessions`, `_ferrioxid_roles`, `_ferrioxid_permissions`, `_ferrioxid_user_roles`, `_ferrioxid_role_permissions` | Auth tables |
| `_ferrioxid_password_resets` | Password reset |
| `_ferrioxid_jobs` | Job queue |
| `_ferrioxid_admin_audit`, `_ferrioxid_admin_drafts` | Admin journal and drafts |
| `_ferrioxid_mail_outbox` | Mail outbox |

Cookies set by this branch:

- `ferrioxid_session`
- `ferrioxid_anonymous`
- `ferrioxid_login_csrf`
- `ferrioxid_reset`
- `ferrioxid_reset_csrf`

Environment variables read by this branch, not by pull request 101:

- `FERRIOXID_ENV`
- `FERRIOXID_DEBUG`
- `FERRIOXID_COOKIE_SECURE`
- `FERRIOXID_ALLOWED_ORIGINS`
- `FERRIOXID_TRUST_PROXY`
- `FERRIOXID_CSRF_SECRET`
- `FERRIOXID_SESSION_HOURS`
- `FERRIOXID_IDLE_MINUTES`
- `FERRIOXID_PASSWORD`
- `FERRIOXID_SMTP_PASSWORD`
- `FERRIOXID_PUBLIC_ORIGIN`
- `FERRIOXID_LOCALE`
- `FERRIOXID_SCHEMA_FILE`
- `FERRIOXID_CHECK_SKIP_CARGO`
- `FERRIOXID_CHECK_SKIP_SCHEMA`
- `FERRIOXID_LOG_FORMAT`
- `FERRIOXID_MIGRATIONS_DOWN_DATABASE_URL`

`DATABASE_URL` and `BIND_ADDRESS` are not framework-branded. Leave them.

## Compatibility rules

A rename that only changes Rust identifiers and the binary name makes every generated project fail to compile, and a rename of the history tables makes `migrate` treat an existing database as empty. Neither may happen silently.

1. Keep reading `ferrioxid.toml`. Accept `fexu.toml` only when `ferrioxid.toml` is absent. If both exist, stop with an error and do not guess.
2. Keep reading `.ferrioxid/`. Do not move `csrf.key` automatically. A later explicit command can copy it.
3. Keep the CLI able to run as `ferrioxid` for one release after `fexu` exists. Scripts and `schema_command` values in existing project files call `ferrioxid`.
4. Keep every `_ferrioxid_*` table name. New databases in that release still use those names. A later migration may copy history into `_fexu_*` tables, and it must be opt-in. Startup must not create a second history table and replay migrations.
5. Keep the cookie names for that same release. Changing them logs every user out. Document the release in which the old cookies stop being read.
6. Keep reading `FERRIOXID_*` and the `FERRUM_*` aliases that pull request 101 already accepts. Add `FEXU_*` as the preferred name. If two aliases for the same setting differ, fail without printing the values. Do not drop `FERRIOXID_*` or `FERRUM_*` in the same release that introduces `FEXU_*`.
7. Generated migration files store checksums. Do not rewrite the id or checksum of an embedded auth, job or admin migration as part of the rename.
8. OpenAPI names the session cookie `ferrioxid_session`. Change that text only when the cookie name changes, and say so in the HTTP contract note.
9. The generator should grow a flag or template version that writes `fexu` imports. Old output stays valid while the facade is still named `ferrioxid`, and new output waits until the crate rename is released. Do not make `ferrioxid new` emit `use fexu::` before crates.io has `fexu`.
10. Dependency detection in `ferrioxid_migrations::ferrioxid_dependency` must recognize both package names during the transition. A project that still says `ferrioxid = "0.1"` must not be reported as having no framework dependency.

## Order of work

1. Finish publish blockers that are not the name: SQLite patch, unpublished internal crates, and the isolated Tokio features.
2. Add the compatibility readers above while the packages are still called Ferrioxid. Tests must use an existing generated tree and an existing SQLite file with `_ferrioxid_migrations` and show that no second history table appears.
3. Rename the Rust crates, imports, binary, repository fields and docs in one commit series. Leave table names, cookie names and `FERRIOXID_*` readers in place.
4. Publish `fexu` and `fexu-*` at the version chosen for that release. Do not also publish the old names.
5. Point `fexu.dev` and the GitHub repository at that release. Keep the old repository as a redirect or a note, not as a second crate source.
6. Only in a later release, stop reading `ferrioxid.toml`, `FERRIOXID_*` and the `ferrioxid` binary. Table renames stay a separate, explicit migration.

## Files the rename series will touch

The list is the area, not a permission to edit it now.

- Root and member `Cargo.toml` files, `Cargo.lock`, `rust-toolchain.toml` is unchanged
- `crates/ferrioxid/src/lib.rs` and every `use ferrioxid` / `use ferrioxid_*` in crates, examples and the CLI generator
- `crates/ferrioxid-cli/src/main.rs` command name, `scaffold.rs`, `models.rs`, `apps.rs`
- `crates/ferrioxid-migrations/src/project.rs` and `history.rs`
- `crates/ferrioxid-auth/src/config.rs` and `recovery.rs`
- `crates/ferrioxid-api/src/cookie_guard.rs` and `openapi.rs`
- `crates/ferrioxid-admin` login cookie
- `crates/ferrioxid-cache` has no Ferrioxid dependency today. Its package name still changes with the others.
- `README.md`, `docs/`, `CHANGELOG.md`, `.github/workflows/ci.yml`
- Deploy examples that mention the binary or `FERRIOXID_*`

Pull request 101 is already squashed into `main`. The rename keeps that branch's `FERRUM_*` readers and `trusted_proxies`. It does not drop them while adding `FEXU_*`.
