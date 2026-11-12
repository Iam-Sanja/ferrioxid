# RSA advisory RUSTSEC-2023-0071

`Cargo.lock` still records `rsa 0.9.10` because `sqlx-mysql 0.8.6` lists it. Ferrioxid does not enable the MySQL driver.

Checked on this branch with:

```text
cargo tree --workspace --target all --prefix none --locked -i rsa
cargo tree --workspace --target all --prefix none --locked -i sqlx-mysql
```

Both commands printed `warning: nothing to print.` The crates are not in the enabled dependency graph for any workspace target.

The security workflow still runs `cargo tree --workspace --target all` and fails if a line starts with `rsa v`. `cargo audit --ignore RUSTSEC-2023-0071` remains only for that lockfile-only match. Issue #8 stays open because the advisory is not gone from the lockfile and this exception is not a merged decision by itself.
