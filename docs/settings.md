# Settings

[Overview](index.md) · Own values: [custom-settings.md](custom-settings.md) · **actual file:** `crates/ferrioxid-migrations/src/project.rs`.

## Purpose

`ferrioxid.toml` is the project file written for new projects. A directory that contains only `ferrum.toml` is still loaded. A directory that contains both files is an error, and Ferrioxid does not choose either database URL. `.ferrum/project.json` remains the on-disk project marker. Database tables keep their existing `_ferrum_*` names and migration IDs.

## Priority

1. Built-in defaults if a key is missing.
2. The value from `ferrioxid.toml`.
3. A set environment variable replaces only the intended value.

An unknown section or key remains an error. Exceptions are `[custom.<name>]`, described in [custom-settings.md](custom-settings.md), and `[app.<name>]`, described in [app-registry.md](app-registry.md). `[custom]` and `[app]` without names are errors. `[app.<name>]` declares a migration directory and does not load a rust code. App settings remain under `[custom.<name>]` and do not override framework keys.

## Reading

```rust
fn load() -> Result<ferrioxid::migrations::ProjectConfig, Box<dyn std::error::Error>> {
    let settings = ferrioxid::migrations::Settings::discover(std::path::Path::new("."))?;
    let project = settings.project();
    let _url = project.resolved_database_url();
    let _address = project.bind_address()?;
    Ok(project.clone())
}
```

`discover` searches `ferrioxid.toml` from the start directory upwards and calls `enforce_runtime`. `from_project` checks an already read `ProjectConfig` again. `project()` borrows the configuration. The generator writes the same sequence in `src/settings.rs`.

## Sections

Existing sections remain `project`, `server`, `database`, `admin`, `auth`, `templates`, `branding`, `static`, `storage`, `mail` and `jobs`. Omitted settings retain their defaults: language `de` with fallback `en`, 12-hour sessions, a 30-minute idle timeout, templates disabled, color `#0f172a`, currency `EUR`, static files disabled, local storage under `media` with a 10 MiB limit, and mail backend `disabled`. `[jobs]` is optional. Without it, workers are permitted but never start automatically. See [workers.md](workers.md) for limits.

`server.host` must not be `0.0.0.0`. `templates.engine` is `tera`. Languages are `de` and `en`. `branding.currency` consists of three capital letters. `storage.backend` is `local`. Static prefixes must not cover Admin, API, Auth, Health and Uploads.

## Environment

| Variable | Effect |
|---|---|
| `DATABASE_URL` |Replaces the file URL if it is not empty. The value does not appear in `Debug`|
| `FERRIOXID_ENV=production` | Turn on the production mode. `FERRUM_ENV` is the legacy alias. Different values are an error. Any other or missing value remains development |
| `FERRIOXID_DEBUG` | Only `true` or `false`. `FERRUM_DEBUG` is the legacy alias. Different values are an error and are not replaced by the file |
| `FERRIOXID_SMTP_PASSWORD` | SMTP password. `FERRUM_SMTP_PASSWORD` is the legacy alias. It is not in `ferrioxid.toml` or `ferrum.toml` |
| `FERRIOXID_CSRF_SECRET` | 64 hex characters in production. `FERRUM_CSRF_SECRET` is the legacy alias. The value is not logged |
| `BIND_ADDRESS` |Replaces `host:port`. A value starting with `0.0.0.0` is rejected|
| `FERRIOXID_PUBLIC_ORIGIN` |Replaces `mail.public_origin` only when opening the mail service|
| `FERRIOXID_SMTP_USERNAME` | SMTP user. `FERRUM_SMTP_USERNAME` is the legacy alias. If both are missing, the field remains empty |
| `FERRIOXID_SESSION_HOURS` | Replaces the file when set. `FERRUM_SESSION_HOURS` is the legacy alias |
| `FERRIOXID_IDLE_MINUTES` | Replaces the file when set. `FERRUM_IDLE_MINUTES` is the legacy alias |
| `FERRIOXID_COOKIE_SECURE` | `1` or `true` sets `Secure` on `ferrioxid_session` and `ferrioxid_anonymous`. `FERRUM_COOKIE_SECURE` is the legacy alias. In production this is the default |
| `FERRIOXID_ALLOWED_ORIGINS` | Comma-separated origins. `FERRUM_ALLOWED_ORIGINS` is the legacy alias. In production without this variable, the list is empty until `apply_bound_address` sets loopback |
| `FERRIOXID_TRUST_PROXY` | `1` or `true` allows reading `X-Forwarded-For`. `FERRUM_TRUST_PROXY` is the legacy alias. Alone it trusts no peer |
| `FERRIOXID_TRUSTED_PROXIES` | Comma-separated immediate peer addresses allowed to supply `X-Forwarded-For`. `FERRUM_TRUSTED_PROXIES` is the legacy alias. Empty means no peer is trusted. A `*` entry is ignored |

A missing optional variable retains the file or default value. `DATABASE_URL` and `BIND_ADDRESS` have no `FERRUM_` alias. For every other `FERRIOXID_` variable listed above, the matching `FERRUM_` variable is accepted. If both are set to different values, Ferrioxid stops and does not print either value. A missing mandatory production specification remains an error: `FERRIOXID_DEBUG` with invalid text, `BIND_ADDRESS` on `0.0.0.0`, and `server.debug = true` together with `FERRIOXID_ENV=production`. Ferrioxid does not switch debug silently to `false` in this case.

## Development and production

Development is the default. Generated projects set `debug = true` and bind to `127.0.0.1`. Production is selected only by `FERRIOXID_ENV=production`; `enforce_runtime` then rejects enabled debug mode. `ferrioxid check` and `ferrioxid doctor` use the same runtime validation without opening a database or SMTP connection for that check. Success is `OK settings.runtime`; failure is `ERROR settings.runtime` with exit code 1.

## Debug and logs

`ProjectConfig` implements `Debug` by hand. The database URL with user and password becomes `schema://[redacted]@host`. A URL with `password=` becomes `[redacted]`. Mail passwords, CSRF secrets and custom values with `password`, `secret`, `token` or `credential` in the key name are rejected when read without writing the value into the message.

## CLI

`ferrioxid check` and `ferrioxid doctor` read the file, check the runtime rules and report a set `DATABASE_URL` without outputting it. A static test is not combined.

## Example

```toml
[project]
name = "myshop"
template = "fullstack"
default_locale = "de"
fallback_locale = "en"

[server]
host = "127.0.0.1"
port = 3000
debug = true

[database]
url = "sqlite://db.sqlite3?mode=rwc"
```

The workspace file with only `migrations_dir` and `schema_command` remains valid.

## Tests

`legacy_manifest_keeps_defaults`, `full_settings_parse_and_unknown_keys_fail` and `custom_settings_env_and_production_stay_explicit` in `project.rs`.

## Limitations

`resolved_database_url` returns the environment value to the caller. Logging that value directly bypasses the redaction provided by `ProjectConfig::Debug`. HTML error responses from view helpers are separate from project templates under `templates/errors/`.
