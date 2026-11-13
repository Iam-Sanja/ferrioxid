# RSA advisory RUSTSEC-2023-0071

`Cargo.lock` still records `rsa 0.9.10` because `sqlx 0.8.6` and `sqlx-macros-core 0.8.6` list the optional crate `sqlx-mysql 0.8.6`. Fexu does not enable the MySQL driver. Re-checked on 2026-11-13 from `work/integration`: `cargo tree --workspace --target all --prefix none --locked -i rsa` and the same command for `sqlx-mysql` both print nothing. The workspace `sea-orm` dependency already sets `default-features = false` and enables only `sqlx-postgres` and `sqlx-sqlite`. No feature flag in this workspace selects MySQL, so turning a feature off cannot delete the lockfile entry. Cargo still records SQLx's optional driver. Patching `rsa` to 0.10 would not compile against `sqlx-mysql` 0.8.6, and this project does not build that crate. Forking SQLx to drop the optional dependency is not a minimal fix. The advisory stays lockfile-only. Issue #8 stays open. The ignore is not extended.

Checked on this branch with:

```text
cargo tree --workspace --target all --prefix none --locked -i rsa
cargo tree --workspace --target all --prefix none --locked -i sqlx-mysql
```

Both commands printed `warning: nothing to print.` The crates are not in the enabled dependency graph for any workspace target.

The security workflow still runs `cargo tree --workspace --target all` and fails if a line starts with `rsa v`. The unfiltered `cargo audit` stays red on `RUSTSEC-2023-0071` so the advisory is visible. `cargo audit --ignore RUSTSEC-2023-0071` is only the gated check after that reachability proof. Issue #8 stays open.

## Removal path

`rsa 0.9.10` is a dependency of optional `sqlx-mysql 0.8.6`. The enabled graph compiles `sqlx-postgres` and the vendored `sqlx-sqlite`, not MySQL.

Do not patch `rsa` to 0.10 in this repository. The 0.10 API is not a drop-in replacement for the 0.9 calls inside `sqlx-mysql` 0.8.6, and a patch would change a crate this project does not build.

SQLx 0.9 can make the `rsa` crate opt-in for MySQL authentication, but that line still resolves `rsa` 0.9 when the feature is selected. It is not a fix for `RUSTSEC-2023-0071`, and it is not compatible with the vendored SQLite 0.8.6 patch for `RUSTSEC-2026-0459`. This change does not bump SeaORM or SQLx.

Delete the audit ignore only after a lockfile no longer contains `rsa` 0.9, `cargo tree` still prints nothing for `rsa`, and the unfiltered audit is clean. Until then the ignore stays exactly `RUSTSEC-2023-0071`.
