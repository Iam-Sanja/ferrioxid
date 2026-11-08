# Bestandsprojekte

[Zur Übersicht](index.md) · Upgrade: [project-upgrade.md](project-upgrade.md) · Diagnose: [migration-diagnostics.md](migration-diagnostics.md)

A project without `.ferrum/project.json` is a legacy project. That is not a mistake. `ferrioxid check` reports `project.template.legacy` as a warning. Without `--strict`, the exit code remains 0.

## Erkennen

Ferrioxid liest `Cargo.toml` als Text.

- `path = "…"` is `project.dependency.path`. Relative paths, `D:/…` and `D:\…` remain.
- `git = "…"` is `project.dependency.git`.
- A version without a path is `project.dependency.registry`.
- `0.1.0` is the cargo version of all previous functions and does not say whether authentic migrations `0002_admin_journal` and `0003_audit_operation` are applied.

`admin-ui` darf bleiben.

## Datenbank

The diagnosis uses `DATABASE_URL` or `database.url`. It opens SQLite only when it reads and only when the file already exists. Missing Auth migrations appear as `migration.auth.pending`. The repair remains:

```powershell
ferrioxid migrate up
```

This step is deliberately not part of `ferrioxid upgrade`.

## Dateien

As long as no manifest with hashes exists, ferrioxide does not classify any file as unchanged. `ferrioxid upgrade --yes` only writes `.ferrum/project.json` with `origin: legacy` and an empty file list. Models, routes, templates, static files, migrations and own files remain byte-like.

A second `ferrioxid upgrade --yes` finds the manifest and does not change it.

## Not automatically

These points remain manual, even after the Legacy Manifesto:

- Match missing or different project files to a newer template
- Change `schema_command`, storage routes or admin registration in your own `main.rs`
- Users, roles and passwords
- already applied migration files

`ferrioxid doctor` does not claim that storage routes are missing just because `[storage]` is in `ferrum.toml`. It reports a missing directory as `project.file.missing` and says that the routes are not closed from the source code.
