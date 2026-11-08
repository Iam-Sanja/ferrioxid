# Entwicklungsserver

[For overview](index.md) · **IST file:** `crates/ferrioxid-cli/src/develop.rs`.]

## Aufgabe

`ferrioxid dev` translates the project, starts the binary and restarts it after relevant file changes. `ferrioxid build` creates a release build.

## Responsible files

- `crates/ferrioxid-cli/src/develop.rs`
- `crates/ferrioxid-cli/src/main.rs`

## Public interface

```powershell
ferrioxid dev
ferrioxid build
```

Both commands read `ferrum.toml` in the working directory or a parent directory.

## Konfiguration

`BIND_ADDRESS` and `DATABASE_URL` apply to the started process. Before the build, `ferrioxid dev` checks the migration situation only by reading. Pending migrations or a non-auditable database result in a warning and not an abort. Nothing is migrated. `ferrioxid build` and `ferrioxid dev` do not start Node.js. The admin is included in the rust binary. The warning describes [migration-diagnostics.md](migration-diagnostics.md)].

## Datenfluss

A restart also reloads the Tera templates. `dev` starts the binary from `target/debug` or from `CARGO_TARGET_DIR/debug` when this variable is set. The guardian observes the project recursively. `target`, `node_modules`, `.git`, `dist` and `embedded-admin` are ignored. Only `.rs`, `.toml` and `.json` will trigger a restart. Between two starts are at least 400 milliseconds. The CLI starts `cargo build --bin <paket>` and then the Binary. A crash does not restart the server until a file changes. Ctrl+C ends the child process. At most, one server instance is running.

`build` checks the frontend for embedded assets and then executes `cargo build --release`. The list of results is output. If `CARGO_TARGET_DIR` is in the environment, this is the release folder there.

## Error cases

| Fall | Verhalten |
|---|---|
| Keine `ferrum.toml` |Termination with notice|
|Translation errors|Message that the old process has ended, no restart until the next change|
|The admin is missing in the binary|This is not due to a lack of frontend. A compile error of Rust remains a compile error.|
|Address starts with `0.0.0.0`|The generated application ends|

## Sicherheitsregeln

The guard does not start a shell with the filename. Child processes are terminated upon termination of the CLI. The admin is already in the binary.

## Beispiele

Windows and Linux:

```powershell
ferrioxid dev
```

```bash
ferrioxid dev
```

Finish in both cases with Ctrl+C.

## Tests

The integration into `crates/ferrioxid-cli/tests/generator.rs` sets `FERRUM_DEV_EXIT_MS` and checks that the server is listening and the process ends afterwards. A unit test checks that paths under `target` and `node_modules` are not relevant. Another unit test ends a child process.

## Limitations

Templates and CSS change with the next Rust build. `FERRUM_DEV_EXIT_MS` is only a test aid and not a user command.
