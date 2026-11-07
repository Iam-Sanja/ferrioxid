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

Statuszeilen sind `[OK]`, `[WARN]` und `[ERROR]`. Ein Fehler ergibt Exitcode 1. `check --strict` ergibt Exitcode 2, wenn nur Warnungen bleiben. Ohne `--strict` bleiben Warnungen bei Exitcode 0.

## Konfiguration

Gelesen wird `ferrum.toml`. `DATABASE_URL` darf den Datenbankeintrag ersetzen. `check` lehnt eine leere `DATABASE_URL` ab. Ist `FERRUM_CSRF_SECRET` gesetzt, muss es 64 Hex-Zeichen haben. Ein fehlender Wert ist hier kein Fehler, weil die Laufzeit einen Entwicklungsschlüssel erzeugen kann.

## Datenfluss

`doctor` ruft `rustc` und `cargo` mit `--version` auf. Danach sucht es `ferrum.toml`, `Cargo.toml`, `src/main.rs`, `src/lib.rs` und das Migrationsverzeichnis. Eine vorhandene Datenbank wird nur gelesen. Eine noch nicht vorhandene SQLite-Datei ist eine Warnung und wird nicht angelegt. PostgreSQL wird lesend verbunden, wenn die URL darauf zeigt. Ein Fehlschlag ist ein Fehler. Bei einem Fullstack-Projekt meldet `doctor`, dass der Admin im Rust-Binary liegt. Node.js wird nicht verlangt. Storage-Routen werden nicht aus `main.rs` geschlossen.

`check` prüft die Umgebung, beide Migrationshistorien und führt `cargo check` aus. Liegt die Datenbankdatei vor und ist `schema_command` gesetzt, vergleicht es das Modell. `FERRUM_CHECK_SKIP_SCHEMA=1` und `FERRUM_CHECK_SKIP_CARGO=1` überspringen diese Schritte in Tests. Es ruft `migrate up` nicht auf und löscht nichts.

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

Verbindungszeichenketten werden nicht ausgegeben. `doctor` und `check` legen keine SQLite-Datei und keine Historientabelle an. `check` schreibt keine Migration.

## Beispiele

```powershell
cd demo
ferrum doctor
ferrum check
ferrum check --strict
```

Dieselbe Aufrufform gilt unter Linux in der jeweiligen Shell.

## Tests

`generator.rs` erwartet Erfolg in einem frischen API-Projekt und einen Fehler, nachdem `src/lib.rs` entfernt wurde. Dieselbe Datei vergleicht die Migrationsbytes vor und nach `ferrum check`.

## Einschränkungen

Eine fehlende SQLite-Datei wird nicht angelegt. Existiert sie, wird sie mit `mode=ro` gelesen. `doctor` startet `schema_command` nicht. `check` und `migrate status` vergleichen das Modell, wenn die Datei vorhanden ist und ein Schema geladen werden kann. Der gemeinsame Bericht steht in [migration-diagnostics.md](migration-diagnostics.md).
