# Settings

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-migrations/src/project.rs`.

## Aufgabe

`ferrum.toml` ist die einzige Projektdatei für Einstellungen. CLI, Generator und die generierte Anwendung lesen dieselben Felder. Secrets stehen nicht in der Datei.

## Zuständige Dateien

- `crates/ferrum-migrations/src/project.rs`
- `crates/ferrum-auth/src/config.rs`
- `crates/ferrum-cli/src/scaffold.rs`
- `crates/ferrum-cli/src/diagnose.rs`

## Öffentliche Schnittstelle

`ProjectConfig` enthält Projekt, Server, Datenbank, Admin, Auth, Vorlagen und Branding. `resolved_database_url` bevorzugt eine gesetzte `DATABASE_URL`. `AuthConfig::apply_project_session` übernimmt Sitzungsdauer und Leerlauf aus der Datei, sofern `FERRUM_SESSION_HOURS` oder `FERRUM_IDLE_MINUTES` nicht gesetzt sind.

## Konfiguration

Fehlende neue Schlüssel behalten Defaults: Sprache `de` mit Fallback `en`, Sitzung 12 Stunden, Leerlauf 30 Minuten, Vorlagen aus, Farbe `#0f172a`, Währung `EUR`. `branding.currency` muss drei Großbuchstaben sein. Der Admin übernimmt die öffentlichen Werte in `AdminBranding`. `GET /admin/api/config` liefert dieselbe geprüfte Teilmenge. Die HTML-Seiten lesen die Registry, nicht die Datei bei jedem Request. Secrets, Datenbank-URL und Sitzungsschlüssel gehören nicht in diese Antwort. Seit v0.12 gibt es `[static]` und `[storage]`. Ohne Abschnitt bleiben Staticfiles aus und der Speicher lokal unter `media` mit 10 MiB. `storage.backend` ist nur `local`. Präfixe dürfen Admin, API, Auth, Health und Uploads nicht überdecken. Die bisherige Workspace-Datei mit `migrations_dir` und `schema_command` bleibt gültig. Unbekannte Abschnitte und Schlüssel sind Fehler. `server.host` darf nicht `0.0.0.0` sein. `templates.engine` ist `tera`. Sprachen sind derzeit `de` und `en`.

## Datenfluss

Der Generator schreibt die Datei. `discover_project` sucht sie vom Arbeitsverzeichnis aufwärts. Die Anwendung verbindet mit `resolved_database_url` und bindet `BIND_ADDRESS` oder `host:port`.

## Fehlerfälle

Ungültige Farbe, unbekannte Sprache, zu lange Leerlaufzeit, ein absoluter Vorlagenpfad oder `..` im Vorlagenordner brechen das Lesen ab. Die Meldungen nennen den Schlüssel.

## Sicherheitsregeln

Keine Passwörter und kein CSRF-Geheimnis in `ferrum.toml`. `doctor` nennt eine gesetzte `DATABASE_URL`, zeigt den Wert aber nicht. Die Datei-URL bleibt SQLite, bis die Umgebung sie ersetzt.

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
```

## Tests

`legacy_manifest_keeps_defaults` und `full_settings_parse_and_unknown_keys_fail` in `project.rs`.

## Einschränkungen

Das eingebaute Admin-Frontend liest `site_title` noch nicht aus der Datei. Die Startseite verwendet Branding und Sprache. Weitere Sprachen sind geplant.
