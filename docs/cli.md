# ferrioxid-cli – Entwicklerkommandos

[For overview](index.md) · **IST file:** `crates/ferrioxid-cli/src/main.rs`.]

## Actual Functionality (IST)

`ferrioxid doctor` and `ferrioxid check` check tools, files, both migration areas, `settings.runtime` and the mail form. The runtime test does not open a connection. `settings.runtime` becomes `ERROR` when production and debug coincide, `FERRUM_DEBUG` is neither `true` nor `false`, or `BIND_ADDRESS` begins with `0.0.0.0`. `disabled` does not expect an SMTP server. `console` and `memory` remain local. `smtp` is not connected. If `FERRUM_SMTP_PASSWORD` is missing, the `mail.credentials` warning arises without showing the value. Details are available in [cli-doctor.md](cli-doctor.md) and [migration-diagnostics.md](migration-diagnostics.md)]. `ferrioxid upgrade` describes [project-upgrade.md](project-upgrade.md)]. Generator, templates and the development server are available in [project-generator.md](project-generator.md), [project-templates.md](project-templates.md) and [development-server.md](development-server.md)].

```powershell
cargo run -p ferrioxid-cli -- new myproject --template fullstack --ferrioxid-path .
cargo run -p ferrioxid-cli -- doctor
cargo run -p ferrioxid-cli -- check
cargo run -p ferrioxid-cli -- dev
cargo run -p ferrioxid-cli -- build
cargo run -p ferrioxid-cli -- generate model Customer
cargo run -p ferrioxid-cli -- generate model Product --timestamps
cargo run -p ferrioxid-cli -- worker
cargo run -p ferrioxid-cli -- jobs list
cargo run -p ferrioxid-cli -- generate app shop
cargo run -p ferrioxid-cli -- generate worker
cargo run -p ferrioxid-cli -- jobs inspect <id>
cargo run -p ferrioxid-cli -- jobs inspect <id> --show-payload
cargo run -p ferrioxid-cli -- migrate make <name>
cargo run -p ferrioxid-cli -- migrate up
cargo run -p ferrioxid-cli -- migrate status
cargo run -p ferrioxid-cli -- check --strict
cargo run -p ferrioxid-cli -- upgrade --check
cargo run -p ferrioxid-cli -- upgrade --dry-run
cargo run -p ferrioxid-cli -- upgrade --yes
```

`migrate` reads `ferrum.toml` or the flags `--schema-file`, `--migrations-dir` and `--database-url`. The execution describes [migrations.md](migrations.md)]. `--bootstrap` belongs to the `myshop` binary, is legacy and does not perform a migration.

```powershell
cargo run -p ferrioxid-cli -- admin create-user --email admin@example.com --superuser
cargo run -p ferrioxid-cli -- auth list-users
cargo run -p ferrioxid-cli -- auth create-role editor
cargo run -p ferrioxid-cli -- auth grant-permission notes.read --role editor
cargo run -p ferrioxid-cli -- auth assign-role --email admin@example.com --role editor
```

`create-user` reads the password hidden and requires repetition. It does not accept a password argument. If `FERRUM_PASSWORD` is set, the value must have at least 12 characters. There is no standard password. `migrate up` first applies the Auth migrations. Writing Auth commands also create the Auth tables if they are missing. `list-users` does not show hashes.

`generate worker` writes `src/bin/worker.rs` and does not overwrite an existing file. `jobs inspect` hides the payload. `--show-payload` shows it and points out that it may contain personal data. The lease token stays away in both issues. `ferrioxid worker` still only executes the mail handler.

Non-existent: `ferrioxid migrate down`. `ferrioxid dev` does not start a worker. Vite does not start `ferrioxid dev`. The Generator integration test limits own cargo builds to two jobs because an unlimited release build on Windows rustc crashed 1.98.1. The product command `ferrioxid build` remains a release build. `ferrioxid dev` warns against pending migrations and does not apply them.

Exit codes of the test: `0` without error, warnings included. `1` for conflicts, an unreadable existing database or a failed `cargo check`. `migrate status` also uses `1` if the SQLite file is missing and does not create it. `check --strict` uses `2` if only warnings remain. `upgrade` uses `1` for file conflicts and then does not write anything.

## SOLL: CLI-Kommandostruktur

```text
ferrioxid
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

### Implementation guidelines (reference)

- Parsing with `clap` subcommands; Exit code 0 only if successful.
- `new`, `generate model`, `doctor`, `check`, `dev` and `build` are implemented. `new` does not overwrite an existing directory.
- `doctor` checks Cargo and the URL schema and does not create a database file.
- `dev` ends the child process at Ctrl+C. A crash does not restart itself.
- Migrations are not aliases of `--bootstrap`.
- CLI outputs with precise paths and repairable error messages.

## Testmatrix (SOLL)

- `--help` and `--version` work.
- Unknown Command: Exit != 0.
- Project goal already present: no overwriting.
- Doctor without DB: Fake relevant diagnosis instead of success.
- Database errors do not contain passwords.

For migrations see [migrations.md](migrations.md)].
