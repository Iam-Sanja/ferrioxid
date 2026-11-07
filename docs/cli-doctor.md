# Diagnose und Projektprüfung

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-cli/src/diagnose.rs`.

## Aufgabe

`ferrum doctor` prüft die Werkzeugkette und das Projekt. `ferrum check` prüft Konfiguration, Migrationen und die Übersetzung, ohne Daten zu ändern.

## Zuständige Dateien

- `crates/ferrum-cli/src/diagnose.rs`
- `crates/ferrum-migrations/src/project.rs`

## Öffentliche Schnittstelle

```powershell
ferrum doctor
ferrum check
```

Statuszeilen sind `[ok]`, `[warnung]` und `[fehler]`. Ein Fehler ergibt Exit-Code 1.

## Konfiguration

Gelesen wird `ferrum.toml`. `DATABASE_URL` darf den Datenbankeintrag ersetzen. `check` lehnt eine leere `DATABASE_URL` ab. Ist `FERRUM_CSRF_SECRET` gesetzt, muss es 64 Hex-Zeichen haben. Ein fehlender Wert ist hier kein Fehler, weil die Laufzeit einen Entwicklungsschlüssel erzeugen kann.

## Datenfluss

`doctor` ruft `rustc` und `cargo` mit `--version` auf. Danach sucht es `ferrum.toml`, `Cargo.toml`, `src/main.rs`, `src/lib.rs` und das Migrationsverzeichnis. SQLite wird als konfiguriert gemeldet. Eine noch nicht vorhandene Datei ist eine Warnung und wird nicht angelegt. PostgreSQL ist eine Warnung ohne Verbindung. Bei einem Fullstack-Projekt meldet `doctor`, dass der Admin im Rust-Binary liegt. Node.js wird nicht verlangt.

`check` prüft die Umgebung, liest die Migrationsdateien und ihre Prüfsummen und führt `cargo check` aus. Es ruft `migrate up` nicht auf und löscht nichts.

## Fehlerfälle

| Fall | Status |
|---|---|
| `rustc` oder `cargo` fehlt | Fehler |
| `ferrum.toml` oder `src/lib.rs` fehlt | Fehler |
| `server.host` ist `0.0.0.0` | Fehler |
| SQLite-Datei fehlt noch | Warnung |
| Docker fehlt | kein Fehler; Docker ist optional und wird nicht geprüft |
| Prüfsummenkonflikt | Fehler in `check` |

`doctor` im Ferrum-Workspace selbst meldet fehlende `src/main.rs` und `src/lib.rs`, weil das Workspace-Root keine Anwendung ist. Der Befehl gilt für generierte Projekte und für `examples/myshop` nur, wenn dort eine eigene `ferrum.toml` liegt. Die Workspace-Datei bleibt gültig für `migrate`.

## Sicherheitsregeln

Verbindungszeichenketten werden nicht ausgegeben. `doctor` öffnet keine PostgreSQL-Verbindung und legt keine SQLite-Datei an. `check` schreibt keine Migration.

## Beispiele

```powershell
cd demo
ferrum doctor
ferrum check
```

Dieselbe Aufrufform gilt unter Linux in der jeweiligen Shell.

## Tests

`generator.rs` erwartet Erfolg in einem frischen API-Projekt und einen Fehler, nachdem `src/lib.rs` entfernt wurde. Dieselbe Datei vergleicht die Migrationsbytes vor und nach `ferrum check`.

## Einschränkungen

Eine echte SQLite-Verbindung wird nicht geöffnet, damit `doctor` keine Datei anlegt. Schemavergleich gegen eine laufende Datenbank gehört zu `migrate status`, nicht zu `check`.
