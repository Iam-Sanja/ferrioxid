# Development server

[Overview](index.md) · **Implemented file:** `crates/fexu-cli/src/develop.rs`.

## Purpose

`fexu dev` compiles the project, starts its binary and restarts it after relevant file changes. `fexu build` creates a release build.

## Responsible files

- `crates/fexu-cli/src/develop.rs`
- `crates/fexu-cli/src/main.rs`

## Public interface

```powershell
fexu dev
fexu build
```

Both commands read `fexu.toml` in the working directory or a parent directory.

## Configuration

`BIND_ADDRESS` and `DATABASE_URL` apply to the started process. Before the build, `fexu dev` checks the migration situation only by reading. Pending migrations or a non-auditable database result in a warning and not an abort. Nothing is migrated. A busy `127.0.0.1`, `localhost` or `::1` port stops the command before the process starts. `fexu build` and `fexu dev` do not start Node.js. The admin is included in the rust binary. The warning describes [migration-diagnostics.md](migration-diagnostics.md).

## Data flow

Restarting reloads Tera templates. `dev` launches the binary from `target/debug` or `CARGO_TARGET_DIR/debug`. Its recursive file watcher ignores `target`, `node_modules`, `.git`, `dist` and `embedded-admin`; only `.rs`, `.toml` and `.json` changes trigger restarts, at least 400 ms apart. It runs `cargo build --bin <paket>` before launching the binary. A crash waits for the next file change. Ctrl+C stops the child process, and only one server instance runs at a time.

`build` runs `cargo build --release` and prints the resulting artifact paths. When `CARGO_TARGET_DIR` is set, artifacts are placed in its release directory. No frontend build is required.

## Error cases

| Case | Behavior |
|---|---|
| Missing `fexu.toml` | Exit with a diagnostic |
| Compilation errors | Report that the previous process has stopped; retry on the next change |
|The admin is missing in the binary|This is not due to a lack of frontend. A compile error of Rust remains a compile error.|
|Address starts with `0.0.0.0`|The generated application ends|

## Security rules

The guard does not start a shell with the filename. Child processes are terminated upon termination of the CLI. The admin is already in the binary.

## Examples

Windows and Linux:

```powershell
fexu dev
```

```bash
fexu dev
```

Finish in both cases with Ctrl+C.

## Tests

The integration into `crates/fexu-cli/tests/generator.rs` sets `FEXU_DEV_EXIT_MS` and checks that the server is listening and the process ends afterwards. A unit test checks that paths under `target` and `node_modules` are not relevant. Another unit test ends a child process.

## Limitations

Templates and CSS change with the next Rust build. `FEXU_DEV_EXIT_MS` is only a test aid and not a user command.
