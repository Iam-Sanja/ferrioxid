# Configuration and runtime environment

[For overview](index.md) · **IST:** `examples/myshop/src/main.rs`, `crates/ferrioxid-db/src/lib.rs`, `.env.example`, `compose.yaml`.]

`FERRUM_LOCALE` can be `de` or `en` and controls the central CLI texts. The browser stores the admin language under `ferrioxid-locale`. Public branding values describe [settings.md](settings.md). `[static]` and `[storage]` are in the same file.] `media/` contains uploads and does not belong in Git.

## Actual variables (IST)

| Name | Einleser | Default | Verwendung |
|---|---|---|---|
| `DATABASE_URL` | `myshop/src/main.rs` | `sqlite://db.sqlite3?mode=rwc` |Connection. The file path is relative to the working directory.|
| `BIND_ADDRESS` | `myshop/src/main.rs` | `127.0.0.1:3000` | TCP-Listener |
| `POSTGRES_PASSWORD` | Docker Compose | `ferrioxid_dev_only` |Only the optional Postgres container|
| `RUST_LOG` |Tracing subscriber via the environment filter preset `info`| `info` in `.env.example` |Log level when the process sees it exported|
| `FERRUM_ENV` | `AuthConfig::from_env` | `development` |`production` sets `Secure` as default and empties permitted origins if no list is set|
| `FERRUM_COOKIE_SECURE` | `AuthConfig::from_env` |Only true in production|`1` or `true` enforces `Secure`. Not derived from `BIND_ADDRESS`|
| `FERRUM_ALLOWED_ORIGINS` | `AuthConfig::from_env` |Loopback of the bind address|comma separated origins. If the variable is set, it replaces the list completely. Otherwise, `apply_bound_address` puts `http://127.0.0.1:{port}` and `http://localhost:{port}` from `BIND_ADDRESS` or `ferrum.toml`. The host header is not taken over.|
| `FERRUM_TRUST_PROXY` | `AuthConfig::from_env` | falsch |only then applies the first `X-Forwarded-For` hop|

`.env.example` is not automatically loaded. Values must be set in the shell. A set `DATABASE_URL` replaces SQLite:

```text
DATABASE_URL=postgres://ferrioxid:ferrioxid_dev_only@127.0.0.1:5432/ferrioxid
```

Tests can use `sqlite::memory:`. This URL creates a private storage database. `connect` sets for each `sqlite:` URL `max_connections(1)` and `PRAGMA foreign_keys=ON`. This keeps the bootstrap, repository and HTTP handler on the same connection. The sample password only applies to local PostgreSQL development. `DATABASE_URL` never fully log.

The SQLite file `db.sqlite3` as well as journal, WAL and SHM files are excluded from Git. Anyone who starts myshop from another working directory will receive a different file.

The full Settings description is in [settings.md](settings.md)]. `ferrum.toml` is not a secret file. The workspace file continues to name only `migrations_dir` and `schema_command`. Generated projects may additionally contain `[project]`, `[database]`, `[server]` and `[admin]`. Unknown sections and keys are errors. `database.url` is standard `sqlite://db.sqlite3?mode=rwc`, `server.host` is `127.0.0.1`, `server.port` is 3000. `DATABASE_URL` replaces the file URL at runtime and is not printed by `migrate status`. The CLI starts `schema_command` in the directory of the TOML file and reads the schema of stdout.

`migrate baseline` reads the same URL. Without `--confirm`, the command does not write anything. For PostgreSQL, it ends with an unsupported message. Column and table names for `rename-column` and `rename-table` come from the CLI arguments and are tested as identifiers. They are not taken from HTTP requests.

## Not implemented (reference)

- Zentraler typisierter `FerrioxidConfig`.
- Fixed source order of defaults, file, environment and CLI.
- Production start that rejects missing secrets.
- Profiles `development` and `production`.
- Adjustable pool size, timeouts and read replicas.

## Abnahme

The localhost default remains `127.0.0.1:3000`. `0.0.0.0` is not a production-ready bind address. Compare [deployment.md](deployment.md) and [security.md](security.md)].

## Admin

`FERRUM_ADMIN_DIST` is no longer read. CSS and templates come from the binary. `FERRUM_CSRF_SECRET` is optional and must be 64 hex characters long. Without the variable used ferrioxide `.ferrum/csrf.key`. `FERRUM_COOKIE_SECURE` and `FERRUM_ENV=production` apply to the admin cookie as well as to the rest of the session. Theme and language are in the cookies `ferrioxid_theme` and `ferrioxid_locale`.
