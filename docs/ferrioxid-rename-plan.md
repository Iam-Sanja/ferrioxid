# Ferrioxid rename implementation plan

Tracking: #9. **Historical implementation plan:** the framework and CLI rename was merged in PR #18. The crates and executable are now named Ferrioxid. The checklist below records the original scope and compatibility policy; it is not a claim that the rename is still pending.

## Identity

- Brand: **Ferrioxid**; facade crate: `ferrioxid`; CLI executable: `ferrioxid`; CLI package: `ferrioxid-cli`.
- Rename every `crates/ferrum*` directory, Cargo package/dependency, Rust import, macro expansion, generator template, example and test to the corresponding `ferrioxid*` name.
- Workspace version remains `0.1.0`, license remains MIT; support stays SQLite + PostgreSQL.
- Repository has already moved to `Iam-Sanja/ferrioxid`. The domain `ferrioxid.dev` is a branding target, not evidence of a deployed docs website.
- English CLI language (#16) and English technical documentation (#17) have separate scope.

## Atomic implementation checklist

1. Rename the 15 framework directories under `crates/` using Git-aware moves; update root workspace members and every package manifest, path dependency, feature target, binary target, and `Cargo.lock` through Cargo.
2. Update Rust crate references (`ferrum::`, `ferrum_*::`, derives, attributes and proc-macro tokens) across all framework crates and examples. Include all generator/scaffold code and test fixtures.
3. Rename CLI binary, help/prog name, package references, generated commands, README code examples and dev commands. Do **not** invent new subcommands.
4. Update native Admin branding defaults, example branding, documentation links, CI path references, and CLI tests. Keep custom application branding overrides working.
5. Search the full tree case-insensitively for `ferrum`; classify each remaining match as a required legacy alias/persistence key, test fixture, historical record or unfinished rename. No blind global replacement.
6. Keep this PR focused on a working atomic rename; do not publish crates or releases.

## Compatibility and safety

- Existing `ferrum.toml`, `FERRUM_*` settings, `_ferrum_*` database tables, migration IDs, old projects, and persisted data are **compatibility contracts**. Do not rename database tables or historical migrations simply to match the new brand.
- Preserve old config and environment settings during transition. If new `ferrioxid.toml` / `FERRIOXID_*` aliases are introduced, explicitly define precedence, conflicts, deprecation and tests. Never silently alter secrets or database paths.
- Use only disposable SQLite/PostgreSQL data for smoke tests. Never modify the external `testprojekt` or a real project database.
- Do not apply or delete the existing local `stash@{0}`; preserve the old local Admin branch until separately reviewed.
- Do not modify existing Git history, force-push or auto-merge.

## Validation gate

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --offline -- -D warnings`
- `cargo test --workspace --offline`
- `cargo build --workspace --offline`
- `cargo metadata --no-deps` with new package/path names
- `cargo run -p ferrioxid-cli -- --help`
- Generator smoke test that compiles a newly generated application with the renamed facade crate.
- Admin 2.0, worker, migrations and authentication regression tests.
- Legacy config / persisted schema compatibility tests.
- GitHub CI on both Windows and Ubuntu, PostgreSQL and the complete guarded Security audit.

The rename was implemented on `feat/framework-rename` and merged into `main` through PR #18. Subsequent fixes must continue to preserve legacy configuration and persisted data.
