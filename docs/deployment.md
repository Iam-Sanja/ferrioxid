# Deployment and operation

[Overview](index.md) · **Implemented:** local process with SQLite. Docker is optional and only launches PostgreSQL. **Planned:** App container and reverse proxy.

## Local Development (Implemented)

SQLite is the standard database. Rust and Cargo are enough. Docker and PostgreSQL do not have to be installed.

```powershell
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

A generated project starts with `ferrioxid dev` in the project directory. Windows and Linux use the same command. The server starts without automatic migration. First run `ferrioxid migrate up`. Generated applications do not bind to `0.0.0.0`. `myshop --bootstrap` does not change the file. The file `db.sqlite3` is in the working directory. An older bootstrap file without `_ferrum_migrations` is not taken over and not deleted.

The admin does not need a node process. `cargo build` is sufficient. Put `FERRUM_CSRF_SECRET` into production. The `.ferrum/csrf.key` file is only the local source.

A different `BIND_ADDRESS`, in particular `0.0.0.0:3000`, can make the service accessible on other networks. With the anonymous demo model, this is not an Internet deployment.

## Optional PostgreSQL (Implemented)

`compose.yaml` starts `postgres:16-alpine` with database, user, volume and `pg_isready`. The port is limited to `127.0.0.1:5432:5432`. The repository does not contain a Dockerfile for the API server.

```powershell
docker compose up -d db
$env:DATABASE_URL="postgres://ferrioxid:ferrioxid_dev_only@127.0.0.1:5432/ferrioxid"
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

The same registered application then speaks PostgreSQL. The handlers remain unchanged. Docker is an optional tool for this server, not the prerequisite for Ferrioxid.

## Production architecture (planned, not implemented)

```text
TLS-Client → Reverse Proxy → Ferrioxid-App → PostgreSQL
                                  └→ Log-/Metrics-Ziel
```

1. Multi-stage image with non-root user.
2. An app container and a separately managed PostgreSQL.
3. Readiness including database. `/healthz` currently only checks that the process responds.
4. Graceful shutdown, timeouts and adjustable connection limits.
5. Forwarded headers only from trusted proxies.
6. HTTPS on the proxy. Sessions and CSRF together with authentication.
7. Secrets about the operational environment, not in the image or repository.
8. Schema changes remain a separate step via `ferrioxid migrate up`, not at the start of the process. `down` is not part of this step.
9. No productive `allow_anonymous=true` without security review.

SQLite can carry local development and testing. For several app processes, it is not the intended production database: the pool has a connection, and write accesses serialize through the file. PostgreSQL remains the supported server backend as soon as it is operated.

## Docker, Portainer and Nginx Proxy Manager (Planned)

An internal Compose network and Nginx Proxy Manager are not implemented. Subsequent guidance must describe TLS, proxy headers and intended accessibility individually.

## Acceptance criteria

The local SQLite startup does not need a database server. An optional PostgreSQL setup remains limited to localhost. Secrets do not appear in the log. There is no app container and no automatic migration.

## Serving the admin

`/admin` renders HTML from the binary. Unknown asset paths are 404. `/admin/api` remains the JSON API. Paths with `..` are not read as files.
