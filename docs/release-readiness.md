# Release readiness

> **Rollout snapshot (2026-11-14):** six crates (`fexu-core`, `fexu-macros`, `fexu-cache`, `fexu-static`, `fexu-middleware`, `fexu-mail`) were confirmed in the crates.io index by Cargo. `fexu-db` passed the dry-run, but its first upload received HTTP 429 due to the new-crate rate limit. **This is a dated snapshot, not a live counter; verify individual crates on crates.io before proceeding.** This documentation branch does not publish anything.

Original readiness assessment checked on 2026-11-14 from `origin/main` at `e803261` (`feat: integrate SQLite and PostgreSQL without vendor patches`, pull request 115) on branch `release/crates-io-readiness`. The original readiness check uploaded or tagged nothing; PR #116 was subsequently squash-merged into `main` as `a39c952`.

Original decision: **READY** to begin publishing the 16 Fexu crates at `0.1.0`, in the order below, by a person who already has a crates.io token. This file is not that publication. `0.1.0` remains experimental and is not a 1.0 or production release.

The Fexu rename is already on `main`. Publish these `fexu*` names once. Do not publish a `ferrioxid` crate from this tree. There is no `[patch.crates-io]` table and no `vendor/sqlx-sqlite` directory. `cargo audit` has no advisory ignore.

## Names

Every name below returned HTTP 404 from `https://crates.io/api/v1/crates/<name>` and from the sparse index `https://index.crates.io/fe/xu/<name>`. The index check was confirmed against the existing crate `fexun` 0.1.0, which is a Bevy visual-novel plugin and a different name. crates.io treats `-` and `_` as the same name, so the 404 also covers the underscore spelling. No Fexu crate has an owner because none is registered.

| Crate | crates.io API | Sparse index |
|---|---|---|
| `fexu`, `fexu-core`, `fexu-macros`, `fexu-cache`, `fexu-static`, `fexu-middleware` | 404 | 404 |
| `fexu-mail`, `fexu-db`, `fexu-migrations`, `fexu-auth`, `fexu-storage` | 404 | 404 |
| `fexu-forms`, `fexu-jobs`, `fexu-cli`, `fexu-admin`, `fexu-api` | 404 | 404 |

`cargo info fexu --registry crates-io` and `cargo info fexu-cli --registry crates-io`, run outside this workspace, both stop with `could not find` in the crates.io index.

## Metadata

All 16 crates inherit `version = "0.1.0"`, `license = "MIT"`, `edition = "2021"`, `rust-version = "1.89.0"`, the repository, the homepage, and the workspace `README.md`. Each has its own description. Examples `myshop`, `notes`, and `desk` set `publish = false`.

The packaged manifest rewrites path dependencies to `version = "0.1.0"`. The five crates with no Fexu dependency were packaged. Their normalized manifests contain registry versions only. `LICENSE-MIT` stays at the repository root and is not copied into each archive; the SPDX field `MIT` is what crates.io records.

`cargo package --list --no-verify --locked` for all 16 archives lists sources, the workspace README, admin templates and static files, migration JSON, and tests. The lists contain no `.env`, private key, certificate, `vendor/` path, or `[patch.crates-io]` entry.

## Minimum supported Rust

`1.89.0`.

The resolved lockfile's highest `rust-version` is `1.89.0`, required by `rusqlite` 0.42.0 and `uuid` 1.29.0. `libsqlite3-sys` 0.40.0 requires `1.88.0`. `rust-toolchain.toml` still selects stable. On this machine stable is `1.98.1`, and that compiler ran fmt, clippy, the workspace tests, the workspace build, and `cargo audit`.

`cargo +1.89.0 check --workspace --locked --all-targets` passed after `rust-version` was set. Older compilers are outside the supported range.

## Dependency order

A crate can be uploaded only after every crate it names, including dev-dependencies, is visible in the crates.io index. `fexu-jobs` depends on `fexu-auth` only for tests, and `fexu-storage` depends on `fexu-db` only for tests. Verification still fetches those crates.

`fexu` does not depend on `fexu-cli`. `cargo add fexu` installs the library. `cargo install fexu-cli` installs the binaries `fexu` and `embedded-migrate`.

1. `fexu-core`, `fexu-macros`, `fexu-cache`, `fexu-static`, `fexu-middleware`
2. `fexu-mail`, `fexu-db`
3. `fexu-migrations`
4. `fexu-auth`
5. `fexu-storage`, `fexu-jobs`
6. `fexu-forms`, `fexu-cli`
7. `fexu-admin`
8. `fexu-api`
9. `fexu`

## Dry-run

`cargo publish --dry-run --locked` uploads nothing. Cargo prints `aborting upload due to dry run` and returns success only when packaging and verification finish.

| Crate | Result |
|---|---|
| `fexu-core` | Verified. Upload aborted. |
| `fexu-macros` | Verified. Upload aborted. |
| `fexu-cache` | Verified. Upload aborted. |
| `fexu-static` | Verified. Upload aborted. |
| `fexu-middleware` | Verified. Upload aborted. |
| `fexu-mail`, `fexu-db`, `fexu-migrations`, `fexu-auth`, `fexu-forms`, `fexu-jobs` | Stop before verification. The first missing package is `fexu-core`. |
| `fexu-storage`, `fexu-cli`, `fexu-admin` | Stop before verification. The first missing package is `fexu-auth`. |
| `fexu-api`, `fexu` | Stop before verification. The first missing package is `fexu-admin`. |

The error is `no matching package named <crate> found` with `location searched: crates.io index`. No other packaging error appears before that stop. Those eleven dry-runs become possible only after the earlier waves are in the index.

## Publication commands

Run these from a clean checkout of the commit that should be released. Authenticate with `cargo login` using a token that is not stored in this repository. Do not pass the token on the command line. Wait after each wave until `cargo info <name> --registry crates-io` shows `0.1.0` from crates.io rather than a path.

```powershell
cargo publish -p fexu-core --locked
cargo publish -p fexu-macros --locked
cargo publish -p fexu-cache --locked
cargo publish -p fexu-static --locked
cargo publish -p fexu-middleware --locked
```

```powershell
cargo publish -p fexu-mail --locked
cargo publish -p fexu-db --locked
```

```powershell
cargo publish -p fexu-migrations --locked
```

```powershell
cargo publish -p fexu-auth --locked
```

```powershell
cargo publish -p fexu-storage --locked
cargo publish -p fexu-jobs --locked
```

```powershell
cargo publish -p fexu-forms --locked
cargo publish -p fexu-cli --locked
```

```powershell
cargo publish -p fexu-admin --locked
```

```powershell
cargo publish -p fexu-api --locked
```

```powershell
cargo publish -p fexu --locked
```

After the index shows the crates:

```powershell
cargo add fexu
cargo install fexu-cli --locked
```

`cargo add fexu` and `cargo install fexu-cli` cannot succeed before that. Both names are absent from the registry. Until then, a checkout still installs the CLI with `cargo install --path crates/fexu-cli`.

## Checks on this commit

`DATABASE_URL` was unset. No existing database was changed. Ignored PostgreSQL tests stayed ignored.

On `e803261`, before this documentation commit:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo build --workspace --locked`
- `cargo audit` (1509 advisories, 349 lockfile crates, no ignore, exit 0)

The manifest change after that run is `rust-version = "1.89.0"`. `cargo +1.89.0 check --workspace --locked --all-targets` passed with that field set. The five leaf dry-runs were repeated on the committed tree.

An independent consumer at `D:\04_Projekte_Arbeit\Projekte\fexu-release-consumer` depends on `crates/fexu` by path, has no `[patch.crates-io]`, and is not part of this repository. Its test opens `sqlite::memory:`, rolls a transaction back, and reads `sqlite_version()`.

## What this check does not do

- It does not run `cargo publish` without `--dry-run`.
- It does not create a tag, a GitHub release, or a crates.io token.
- It does not merge the readiness pull request.
- It does not call the experimental `0.1.0` line stable.
