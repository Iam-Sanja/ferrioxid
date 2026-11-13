# Local deployment example

This stack is for a disposable machine. It is not a production deployment.
`docker compose up` in the repository root still starts only PostgreSQL on
`127.0.0.1`. This directory is separate.

Do not point `DATABASE_URL` at a database you need to keep. The backup script
refuses a database name that does not contain `fexu_test`.

## What is here

- `Dockerfile` builds the `myshop` example. The image runs as a non-root user.
- `compose.yaml` puts PostgreSQL on an internal network, publishes the proxy
  only on `127.0.0.1:8443`, and does not publish the database port.
- `Caddyfile` terminates TLS with Caddy's local issuer and forwards to the app.
- `backup-restore.sh` dumps and restores a database whose name contains
  `fexu_test`.

The app container does not run migrations on startup. Run
`fexu migrate up` as a separate step. `/healthz` only shows that the
process is up. `/readyz` runs `SELECT 1`.

`FEXU_TRUST_PROXY=1` is not set. On this branch that flag trusts
`X-Forwarded-For` from every peer. A peer allow-list is pull request 97 and is
not included here.

No container from this file was started while the files were added if Docker
is not installed on the machine that prepared them.

## Commands

```powershell
$env:FEXU_CSRF_SECRET = "<64 hexadecimal characters>"
docker compose -f deploy/compose.yaml up --build
```

Generate the secret outside the shell history when you can. The example
password `fexu_dev_only` is a local default, not a production secret.
