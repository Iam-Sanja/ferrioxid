# Konfiguration und Laufzeitumgebung

[Zur Übersicht](index.md) · **IST:** `examples/myshop/src/main.rs`, `crates/ferrioxid-db/src/lib.rs`, `.env.example`, `compose.yaml`.

`FERRUM_LOCALE` darf `de` oder `en` sein und steuert die zentralen CLI-Texte. Die Admin-Sprache speichert der Browser unter `ferrioxid-locale`. Öffentliche Branding-Werte beschreibt [settings.md](settings.md). `[static]` und `[storage]` stehen in derselben Datei. `media/` enthält Uploads und gehört nicht in Git.

## Tatsächliche Variablen (IST)

| Name | Einleser | Default | Verwendung |
|---|---|---|---|
| `DATABASE_URL` | `myshop/src/main.rs` | `sqlite://db.sqlite3?mode=rwc` | Verbindung. Der Dateipfad ist relativ zum Arbeitsverzeichnis. |
| `BIND_ADDRESS` | `myshop/src/main.rs` | `127.0.0.1:3000` | TCP-Listener |
| `POSTGRES_PASSWORD` | Docker Compose | `ferrioxid_dev_only` | Nur der optionale Postgres-Container |
| `RUST_LOG` | Tracing-Subscriber über die Umgebungsfilter-Voreinstellung `info` | `info` in `.env.example` | Log-Niveau, wenn der Prozess es exportiert sieht |
| `FERRUM_ENV` | `AuthConfig::from_env` | `development` | `production` setzt `Secure` als Default und leert erlaubte Origins, wenn keine Liste gesetzt ist |
| `FERRUM_COOKIE_SECURE` | `AuthConfig::from_env` | nur in Produktion wahr | `1` oder `true` erzwingt `Secure`. Nicht von `BIND_ADDRESS` abgeleitet |
| `FERRUM_ALLOWED_ORIGINS` | `AuthConfig::from_env` | Loopback der Bind-Adresse | kommagetrennte Origins. Ist die Variable gesetzt, ersetzt sie die Liste vollständig. Sonst setzt `apply_bound_address` `http://127.0.0.1:{port}` und `http://localhost:{port}` aus `BIND_ADDRESS` oder `ferrum.toml`. Der Host-Header wird nicht übernommen. |
| `FERRUM_TRUST_PROXY` | `AuthConfig::from_env` | falsch | nur dann gilt der erste `X-Forwarded-For`-Hop |

`.env.example` wird nicht automatisch geladen. Werte müssen in der Shell gesetzt sein. Eine gesetzte `DATABASE_URL` ersetzt SQLite:

```text
DATABASE_URL=postgres://ferrioxid:ferrioxid_dev_only@127.0.0.1:5432/ferrioxid
```

Tests können `sqlite::memory:` verwenden. Diese URL erzeugt eine private Speicherdatenbank. `connect` setzt für jede `sqlite:`-URL `max_connections(1)` und `PRAGMA foreign_keys=ON`. Dadurch bleiben Bootstrap, Repository und HTTP-Handler auf derselben Verbindung. Das Beispielpasswort gilt nur für lokale PostgreSQL-Entwicklung. `DATABASE_URL` niemals vollständig protokollieren.

Die SQLite-Datei `db.sqlite3` sowie Journal-, WAL- und SHM-Dateien sind von Git ausgeschlossen. Wer myshop aus einem anderen Arbeitsverzeichnis startet, erhält eine andere Datei.

Die vollständige Settings-Beschreibung steht in [settings.md](settings.md). `ferrum.toml` ist keine Geheimnisdatei. Die Workspace-Datei nennt weiterhin nur `migrations_dir` und `schema_command`. Generierte Projekte dürfen zusätzlich `[project]`, `[database]`, `[server]` und `[admin]` enthalten. Unbekannte Abschnitte und Schlüssel sind Fehler. `database.url` ist standardmäßig `sqlite://db.sqlite3?mode=rwc`, `server.host` ist `127.0.0.1`, `server.port` ist 3000. `DATABASE_URL` ersetzt die Datei-URL zur Laufzeit und wird von `migrate status` nicht gedruckt. Die CLI startet `schema_command` im Verzeichnis der TOML-Datei und liest das Schema von stdout.

`migrate baseline` liest dieselbe URL. Ohne `--confirm` schreibt der Befehl nichts. Für PostgreSQL endet er mit einer Unsupported-Meldung. Spalten- und Tabellennamen für `rename-column` und `rename-table` kommen von den CLI-Argumenten und werden als Bezeichner geprüft. Sie werden nicht aus HTTP-Anfragen übernommen.

## Nicht implementiert (SOLL)

- Zentraler typisierter `FerrioxidConfig`.
- Feste Quellenreihenfolge aus Defaults, Datei, Umgebung und CLI.
- Produktionsstart, der fehlende Secrets ablehnt.
- Profile `development` und `production`.
- Einstellbare Pool-Größe, Timeouts und Read-Replicas.

## Abnahme

Der Localhost-Default bleibt `127.0.0.1:3000`. `0.0.0.0` ist keine produktionsreife Bind-Adresse. Vergleiche [deployment.md](deployment.md) und [security.md](security.md).

## Admin

`FERRUM_ADMIN_DIST` wird nicht mehr gelesen. CSS und Vorlagen kommen aus dem Binary. `FERRUM_CSRF_SECRET` ist optional und muss 64 Hex-Zeichen lang sein. Ohne die Variable verwendet Ferrioxid `.ferrum/csrf.key`. `FERRUM_COOKIE_SECURE` und `FERRUM_ENV=production` gelten für das Admin-Cookie genauso wie für die übrige Sitzung. Theme und Sprache liegen in den Cookies `ferrioxid_theme` und `ferrioxid_locale`.
