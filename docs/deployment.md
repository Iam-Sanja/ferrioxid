# Deployment and operation

[Overview](index.md) · Local SQLite remains the default. `compose.yaml` starts PostgreSQL on `127.0.0.1`. [deploy/README.md](../deploy/README.md) is a disposable app, proxy and database example. It was not started as part of writing the files when Docker is absent.

## Local Development (Implemented)

SQLite is the standard database. Rust and Cargo are enough. Docker and PostgreSQL do not have to be installed.

```powershell
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

A generated project starts with `ferrioxid dev` in the project directory. Windows and Linux use the same command. The server starts without automatic migration. First run `ferrioxid migrate up`. Generated applications do not bind to `0.0.0.0`. `myshop --bootstrap` does not change the file. The file `db.sqlite3` is in the working directory. An older bootstrap file without `_ferrioxid_migrations` is not taken over and not deleted.

The admin does not need a node process. `cargo build` is sufficient. Put `FERRIOXID_CSRF_SECRET` into production. The `.ferrioxid/csrf.key` file is only the local source. Production should set `FERRIOXID_ENV=production` or `FERRIOXID_COOKIE_SECURE=1`, and `FERRIOXID_ALLOWED_ORIGINS` to the public `https` origins. A reverse proxy must forward `Origin` and `Referer` for cookie-authenticated unsafe requests. `FERRIOXID_TRUST_PROXY` only changes the login rate-limit client address. It does not add an origin and it does not turn off `Secure`. Do not log `Cookie`, `x-csrf-token`, or `Authorization`.

`ferrioxid check --deploy` is read-only. It reports debug, bind address, cookie `Secure`, the presence and shape of `FERRIOXID_CSRF_SECRET` without printing it, https origins, pending migrations, and the proxy flag. `FERRIOXID_TRUST_PROXY=1` fails this check on this branch because `X-Forwarded-For` would be trusted from every peer. A peer allow-list is pull request 97 and is not merged here. The check does not migrate.

`ferrioxid::serve` stops the process on Ctrl+C and, on Unix, on SIGTERM. `FERRIOXID_LOG_FORMAT=json` selects JSON logs. The access log records method, path, status, elapsed time and `x-request-id`. Query strings are not logged. An inbound `x-request-id` is kept only when it is 8 to 64 ASCII letters, digits or hyphens. OpenTelemetry is not bundled. `/healthz` does not touch the database. `/readyz` runs `SELECT 1` and does not migrate. `App::strict_migrations` refuses startup when migrations are pending and does not apply them.

A different `BIND_ADDRESS`, in particular `0.0.0.0:3000`, can make the service accessible on other networks. With the anonymous demo model, this is not an Internet deployment. Inside `deploy/compose.yaml` the app listens on `0.0.0.0` only on the Compose network. The published port is the proxy on `127.0.0.1:8443`. The database port is not published.

## Optional PostgreSQL (Implemented)

`compose.yaml` starts `postgres:16-alpine` with database, user, volume and `pg_isready`. The port is limited to `127.0.0.1:5432:5432`. That file does not build the API. The disposable image, proxy and unpublished database live in [deploy/README.md](../deploy/README.md).

```powershell
docker compose up -d db
$env:DATABASE_URL="postgres://ferrioxid:ferrioxid_dev_only@127.0.0.1:5432/ferrioxid"
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

The same registered application then speaks PostgreSQL. The handlers remain unchanged. Docker is an optional tool for this server, not the prerequisite for Ferrioxid.

## Production architecture (not a production deployment)

`deploy/` is a local example: multi-stage image, non-root user, Caddy on `127.0.0.1:8443`, `/readyz`, graceful shutdown and a separate migration step. It was not booted when Docker was absent. It is not a production deployment. Still required before calling a host production:

```text
TLS-Client → Reverse Proxy → Ferrioxid-App → PostgreSQL
                                  └→ Log-/Metrics-Ziel
```

1. A separately managed PostgreSQL, not the unpublished example database.
2. Forwarded headers only from trusted proxies. This branch rejects `FERRIOXID_TRUST_PROXY=1` in `check --deploy`.
3. HTTPS on the proxy, with session cookies and CSRF. The example uses Caddy's local issuer.
4. Secrets from the operational environment, not from the image or the repository.
5. Schema changes through `ferrioxid migrate up`, not at process start. `down` is not that step.
6. No productive `allow_anonymous=true` without a security review.
7. Timeouts, connection limits and a metrics target. OpenTelemetry is not bundled.

SQLite can carry local development and testing. For several app processes, it is not the intended production database: the pool has a connection, and write accesses serialize through the file. PostgreSQL remains the supported server backend as soon as it is operated.

## Docker, Portainer and Nginx Proxy Manager

`deploy/compose.yaml` uses an internal network and Caddy. Portainer and Nginx Proxy Manager are not part of this repository.

## Acceptance criteria

The local SQLite startup does not need a database server. An optional PostgreSQL setup remains limited to localhost. Secrets do not appear in the log. `deploy/` does not publish the database and does not migrate on startup.

## Serving the admin

`/admin` renders HTML from the binary. Unknown asset paths are 404. `/admin/api` remains the JSON API. Paths with `..` are not read as files.
