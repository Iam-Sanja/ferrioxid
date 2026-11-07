# ferrum-cli – Entwicklerkommandos

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-cli/src/main.rs`.

## Tatsächlicher Funktionsumfang (IST)

`ferrum doctor` und `ferrum check` prüfen Werkzeuge, Dateien, beide Migrationsbereiche, `settings.runtime` und die Mail-Form. Die Laufzeitprüfung öffnet keine Verbindung. `settings.runtime` wird `ERROR`, wenn Produktion und Debug zusammenfallen, `FERRUM_DEBUG` weder `true` noch `false` ist oder `BIND_ADDRESS` mit `0.0.0.0` beginnt. `disabled` erwartet keinen SMTP-Server. `console` und `memory` bleiben lokal. `smtp` wird nicht verbunden. Fehlt `FERRUM_SMTP_PASSWORD`, entsteht die Warnung `mail.credentials`, ohne den Wert zu zeigen. Details stehen in [cli-doctor.md](cli-doctor.md) und [migration-diagnostics.md](migration-diagnostics.md). `ferrum upgrade` beschreibt [project-upgrade.md](project-upgrade.md). Generator, Vorlagen und der Entwicklungsserver stehen in [project-generator.md](project-generator.md), [project-templates.md](project-templates.md) und [development-server.md](development-server.md).

```powershell
cargo run -p ferrum-cli -- new myproject --template fullstack --ferrum-path .
cargo run -p ferrum-cli -- doctor
cargo run -p ferrum-cli -- check
cargo run -p ferrum-cli -- dev
cargo run -p ferrum-cli -- build
cargo run -p ferrum-cli -- generate model Customer
cargo run -p ferrum-cli -- generate model Product --timestamps
cargo run -p ferrum-cli -- worker
cargo run -p ferrum-cli -- jobs list
cargo run -p ferrum-cli -- generate app shop
cargo run -p ferrum-cli -- generate worker
cargo run -p ferrum-cli -- jobs inspect <id>
cargo run -p ferrum-cli -- jobs inspect <id> --show-payload
cargo run -p ferrum-cli -- migrate make <name>
cargo run -p ferrum-cli -- migrate up
cargo run -p ferrum-cli -- migrate status
cargo run -p ferrum-cli -- check --strict
cargo run -p ferrum-cli -- upgrade --check
cargo run -p ferrum-cli -- upgrade --dry-run
cargo run -p ferrum-cli -- upgrade --yes
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

`generate worker` schreibt `src/bin/worker.rs` und überschreibt eine vorhandene Datei nicht. `jobs inspect` verbirgt die Payload. `--show-payload` zeigt sie und weist darauf hin, dass sie personenbezogene Daten enthalten kann. Das Lease-Token bleibt in beiden Ausgaben weg. `ferrum worker` führt weiterhin nur den Mail-Handler aus.

Nicht existent: `ferrum migrate down`. `ferrum dev` startet keinen Worker. Vite startet `ferrum dev` nicht mit. Der Generator-Integrationstest begrenzt eigene Cargo-Builds auf zwei Jobs, weil ein unbegrenzter Release-Build auf Windows rustc 1.98.1 abstürzen ließ. Das Produktkommando `ferrum build` bleibt ein Release-Build. `ferrum dev` warnt vor ausstehenden Migrationen und wendet sie nicht an.

Exitcodes der Prüfung: `0` ohne Fehler, Warnungen eingeschlossen. `1` bei Konflikten, einer nicht lesbaren vorhandenen Datenbank oder einem fehlgeschlagenen `cargo check`. `migrate status` nutzt `1` auch, wenn die SQLite-Datei fehlt, und legt sie nicht an. `check --strict` nutzt `2`, wenn nur Warnungen bleiben. `upgrade` nutzt `1` bei Dateikonflikten und schreibt dann nichts.

## SOLL: CLI-Kommandostruktur

```text
ferrum
├─ new <name>                  Projekt-Template erstellen
├─ doctor                      Reale Umgebungsprüfung
├─ dev                         Entwicklungsprozess starten
├─ generate model <name>       Kompilierbare Modelldatei erstellen
├─ check [--strict]            IST, Projekt und Migrationen, ohne Schreiben
├─ upgrade --check|--dry-run|--yes   IST, nur ein fehlendes Legacy-Manifest
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
