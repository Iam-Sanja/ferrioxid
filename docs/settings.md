# Settings

[Zur Übersicht](index.md) · Eigene Werte: [custom-settings.md](custom-settings.md) · **IST-Datei:** `crates/ferrioxid-migrations/src/project.rs`.

## Aufgabe

`ferrum.toml` bleibt die einzige Projektdatei. `Settings` liest sie, prüft die Laufzeit und gibt `ProjectConfig` zurück. Es gibt kein globales Singleton. Secrets stehen in Umgebungsvariablen, nicht in der Datei und nicht in `Debug`.

## Priorität

1. Eingebaute Defaults, wenn ein Schlüssel fehlt.
2. Der Wert aus `ferrum.toml`.
3. Eine gesetzte Umgebungsvariable ersetzt nur den dafür vorgesehenen Wert.

Ein unbekannter Abschnitt oder Schlüssel bleibt ein Fehler. Ausnahmen sind `[custom.<name>]`, beschrieben in [custom-settings.md](custom-settings.md), und `[app.<name>]`, beschrieben in [app-registry.md](app-registry.md). `[custom]` und `[app]` ohne Namen sind Fehler. `[app.<name>]` deklariert ein Migrationsverzeichnis und lädt keinen Rust-Code. App-Einstellungen bleiben unter `[custom.<name>]` und überschreiben keine Framework-Schlüssel.

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

`discover` sucht `ferrum.toml` vom Startverzeichnis aufwärts und ruft `enforce_runtime` auf. `from_project` prüft eine bereits gelesene `ProjectConfig` erneut. `project()` leiht die Konfiguration aus. Der Generator schreibt dieselbe Folge in `src/settings.rs`.

## Bereiche

Die vorhandenen Bereiche bleiben: `project`, `server`, `database`, `admin`, `auth`, `templates`, `branding`, `static`, `storage`, `mail`, `jobs`. Fehlende neue Schlüssel behalten die bisherigen Defaults: Sprache `de` mit Fallback `en`, Sitzung 12 Stunden, Leerlauf 30 Minuten, Vorlagen aus, Farbe `#0f172a`, Währung `EUR`, Staticfiles aus, lokaler Speicher unter `media` mit 10 MiB, Mail `disabled`. `[jobs]` ist optional. Ohne Abschnitt bleibt der Worker erlaubt, startet aber nicht von selbst. Grenzen stehen in [workers.md](workers.md).

`server.host` darf nicht `0.0.0.0` sein. `templates.engine` ist `tera`. Sprachen sind `de` und `en`. `branding.currency` besteht aus drei Großbuchstaben. `storage.backend` ist `local`. Static-Präfixe dürfen Admin, API, Auth, Health und Uploads nicht überdecken.

## Umgebung

| Variable | Wirkung |
|---|---|
| `DATABASE_URL` | Ersetzt die Datei-URL, wenn sie nicht leer ist. Der Wert erscheint nicht in `Debug` |
| `FERRUM_ENV=production` | Schaltet den Produktionsmodus ein. Jeder andere oder fehlende Wert bleibt Entwicklung |
| `FERRUM_DEBUG` | Nur `true` oder `false`. Ein anderer Wert ist ein Fehler und wird nicht durch die Datei ersetzt |
| `BIND_ADDRESS` | Ersetzt `host:port`. Ein Wert, der mit `0.0.0.0` beginnt, wird abgelehnt |
| `FERRUM_PUBLIC_ORIGIN` | Ersetzt `mail.public_origin` erst beim Öffnen des Mail-Dienstes |
| `FERRUM_SMTP_USERNAME` | SMTP-Benutzer. Fehlt er, bleibt das Feld leer |
| `FERRUM_SMTP_PASSWORD` | SMTP-Passwort. Es steht nicht in `ferrum.toml` |
| `FERRUM_SESSION_HOURS` | Ersetzt die Datei, wenn gesetzt. Sonst gilt `auth.session_hours` über `apply_project_session` |
| `FERRUM_IDLE_MINUTES` | Ersetzt die Datei, wenn gesetzt. Sonst gilt `auth.idle_minutes` |
| `FERRUM_COOKIE_SECURE` | `1` oder `true` setzt das Secure-Cookie. In Produktion ist der Standard an |
| `FERRUM_CSRF_SECRET` | 64 Hex-Zeichen in Produktion. Der Wert wird nicht protokolliert |
| `FERRUM_ALLOWED_ORIGINS` | Kommagetrennte Origins. In Produktion ohne diese Variable ist die Liste leer, bis `apply_bound_address` Loopback setzt |

Eine fehlende optionale Variable behält den Datei- oder Standardwert. Eine fehlende verpflichtende Produktionsangabe bleibt ein Fehler: `FERRUM_DEBUG` mit ungültigem Text, `BIND_ADDRESS` auf `0.0.0.0`, und `server.debug = true` zusammen mit `FERRUM_ENV=production`. Ferrioxid schaltet Debug in diesem Fall nicht still auf `false`.

## Entwicklung und Produktion

Entwicklung ist der Normalfall. Generierte Projekte setzen `debug = true` und binden `127.0.0.1`. Produktion ist ausschließlich `FERRUM_ENV=production`. Dann lehnt `enforce_runtime` aktives Debug ab. `ferrioxid check` und `ferrioxid doctor` rufen dieselbe Prüfung auf und öffnen dafür keine Datenbank- und keine SMTP-Verbindung. Erfolg ist `OK settings.runtime`. Ein Fehler ist `ERROR settings.runtime` und Exitcode 1.

## Debug und Logs

`ProjectConfig` implementiert `Debug` von Hand. Die Datenbank-URL mit Benutzer und Passwort wird `schema://[redacted]@host`. Eine URL mit `password=` wird `[redacted]`. Mail-Passwörter, CSRF-Geheimnisse und Custom-Werte mit `password`, `secret`, `token` oder `credential` im Schlüsselnamen werden beim Lesen abgelehnt, ohne den Wert in die Meldung zu schreiben.

## CLI

`ferrioxid check` und `ferrioxid doctor` lesen die Datei, prüfen die Laufzeitregeln und melden eine gesetzte `DATABASE_URL`, ohne sie auszugeben. Eine statische Prüfung verbindet sich nicht.

## Beispiel

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

Die Workspace-Datei mit nur `migrations_dir` und `schema_command` bleibt gültig.

## Tests

`legacy_manifest_keeps_defaults`, `full_settings_parse_and_unknown_keys_fail` und `custom_settings_env_and_production_stay_explicit` in `project.rs`.

## Einschränkungen

`resolved_database_url` gibt den Umgebungswert an den Aufrufer zurück. Wer ihn loggt, umgeht die Schwärzung. Die HTML-Fehlerseiten der Views sind nicht die Projektvorlagen unter `templates/errors/`.
