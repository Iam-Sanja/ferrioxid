# ferrioxid-cli – developer commands

[Overview](index.md) · **Implemented file:** `crates/ferrioxid-cli/src/main.rs`.

Install the CLI from the framework checkout as shown in the [README](../README.md). Run project creation outside the framework workspace and subsequent commands inside the generated project.

## Actual Functionality (Implemented)

`ferrioxid doctor` and `ferrioxid check` check tools, files, both migration areas, `settings.runtime` and the mail form. The runtime test does not open a connection. `settings.runtime` becomes `ERROR` when production and debug coincide, `FERRIOXID_DEBUG` is neither `true` nor `false`, or `BIND_ADDRESS` begins with `0.0.0.0`. `disabled` does not expect an SMTP server. `console` and `memory` remain local. `smtp` is not connected. If `FERRIOXID_SMTP_PASSWORD` is missing, the `mail.credentials` warning arises without showing the value. Details are available in [cli-doctor.md](cli-doctor.md) and [migration-diagnostics.md](migration-diagnostics.md). `ferrioxid upgrade` describes [project-upgrade.md](project-upgrade.md). Generator, templates and the development server are available in [project-generator.md](project-generator.md), [project-templates.md](project-templates.md) and [development-server.md](development-server.md).

```powershell
ferrioxid new project Shop --template fullstack
cd Shop
ferrioxid new app shop
ferrioxid doctor
ferrioxid check
ferrioxid dev
ferrioxid build
ferrioxid generate model Customer --app shop
ferrioxid generate model Product --app shop --timestamps
ferrioxid worker
ferrioxid jobs list
ferrioxid generate worker
ferrioxid jobs inspect <id>
ferrioxid jobs inspect <id> --show-payload
ferrioxid migrate make <name> --app shop
ferrioxid migrate up
ferrioxid migrate status
ferrioxid dumpdata --model products
ferrioxid loaddata fixtures/catalog.json
ferrioxid loaddata fixtures/catalog.json --dry-run
ferrioxid loaddata fixtures/catalog.json --upsert
ferrioxid loaddata fixtures/catalog.json --truncate --yes
ferrioxid manage
ferrioxid manage seed-shop
ferrioxid manage about --name Shop
ferrioxid check --strict
ferrioxid upgrade --check
ferrioxid upgrade --dry-run
ferrioxid upgrade --yes
```

`migrate` reads `ferrioxid.toml` or the flags `--schema-file`, `--migrations-dir` and `--database-url`. The execution describes [migrations.md](migrations.md). `--bootstrap` belongs to the `myshop` binary, is legacy and does not perform a migration.

```powershell
ferrioxid admin create-user --email admin@example.com --staff --superuser
ferrioxid auth list-users
ferrioxid auth create-role editor
ferrioxid auth grant-permission notes.read --role editor
ferrioxid auth assign-role --email admin@example.com --role editor
```

`create-user` prompts for a password without echoing it and asks for confirmation. It accepts no password argument. `FERRIOXID_PASSWORD` can supply a password of at least 12 characters for automation; there is no default. `migrate up` applies auth migrations first. Auth commands that write data also create missing auth tables. `list-users` never prints hashes.

`generate worker` writes `src/bin/worker.rs` without overwriting an existing file. `jobs inspect` hides the payload unless `--show-payload` is supplied, which warns that it may contain personal data. Neither output includes the lease token. `ferrioxid worker` runs only the mail handler.

`ferrioxid migrate plan` compares files, history and the live catalog without writing. Exit code `1` means a history conflict or live drift. A pending migration or a model that is ahead of the latest pending file is printed as a warning and does not by itself change the exit code. `ferrioxid migrate down` reverts the latest migration. `--app <name>` reverts the latest migration of that app. `--dry-run` writes nothing. Dropping a created table or column needs `--allow-destructive` and does not restore those rows. `ferrioxid dev` starts neither a worker nor Vite, and warns about pending migrations without applying them. Generator integration tests limit nested Cargo builds to two jobs after unrestricted release builds crashed rustc 1.98.1 on Windows. The product command `ferrioxid build` still performs a release build.

Exit codes of the test: `0` without error, warnings included. `1` for conflicts, an unreadable existing database or a failed `cargo check`. `migrate status` also uses `1` if the SQLite file is missing and does not create it. `check --strict` uses `2` if only warnings remain. `upgrade` uses `1` for file conflicts and then does not write anything.

In layout 2, create an app before generating its models. Old implicit `new <name>` and `generate app` calls return migration guidance. Layout-1 root models keep working. See [migration instructions](project-generator.md#migration-from-old-clilayouts).

## CLI structure

```text
ferrioxid
├─ new project <name>          Create an empty project
├─ new app <name>              Create and register a business app
├─ doctor                      Reale Umgebungsprüfung
├─ dev                         Entwicklungsprozess starten
├─ generate model <name>       Kompilierbare Modelldatei erstellen
├─ check [--strict]            IST, Projekt und Migrationen, ohne Schreiben
├─ upgrade --check|--dry-run|--yes   IST, nur ein fehlendes Legacy-Manifest
├─ migrate make|up|down|plan|status IST, siehe migrations.md
├─ dumpdata / loaddata         IST, siehe fixtures.md
├─ manage <command>            IST, siehe management-commands.md
├─ migrate rename-column       IST, explizite Spaltenumbenennung
├─ migrate rename-table        IST, explizite Tabellenumbenennung
├─ migrate reseal              IST, Checksum einer noch nicht angewendeten Datei
├─ migrate baseline            IST nur für SQLite und nur mit --confirm
├─ migrate down [--app name]   IST, reversibel nur mit gespeichertem Snapshot
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
