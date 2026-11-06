# Konfiguration und Laufzeitumgebung

[Zur Übersicht](index.md) · **IST:** `examples/myshop/src/main.rs`, `crates/ferrum-db/src/lib.rs`, `.env.example`, `compose.yaml`.

## Tatsächliche Variablen (IST)

| Name | Einleser | Default | Verwendung |
|---|---|---|---|
| `DATABASE_URL` | `myshop/src/main.rs` | `sqlite://db.sqlite3?mode=rwc` | Verbindung. Der Dateipfad ist relativ zum Arbeitsverzeichnis. |
| `BIND_ADDRESS` | `myshop/src/main.rs` | `127.0.0.1:3000` | TCP-Listener |
| `POSTGRES_PASSWORD` | Docker Compose | `ferrum_dev_only` | Nur der optionale Postgres-Container |
| `RUST_LOG` | Tracing-Subscriber über die Umgebungsfilter-Voreinstellung `info` | `info` in `.env.example` | Log-Niveau, wenn der Prozess es exportiert sieht |

`.env.example` wird nicht automatisch geladen. Werte müssen in der Shell gesetzt sein. Eine gesetzte `DATABASE_URL` ersetzt SQLite:

```text
DATABASE_URL=postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum
```

Tests können `sqlite::memory:` verwenden. Diese URL erzeugt eine private Speicherdatenbank. `connect` setzt für jede `sqlite:`-URL `max_connections(1)` und `PRAGMA foreign_keys=ON`. Dadurch bleiben Bootstrap, Repository und HTTP-Handler auf derselben Verbindung. Das Beispielpasswort gilt nur für lokale PostgreSQL-Entwicklung. `DATABASE_URL` niemals vollständig protokollieren.

Die SQLite-Datei `db.sqlite3` sowie Journal-, WAL- und SHM-Dateien sind von Git ausgeschlossen. Wer myshop aus einem anderen Arbeitsverzeichnis startet, erhält eine andere Datei.

`ferrum.toml` ist keine Geheimnisdatei. Sie nennt `migrations_dir` und `schema_command`. Die CLI startet diesen Befehl im Verzeichnis der TOML-Datei und liest das Schema von stdout. `DATABASE_URL` bleibt die einzige Verbindungszeichenkette und wird von `migrate status` nicht gedruckt.

## Nicht implementiert (SOLL)

- Zentraler typisierter `FerrumConfig`.
- Feste Quellenreihenfolge aus Defaults, Datei, Umgebung und CLI.
- Produktionsstart, der fehlende Secrets ablehnt.
- Profile `development` und `production`.
- Einstellbare Pool-Größe, Timeouts und Read-Replicas.

## Abnahme

Der Localhost-Default bleibt `127.0.0.1:3000`. `0.0.0.0` ist keine produktionsreife Bind-Adresse. Vergleiche [deployment.md](deployment.md) und [security.md](security.md).
