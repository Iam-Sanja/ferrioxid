# Docker bind address, public origin and trusted proxies

Three settings are independent.

| Setting | Meaning | Default |
| --- | --- | --- |
| `server.host` and `BIND_ADDRESS` | Socket the process binds. | `127.0.0.1:3000`. |
| `FERRIOXID_ALLOWED_ORIGINS` | Exact browser origins accepted by CSRF and login. | Development loopback origins. Production starts empty. |
| `FERRIOXID_TRUST_PROXY` and `FERRIOXID_TRUSTED_PROXIES` | Which immediate peers may supply `X-Forwarded-For` for the client key. | Off, and an empty peer list. |

A wildcard bind is an explicit opt-in. Set `server.allow_wildcard_bind = true` in `ferrioxid.toml`, or set `FERRIOXID_ALLOW_WILDCARD_BIND=true` when `BIND_ADDRESS` is `0.0.0.0:port` or `[::]:port`. The bind address is not added to the origin allowlist. `Host`, `X-Forwarded-Host` and `X-Forwarded-Proto` are not public origins.

`FERRIOXID_TRUST_PROXY=true` alone does not trust forwarded headers. The immediate peer address must also be listed in `FERRIOXID_TRUSTED_PROXIES`. A `*` entry is ignored. `Origin: null` is rejected unless the browser also sends `Sec-Fetch-Site: same-origin`.

The example in `examples/deploy` is a multi-stage image. The runtime stage copies only `embedded-migrate`. Auth and job migration JSON is compiled into that binary, so the image does not need the Rust source tree or those JSON files. PostgreSQL has no published host port. Caddy publishes HTTPS on `127.0.0.1:443` only.

`embedded-migrate` proves the migration contract. A full application image must also copy the application's own templates, static files and upload directory when those are not embedded. Application migration directories remain files beside the application; an absent application directory is optional and is not reported as a successful framework migration.

Windows and Linux use the same embedded resources. CI copies the built binary to a temporary directory and runs it with a fresh SQLite file. The PostgreSQL 16 job runs the same binary against `ferrioxid_test_relocated`, then drops that database. Local PostgreSQL 18 checks use `ferrioxid::db::DisposablePostgres` and are ignored unless `DATABASE_URL` names an approved admin database.
