# RSA advisory RUSTSEC-2023-0071

`RUSTSEC-2023-0071` matched `rsa` 0.9.10 because the `sqlx` 0.8.6 facade recorded optional `sqlx-mysql`. Fexu no longer depends on the `sqlx` facade or on `sqlx-mysql`. `Cargo.lock` does not contain `rsa`.

The security workflow runs `cargo audit` without an ignore for this advisory. It still fails the build if a workspace target enables a crate whose name starts with `rsa v`. Do not add an ignore to make that check pass. If `rsa` returns, fix the dependency or record a new, reviewed exception. The old exception is not still in force.
