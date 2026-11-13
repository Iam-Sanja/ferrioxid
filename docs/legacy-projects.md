# Existing projects

[Overview](index.md) · Upgrade: [project-upgrade.md](project-upgrade.md) · Diagnostics: [migration-diagnostics.md](migration-diagnostics.md)

A project without `.fexu/project.json` is a legacy project. That is not a mistake. `fexu check` reports `project.template.legacy` as a warning. Without `--strict`, the exit code remains 0.

## Detection

Fexu reads `Cargo.toml` as text.

- `path = "…"` is `project.dependency.path`. Relative paths, `D:/…` and `D:\…` remain.
- `git = "…"` is `project.dependency.git`.
- A version without a path is `project.dependency.registry`.
- `0.1.0` is the Cargo version of all previous functions and does not say whether auth migrations `0002_admin_journal` and `0003_audit_operation` are applied.

The `admin-ui` compatibility feature can remain enabled.

## Database

Diagnostics use `DATABASE_URL` or `database.url`. SQLite is opened read-only and only if its file already exists. Pending auth migrations appear as `migration.auth.pending`. Apply them with:

```powershell
fexu migrate up
```

This step is deliberately not part of `fexu upgrade`.

## Files

Without a manifest containing file hashes, Fexu cannot classify a file as unchanged. `fexu upgrade --yes` creates only `.fexu/project.json` with `origin: legacy` and an empty file list. Models, routes, templates, static files, migrations and custom files remain byte-for-byte unchanged.

A second `fexu upgrade --yes` finds the manifest and does not change it.

## Not automatically

These points remain manual, even after the Legacy Manifest:

- Match missing or different project files to a newer template
- Change `schema_command`, storage routes or admin registration in your own `main.rs`
- Users, roles and passwords
- already applied migration files

`fexu doctor` does not infer missing storage routes from a `[storage]` section in `fexu.toml`. It reports a missing directory as `project.file.missing` and explains that route registration cannot be determined from source code.
