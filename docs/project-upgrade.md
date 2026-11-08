# Project upgrade

[Zur Übersicht](index.md) · Manifest: [project-manifest.md](project-manifest.md) · Diagnose: [migration-diagnostics.md](migration-diagnostics.md)

**IST:** `crates/ferrioxid-migrations/src/upgrade.rs` and `ferrioxid upgrade`. The cargo version remains `0.1.0`. v0.21.1 does not change the writing rules: `--yes` continues to create only a missing legacy manifesto.

## What the Order Does

It checks an existing project and can create a single secure file: the missing legacy manifesto `.ferrum/project.json`. He does not recreate the project.

It does not do without express later extension:

- Apply migrations or reset the database
- Change passwords, roles or data
- Overwrite models, routes, templates, styles or migrations
- Delete unknown files
- Executing git commands
- Change an existing dependency path

`admin-ui` remains a valid compatibility feature. The code `project.dependency.admin_ui` is information, not a request for removal. The internal template version is not derived from the cargo version `0.1.0`.

## Aufruf

```powershell
ferrioxid upgrade --check
ferrioxid upgrade --dry-run
ferrioxid upgrade --yes
```

`--check` analyses dependency, manifesto and migration situation. `--dry-run` shows the same codes and the planned file changes under the heading `Geplante Dateiänderungen`. Both do not write. `--yes` together with `--check` or `--dry-run` also does not write.

Without `--yes` nothing is written. The consent is this flag, not a question.

## Plan

| Aktion | Bedeutung |
|---|---|
| `upgrade.plan.add .ferrum/project.json` |The Legacy Manifesto is missing and may be put on|
| `upgrade.unchanged` |The file corresponds to the recorded hash or the legacy manifesto already lies|
| `project.template.legacy` |Unknown origin. Existing files remain|
| `project.template.outdated` |The template version is not `0.24`. Nothing is replaced|
| `project.file.modified` |conflict. The file remains|
| `project.file.missing` |conflict. The file is not recreated|
| `project.file.unknown` |An additional file is not touched|
| `project.dependency.path` |Local path, also `D:\…` and `D:/…`|
| `project.dependency.git` |Git dependency|
| `project.dependency.registry` |Registry dependency|
| `project.dependency.unavailable` |No ferrioxide dependence in `Cargo.toml`|
| `upgrade.conflict` |Abortion, not replacement|
| `upgrade.unsafe_path` |The path leaves the project|

## Dateisicherheit

Paths must be relative and must contain only normal components. `..`, absolute paths and drive prefixes become `upgrade.unsafe_path`. If the goal or an ancestor exists, the canonical path is compared with the canonical project root. A junction or a symlink to the outside, also for `.ferrioxid`, is rejected before something is written.

The new file is created as `.ferrum/project.json.partial` and will only be renamed afterwards. If there is already a partial file, it migrates to `.ferrum/backup/<zeit>/`. If the target file is created between plan and writing, the run breaks and leaves the existing file. A termination before renaming does not leave half `project.json`.

Conflicts also prevent the creation of the manifesto. The run ends with exit code 1.

## Exitcodes

| Code | Bedeutung |
|---|---|
| 0 |Plan without file conflict. With `--yes` the manifesto was created or there was nothing to do|
| 1 |Invalid project, conflict, uncertain path or demolition|
| 2 |clap reports an invalid command line before ferrioxide builds the plan|

Pending migrations in the upgrade are a recommendation `ferrioxid migrate up`, not a spelling error and not an applied step.

## Grenze

v0.24 does not automatically migrate templates to the generator level. A project with template version `0.21` gets `project.template.outdated` and keeps its files. `src/settings.rs` and `src/urls.rs` do not apply the upgrade. A modified or foreign file remains a manual task. The command shows the code and the path. He does not write a second version and does not replace the file.
