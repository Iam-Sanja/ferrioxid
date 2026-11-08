# ferrioxid-cli – developer commands

[Overview](index.md) · **Implemented file:** `crates/ferrioxid-cli/src/main.rs`.

## Actual Functionality (Implemented)

`ferrioxid doctor` and `ferrioxid check` check tools, files, both migration areas, `settings.runtime` and the mail form. The runtime test does not open a connection. `settings.runtime` becomes `ERROR` when production and debug coincide, `FERRUM_DEBUG` is neither `true` nor `false`, or `BIND_ADDRESS` begins with `0.0.0.0`. `disabled` does not expect an SMTP server. `console` and `memory` remain local. `smtp` is not connected. If `FERRUM_SMTP_PASSWORD` is missing, the `mail.credentials` warning arises without showing the value. Details are available in [cli-doctor.md](cli-doctor.md) and [migration-diagnostics.md](migration-diagnostics.md). `ferrioxid upgrade` describes [project-upgrade.md](project-upgrade.md). Generator, templates and the development server are available in [project-generator.md](project-generator.md), [project-templates.md](project-templates.md) and [development-server.md](development-server.md).

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

`migrate` reads `ferrum.toml` or the flags `--schema-file`, `--migrations-dir` and `--database-url`. The execution describes [migrations.md](migrations.md). `--bootstrap` belongs to the `myshop` binary, is legacy and does not perform a migration.

```powershell
cargo run -p ferrioxid-cli -- admin create-user --email admin@example.com --superuser
cargo run -p ferrioxid-cli -- auth list-users
cargo run -p ferrioxid-cli -- auth create-role editor
cargo run -p ferrioxid-cli -- auth grant-permission notes.read --role editor
cargo run -p ferrioxid-cli -- auth assign-role --email admin@example.com --role editor
```

`create-user` prompts for a password without echoing it and asks for confirmation. It accepts no password argument. `FERRUM_PASSWORD` can supply a password of at least 12 characters for automation; there is no default. `migrate up` applies auth migrations first. Auth commands that write data also create missing auth tables. `list-users` never prints hashes.

`generate worker` writes `src/bin/worker.rs` without overwriting an existing file. `jobs inspect` hides the payload unless `--show-payload` is supplied, which warns that it may contain personal data. Neither output includes the lease token. `ferrioxid worker` runs only the mail handler.

`ferrioxid migrate down` is not implemented. `ferrioxid dev` starts neither a worker nor Vite, and warns about pending migrations without applying them. Generator integration tests limit nested Cargo builds to two jobs after unrestricted release builds crashed rustc 1.98.1 on Windows. The product command `ferrioxid build` still performs a release build.

Exit codes of the test: `0` without error, warnings included. `1` for conflicts, an unreadable existing database or a failed `cargo check`. `migrate status` also uses `1` if the SQLite file is missing and does not create it. `check --strict` uses `2` if only warnings remain. `upgrade` uses `1` for file conflicts and then does not write anything.

## Planned CLI structure

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

### Implementation guidelines (planned)

- Parsing with `clap` subcommands; Exit code 0 only if successful.
- `new`, `generate model`, `doctor`, `check`, `dev` and `build` are implemented. `new` does not overwrite an existing directory.
- `doctor` checks Cargo and the URL scheme without creating a database file.
- `dev` ends the child process at Ctrl+C. A crash does not restart itself.
- Migrations are not aliases of `--bootstrap`.
- CLI outputs with precise paths and repairable error messages.

## Planned test matrix

- `--help` and `--version` work.
- Unknown Command: Exit != 0.
- Existing project directory: refuse to overwrite it.
- Doctor without a database: report the relevant diagnostic rather than claiming success.
- Database errors do not contain passwords.

For migrations see [migrations.md](migrations.md).
