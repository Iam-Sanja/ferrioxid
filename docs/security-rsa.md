# RSA advisory RUSTSEC-2023-0071

`Cargo.lock` still records `rsa 0.9.10` because `sqlx-mysql 0.8.6` lists it. Ferrioxid does not enable the MySQL driver.

Checked on this branch with:

```text
cargo tree --workspace --target all --prefix none --locked -i rsa
cargo tree --workspace --target all --prefix none --locked -i sqlx-mysql
```

Both commands printed `warning: nothing to print.` The crates are not in the enabled dependency graph for any workspace target.

The security workflow still runs `cargo tree --workspace --target all` and fails if a line starts with `rsa v`. The unfiltered `cargo audit` stays red on `RUSTSEC-2023-0071` so the advisory is visible. `cargo audit --ignore RUSTSEC-2023-0071` is only the gated check after that reachability proof. Issue #8 stays open.

## Removal path

`rsa 0.9.10` is a dependency of `sqlx-mysql 0.8.6` in `Cargo.lock`. The workspace enables `sqlx-postgres` and `sqlx-sqlite`, not MySQL, so the crate is not compiled. Cargo still records the optional MySQL driver.

Do not patch `rsa` to 0.10 in this repository. The 0.10 API is not a drop-in replacement for the 0.9 calls inside `sqlx-mysql`, and a patch would change a crate this project does not build.

The removal is an upstream bump: when a `sqlx` release moves `sqlx-mysql` to `rsa >= 0.10`, update SeaORM/SQLx together, run `cargo update -p sqlx-mysql`, and delete the audit ignore only after `cargo tree` still prints nothing for `rsa` and the unfiltered audit is clean. Until that release exists, keep the ignore narrow and do not add another advisory to it.
