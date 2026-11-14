# Documentation consistency audit — Fexu 0.1.0

**Audit date:** 2026-11-14  
**Source baseline:** `main` at `a39c9520879862d128328ebf798553b90b726dbc`  
**Working branch:** `docs/issue-119-refresh` · [Issue #119](https://github.com/Iam-Sanja/fexu-rs/issues/119) · [PR #120](https://github.com/Iam-Sanja/fexu-rs/pull/120)

## Scope and method

- The GitHub recursive tree returned **148 tracked Markdown documents**, of which **138 are under `docs/`**, including nested security notes. All 148 files were retrieved and examined in batches for the known stale terminology and publication claims.
- Checked each Markdown file for local Markdown-link **target path existence** against the complete tracked tree. No missing local path targets were found with this basic path-based check. **External URLs, GitHub redirects and destination heading anchors were not automatically validated.**
- Checked balanced triple-backtick code fence counts in every file; no odd totals were reported. This is a structural check, **not** a parser or execution of fenced Rust, TOML, SQL, shell and PowerShell examples.
- Cross-checked critical live assertions against current Rust implementation: `crates/fexu-db/src/`, `crates/fexu-auth/src/{config,mfa,totp}.rs`, `crates/fexu-cli/src/diagnose.rs` and the PostgreSQL/browser CI description.
- Historical issue links and older branch snapshots were retained as historical records instead of being rewritten into fictional current test results.
- This audit cannot prove every line of every page correct, or that every example compiles. Full code-example integration tests and external link checks require a checked-out repository with Rust and network where applicable.

## Corrected current-facing documentation

| Pages | Prior inconsistency | Resolution |
|---|---|---|
| `README.md`, `docs/architecture.md`, `docs/database.md`, `docs/security.md`, `CONTRIBUTING.md` | Active SeaORM execution path, stale publication status | Described SeaQuery with bundled rusqlite SQLite and direct sqlx-postgres; added release qualification |
| `docs/roadmap.md`, `docs/index.md`, `docs/implementation-guide.md` | Internal v0.12–v0.28 feature labels presented like releases, features marked in PR | Replaced with source capability overview; distinguished Cargo 0.1.0 from historical milestones |
| `docs/django-capability-matrix.md`, `docs/framework-comparison.md` | Old work branches and pre-merge status; incorrect claims of no MFA, cron, cache or cursor-list support | Corrected current main baseline, MFA, cron, typed cache, cursor/OpenAPI and historical comparative evidence |
| `docs/auth.md`, `docs/security.md`, `docs/admin.md`, `docs/admin-security.md` | TOTP MFA falsely listed as missing | Documented implemented local TOTP/recovery and distinguish absent OAuth/OIDC/JWT |
| `docs/auth-providers.md` | MFA key wrongly derived from CSRF key for new enrollments | Documented `FEXU_MFA_KEY` for v2 records, with legacy-CSRF recovery and rotation limitations |
| `docs/deployment.md`, `deploy/README.md`, `docs/framework-comparison.md` | PR #97 'unmerged', any trusted proxy accepted | Aligned with current `FEXU_TRUSTED_PROXIES` peer allow-list and CLI checks |
| `docs/release-readiness.md`, `CHANGELOG.md`, `docs/project-generator.md`, `docs/cli.md` | No crates available or no first release record | Dated staged-release notes with an explicit per-crate confirmation boundary |
| `docs/advanced-framework-progress.md`, `docs/framework-quality-review.md`, `docs/advanced-services.md`, `docs/advanced-framework-integration.md`, `docs/orm-capability-matrix.md`, `docs/admin-devex.md`, `docs/public-release-checklist.md`, `docs/github-maintenance.md` | Past branch snapshots sounded like today's development state | Added historical-context labels while preserving dated observations |

## Additional link check

- The README's `Features` fragment linked to non-existent `#what-can-fexu-do`; corrected to `#why-fexu` after checking actual headings.
- Anchor fragment validation is **not** complete across all 148 documents. The path-only Markdown scan found no missing local document targets; external URLs are unverified.

## Release status is intentionally time-scoped

As of the maintainer's latest status in this conversation, eight of sixteen `0.1.0` crates were reported published: `fexu-core`, `fexu-macros`, `fexu-cache`, `fexu-static`, `fexu-middleware`, `fexu-mail`, `fexu-db`, `fexu-migrations`. The first seven were confirmed by Cargo's index lookup in the provided terminal logs; `fexu-migrations` was reported online without its corresponding `cargo info` transcript. **Registry availability changes independently of the repo.** Always check crates.io before using `cargo add fexu` or `cargo install fexu-cli`.

## Follow-up checks before merging

1. Wait for all required checks on the **latest PR head**; previous cancelled check runs are not a CI pass.
2. Review Markdown anchor fragments and external links using the checkout's markdown tooling; target-path existence alone cannot catch changed heading slugs.
3. Smoke-test copyable CLI and generated-project commands on Rust 1.89+ using a disposable project after `fexu` and `fexu-cli` are published.
4. Check source-versus-doc claims around the PostgreSQL fixture concurrency issue [#117](https://github.com/Iam-Sanja/fexu-rs/issues/117) and live registry rollout before removing the draft label.

This is a record of what was **actually checked**, not a universal certification of all documented behavior.
