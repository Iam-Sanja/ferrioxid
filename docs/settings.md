# Settings

[For overview](index.md) · Own values: [custom-settings.md](custom-settings.md) · **actual file:** `crates/ferrioxid-migrations/src/project.rs`].

## Aufgabe

`ferrum.toml` remains the only project file. `Settings` reads it, checks the runtime and returns `ProjectConfig`. There is no global singleton. Secrets are in environment variables, not in the file and not in `Debug`.

## Priority

1. Built-in defaults if a key is missing.
2. The value from `ferrum.toml`.
3. A set environment variable replaces only the intended value.

An unknown section or key remains an error. Exceptions are `[custom.<name>]`, described in [custom-settings.md](custom-settings.md), and `[app.<name>]`, described in [app-registry.md](app-registry.md). `[custom]` and `[app]` without names are errors]. `[app.<name>]` declares a migration directory and does not load a rust code. App settings remain under `[custom.<name>]` and do not override framework keys.

## Lesen

```rust
fn load() -> Result<ferrioxid::migrations::ProjectConfig, Box<dyn std::error::Error>> {
    let settings = ferrioxid::migrations::Settings::discover(std::path::Path::new("."))?;
    let project = settings.project();
    let _url = project.resolved_database_url();
    let _address = project.bind_address()?;
    Ok(project.clone())
}
```

`discover` searches `ferrum.toml` from the start directory upwards and calls `enforce_runtime`. `from_project` checks an already read `ProjectConfig` again. `project()` borrows the configuration. The generator writes the same sequence in `src/settings.rs`.

## Bereiche

Die vorhandenen Bereiche bleiben: `project`, `server`, `database`, `admin`, `auth`, `templates`, `branding`, `static`, `storage`, `mail`, `jobs`. Fehlende neue Schlüssel behalten die bisherigen Defaults: Sprache `de` mit Fallback `en`, Sitzung 12 Stunden, Leerlauf 30 Minuten, Vorlagen aus, Farbe `#0f172a`, Währung `EUR`, Staticfiles aus, lokaler Speicher unter `media` mit 10 MiB, Mail `disabled`. `[jobs]` ist optional. Ohne Abschnitt bleibt der Worker erlaubt, startet aber nicht von selbst. Grenzen stehen in [workers.md](workers.md).

`server.host` must not be `0.0.0.0`. `templates.engine` is `tera`. Languages are `de` and `en`. `branding.currency` consists of three capital letters. `storage.backend` is `local`. Static prefixes must not cover Admin, API, Auth, Health and Uploads.

## Umgebung

| Variable | Wirkung |
|---|---|
| `DATABASE_URL` |Replaces the file URL if it is not empty. The value does not appear in `Debug`|
| `FERRUM_ENV=production` |Turn on the production mode. Any other or missing value remains development|
| `FERRUM_DEBUG` |Only `true` or `false`. Another value is an error and is not replaced by the file|
| `BIND_ADDRESS` |Replaces `host:port`. A value starting with `0.0.0.0` is rejected|
| `FERRUM_PUBLIC_ORIGIN` |Replaces `mail.public_origin` only when opening the mail service|
| `FERRUM_SMTP_USERNAME` |SMTP users. If it is missing, the field remains empty|
| `FERRUM_SMTP_PASSWORD` |SMTP password. It is not in `ferrum.toml`|
| `FERRUM_SESSION_HOURS` |Replaces the file when set. Otherwise, `auth.session_hours` applies over `apply_project_session`|
| `FERRUM_IDLE_MINUTES` |Replaces the file when set. Otherwise `auth.idle_minutes` applies|
| `FERRUM_COOKIE_SECURE` |`1` or `true` sets the Secure cookie. In production, the standard|
| `FERRUM_CSRF_SECRET` |64 Hex marks in production. The value is not logged|
| `FERRUM_ALLOWED_ORIGINS` |Comage-separated origins. In production without this variable, the list is empty until `apply_bound_address` sets loopback|

A missing optional variable retains the file or default value. A missing mandatory production specification remains an error: `FERRUM_DEBUG` with invalid text, `BIND_ADDRESS` on `0.0.0.0`, and `server.debug = true` together with `FERRUM_ENV=production`. Ferrioxide does not switch debug silently to `false` in this case.

## Development and production

Entwicklung ist der Normalfall. Generierte Projekte setzen `debug = true` und binden `127.0.0.1`. Produktion ist ausschließlich `FERRUM_ENV=production`. Dann lehnt `enforce_runtime` aktives Debug ab. `ferrioxid check` und `ferrioxid doctor` rufen dieselbe Prüfung auf und öffnen dafür keine Datenbank- und keine SMTP-Verbindung. Erfolg ist `OK settings.runtime`. Ein Fehler ist `ERROR settings.runtime` und Exitcode 1.

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

`resolved_database_url` returns the ambient value to the caller. Whoever lied to him avoided blackening. The HTML error pages of the views are not the project templates under `templates/errors/`.
