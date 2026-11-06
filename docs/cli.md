# ferrum-cli – Entwicklerkommandos

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-cli/src/main.rs`.

## Tatsächlicher Funktionsumfang (IST)

`ferrum doctor` und `ferrum check` prüfen Werkzeuge, Dateien und Migrationen. Details stehen in [cli-doctor.md](cli-doctor.md). Generator, Vorlagen und der Entwicklungsserver stehen in [project-generator.md](project-generator.md), [project-templates.md](project-templates.md) und [development-server.md](development-server.md).

```powershell
cargo run -p ferrum-cli -- new myproject --template fullstack --ferrum-path .
cargo run -p ferrum-cli -- doctor
cargo run -p ferrum-cli -- check
cargo run -p ferrum-cli -- dev
cargo run -p ferrum-cli -- build
cargo run -p ferrum-cli -- generate model Customer
cargo run -p ferrum-cli -- migrate make <name>
cargo run -p ferrum-cli -- migrate up
cargo run -p ferrum-cli -- migrate status
```

`migrate` liest `ferrum.toml` oder die Flags `--schema-file`, `--migrations-dir` und `--database-url`. Die Ausführung beschreibt [migrations.md](migrations.md). `--bootstrap` gehört zur `myshop`-Binary, ist Legacy und führt keine Migration aus.

```powershell
cargo run -p ferrum-cli -- admin create-user --email admin@example.com --superuser
cargo run -p ferrum-cli -- auth list-users
cargo run -p ferrum-cli -- auth create-role editor
cargo run -p ferrum-cli -- auth grant-permission notes.read --role editor
cargo run -p ferrum-cli -- auth assign-role --email admin@example.com --role editor
```

`create-user` liest das Passwort verdeckt und verlangt die Wiederholung. Es akzeptiert kein Passwortargument. Ist `FERRUM_PASSWORD` gesetzt, muss der Wert mindestens 12 Zeichen haben. Es gibt kein Standardpasswort. `migrate up` wendet zuerst die Auth-Migrationen an. Schreibende Auth-Befehle legen die Auth-Tabellen ebenfalls an, wenn sie fehlen. `list-users` zeigt keine Hashes.

Nicht existent: `ferrum migrate down`. Vite startet `ferrum dev` nicht mit.

## SOLL: CLI-Kommandostruktur

```text
ferrum
├─ new <name>                  Projekt-Template erstellen
├─ doctor                      Reale Umgebungsprüfung
├─ dev                         Entwicklungsprozess starten
├─ generate model <name>       Kompilierbare Modelldatei erstellen
├─ migrate make|up|status      IST, siehe migrations.md
├─ migrate rename-column       IST, explizite Spaltenumbenennung
├─ migrate rename-table        IST, explizite Tabellenumbenennung
├─ migrate reseal              IST, Checksum einer noch nicht angewendeten Datei
├─ migrate baseline            IST nur für SQLite und nur mit --confirm
├─ migrate down                nicht implementiert
└─ build                       Definierter Release-Build
```

### Implementierungsrichtlinien (SOLL)

- Parsing mit `clap`-Subcommands; Exit-Code 0 nur bei Erfolg.
- `new`, `generate model`, `doctor`, `check`, `dev` und `build` sind implementiert. `new` überschreibt kein vorhandenes Verzeichnis.
- `doctor` prüft Cargo und das URL-Schema und legt keine Datenbankdatei an.
- `dev` beendet den Kindprozess bei Strg+C. Ein Absturz startet nicht von selbst neu.
- Migrationen sind **kein** Alias von `--bootstrap`.
- CLI-Ausgaben mit präzisen Pfaden und reparierbaren Fehlermeldungen.

## Testmatrix (SOLL)

- `--help` und `--version` funktionieren.
- unbekannter Command: Exit != 0.
- Projektziel bereits vorhanden: kein Überschreiben.
- Doctor ohne DB: relevante Diagnose statt Erfolg vortäuschen.
- Datenbankfehler enthalten keine Passwörter.

Für Migrationen siehe [migrations.md](migrations.md).
