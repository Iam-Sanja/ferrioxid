# ferrum-cli – Entwicklerkommandos

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-cli/src/main.rs`.

## Tatsächlicher Funktionsumfang (IST)

`ferrum doctor` druckt den Funktionsumfang. Es prüft weder Toolchain noch eine live Datenbank.

```powershell
cargo run -p ferrum-cli -- doctor
cargo run -p ferrum-cli -- migrate make <name>
cargo run -p ferrum-cli -- migrate up
cargo run -p ferrum-cli -- migrate status
```

`migrate` liest `ferrum.toml` oder die Flags `--schema-file`, `--migrations-dir` und `--database-url`. Die Ausführung beschreibt [migrations.md](migrations.md). `--bootstrap` gehört zur `myshop`-Binary, ist Legacy und führt keine Migration aus.

Nicht existent: `ferrum new`, `ferrum dev`, `ferrum generate`, `ferrum build`, `ferrum migrate down`, `ferrum admin create-user`.

## SOLL: CLI-Kommandostruktur

```text
ferrum
├─ new <name>                  Projekt-Template erstellen
├─ doctor                      Reale Umgebungsprüfung
├─ dev                         Entwicklungsprozess starten
├─ generate model <name>       Kompilierbare Modelldatei erstellen
├─ migrate make|up|status      IST, siehe migrations.md
├─ migrate down                nicht implementiert
└─ build                       Definierter Release-Build
```

### Implementierungsrichtlinien (SOLL)

- Parsing mit `clap`-Subcommands; Exit-Code 0 nur bei Erfolg.
- `new` erzeugt in Zielpfad nur nach Kollisionsprüfung; niemals vorhandene Nutzerdaten überschreiben.
- `generate model` schreibt einen überprüfbaren Draft, ergänzt Registrierungen deterministisch, macht bei Konflikten keine stillen Änderungen.
- `doctor` prüft tatsächlich Cargo, DB-URL-Format und ggf. DB-Verbindung; zeigt keine Secrets.
- `dev` definiert Start/Stop- und Signalverhalten; Watch-Modus erst bei nachgewiesener Zuverlässigkeit.
- Migrationen sind **kein** Alias von `--bootstrap`.
- CLI-Ausgaben mit präzisen Pfaden und reparierbaren Fehlermeldungen.

## Testmatrix (SOLL)

- `--help` und `--version` funktionieren.
- unbekannter Command: Exit != 0.
- Projektziel bereits vorhanden: kein Überschreiben.
- Doctor ohne DB: relevante Diagnose statt Erfolg vortäuschen.
- Datenbankfehler enthalten keine Passwörter.

Für Migrationen siehe [migrations.md](migrations.md).
