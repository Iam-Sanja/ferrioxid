# Configuration and runtime environment

[Overview](index.md) · **Implemented:** `examples/myshop/src/main.rs`, `crates/fexu-db/src/lib.rs`, `.env.example`, `compose.yaml`.

`FEXU_LOCALE` accepts `de` or `en` for central CLI messages. The admin stores language in the HttpOnly `fexu_locale` cookie. Public branding, `[static]` and `[storage]` settings are described in [settings.md](settings.md). Uploads in `media/` do not belong in Git.

## Actual variables (Implemented)

| Name | Reader | Default | Purpose |
|---|---|---|---|
| `DATABASE_URL` | `myshop/src/main.rs` | `sqlite://db.sqlite3?mode=rwc` |Connection. The file path is relative to the working directory.|
| `BIND_ADDRESS` | `myshop/src/main.rs` | `127.0.0.1:3000` | TCP-Listener |
| `POSTGRES_PASSWORD` | Docker Compose | `fexu_dev_only` |Only the optional Postgres container|
| `RUST_LOG` |Tracing subscriber via the environment filter preset `info`| `info` in `.env.example` |Log level when the process sees it exported|
| `FEXU_ENV` | `AuthConfig::from_env` | `development` |`production` sets `Secure` as default and empties permitted origins if no list is set|
| `FEXU_COOKIE_SECURE` | `AuthConfig::from_env` |Only true in production|`1` or `true` enforces `Secure`. Not derived from `BIND_ADDRESS`|
| `FEXU_ALLOWED_ORIGINS` | `AuthConfig::from_env` |Loopback of the bind address|comma separated origins. If the variable is set, it replaces the list completely. Otherwise, `apply_bound_address` puts `http://127.0.0.1:{port}` and `http://localhost:{port}` from `BIND_ADDRESS` or `fexu.toml`. The host header is not taken over.|
| `FEXU_TRUST_PROXY` | `AuthConfig::try_from_env` | false |only together with a listed peer applies the first `X-Forwarded-For` hop|
| `FEXU_TRUSTED_PROXIES` | `AuthConfig::try_from_env` | empty |comma-separated immediate peers. A `*` entry is ignored|

`.env.example` is not automatically loaded. Values must be set in the shell. A set `DATABASE_URL` replaces SQLite:

```text
DATABASE_URL=postgres://fexu:fexu_dev_only@127.0.0.1:5432/fexu
```

Tests can use `sqlite::memory:` to create a private in-memory database. For every `sqlite:` URL, `connect` sets `max_connections(1)` and `PRAGMA foreign_keys=ON`, keeping bootstrap, repository and HTTP operations on the same connection. The example PostgreSQL password is for local development only. Never log the complete `DATABASE_URL`.

The SQLite file `db.sqlite3` as well as journal, WAL and SHM files are excluded from Git. Anyone who starts myshop from another working directory will receive a different file.

The full Settings description is in [settings.md](settings.md). `fexu.toml` is not a secret file. The workspace file continues to name only `migrations_dir` and `schema_command`. Generated projects may additionally contain `[project]`, `[database]`, `[server]` and `[admin]`. Unknown sections and keys are errors. `database.url` is standard `sqlite://db.sqlite3?mode=rwc`, `server.host` is `127.0.0.1`, `server.port` is 3000. `DATABASE_URL` replaces the file URL at runtime and is not printed by `migrate status`. The CLI starts `schema_command` in the directory of the TOML file and reads the schema of stdout.

`migrate baseline` reads the same URL. Without `--confirm`, the command does not write anything. For PostgreSQL, it ends with an unsupported message. Column and table names for `rename-column` and `rename-table` come from the CLI arguments and are tested as identifiers. They are not taken from HTTP requests.

## Not implemented (planned)

- A general configuration API beyond the implemented typed `ProjectConfig` and `Settings` sections.
- General CLI overrides for all typed settings; current precedence is documented in [settings.md](settings.md).
- Production start that rejects missing secrets.
- Profiles `development` and `production`.
- Adjustable pool size, timeouts and read replicas.

## Acceptance criteria

The localhost default remains `127.0.0.1:3000`. `0.0.0.0` is not a production-ready bind address. Compare [deployment.md](deployment.md) and [security.md](security.md).

## Admin

`FEXU_ADMIN_DIST` is no longer read; CSS and templates are embedded in the binary. `FEXU_CSRF_SECRET` is optional but must contain 64 hex characters when set. Otherwise, Fexu uses `.fexu/csrf.key`. `FEXU_COOKIE_SECURE` and `FEXU_ENV=production` apply to the admin session and to `fexu_anonymous`. Anonymous sessions use `FEXU_SESSION_HOURS` and `FEXU_IDLE_MINUTES`. The cookie name is not an environment variable. Theme and locale use `fexu_theme` and `fexu_locale` cookies.
