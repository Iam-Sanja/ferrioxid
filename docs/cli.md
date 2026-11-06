# ferrum-cli – Entwicklerkommandos

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-cli/src/main.rs`.

## Tatsächlicher Funktionsumfang (IST)

Der einzige Subcommand ist `ferrum doctor`. Die Binary nutzt `clap` und druckt feste Setup-Hinweise. **Sie untersucht weder Rust-Toolchain noch Datenbankzustand** und darf daher nicht als Diagnosetool bezeichnet werden.

```bash
cargo run -p ferrum-cli -- doctor
```

Der `--bootstrap`-Parameter gehört **zur `myshop`-Binary**, nicht zur Ferrum-CLI:

```bash
cargo run -p myshop -- --bootstrap
```

Nicht existent: `ferrum new`, `ferrum dev`, `ferrum migrate`, `ferrum generate`, `ferrum build`, `ferrum admin create-user`.

## SOLL: CLI-Kommandostruktur

```text
ferrum
├─ new <name>                  Projekt-Template erstellen
├─ doctor                      Reale Umgebungsprüfung
├─ dev                         Entwicklungsprozess starten
├─ generate model <name>       Kompilierbare Modelldatei erstellen
├─ migrate make|up|down|status Migrationen verwalten
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
