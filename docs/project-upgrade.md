# Project upgrade

[Overview](index.md) · Manifest: [project-manifest.md](project-manifest.md) · Diagnostics: [migration-diagnostics.md](migration-diagnostics.md)

**Implemented:** `crates/fexu-migrations/src/upgrade.rs` and `fexu upgrade`. The Cargo version remains `0.1.0`. v0.21.1 does not change the writing rules: `--yes` continues to create only a missing legacy manifest.

## What the command does

The command checks an existing project and can create a missing legacy manifest at `.fexu/project.json` without regenerating the project.

The following operations are outside the command’s scope:

- Apply migrations or reset the database
- Change passwords, roles or data
- Overwrite models, routes, templates, styles or migrations
- Delete unknown files
- Executing git commands
- Change an existing dependency path

`admin-ui` remains a valid compatibility feature. The code `project.dependency.admin_ui` is information, not a request for removal. The internal template version is not derived from the Cargo version `0.1.0`.

## Usage

```powershell
fexu upgrade --check
fexu upgrade --dry-run
fexu upgrade --yes
```

`--check` analyses dependency, manifest and migration situation. `--dry-run` shows the same codes and the planned file changes under the heading `Geplante Dateiänderungen`. Both do not write. `--yes` together with `--check` or `--dry-run` also does not write.

Without `--yes` nothing is written. The consent is this flag, not a question.

## Plan

| Action | Meaning |
|---|---|
| `upgrade.plan.add .fexu/project.json` | The legacy manifest is missing and may be created. |
| `upgrade.unchanged` | The file matches its recorded hash, or the legacy manifest already exists |
| `project.template.legacy` |Unknown origin. Existing files remain|
| `project.template.outdated` |The template version is not `0.25`. Nothing is replaced|
| `project.file.modified` |conflict. The file remains|
| `project.file.missing` |conflict. The file is not recreated|
| `project.file.unknown` |An additional file is not touched|
| `project.dependency.path` |Local path, also `D:\…` and `D:/…`|
| `project.dependency.git` |Git dependency|
| `project.dependency.registry` |Registry dependency|
| `project.dependency.unavailable` | No Fexu dependency in `Cargo.toml` |
| `upgrade.conflict` | Abort without replacing files |
| `upgrade.unsafe_path` |The path leaves the project|

## File safety

Paths must be relative and contain only normal components. `..`, absolute paths and drive prefixes produce `upgrade.unsafe_path`. If the target or an ancestor exists, its canonical path is checked against the project root. Junctions and symlinks that escape the project, including those under `.fexu`, are rejected before writing.

The manifest is first written to `.fexu/project.json.partial`, then renamed. An existing partial file is moved to `.fexu/backup/<zeit>/`. If the target appears between planning and writing, the command aborts and preserves it. An interruption before the rename leaves no incomplete `project.json`.

Conflicts also prevent the creation of the manifest. The run ends with exit code 1.

## Exit codes

| Code | Meaning |
|---|---|
| 0 |Plan without file conflict. With `--yes` the manifest was created or there was nothing to do|
| 1 | Invalid project, file conflict, unsafe path or aborted operation |
| 2 |clap reports an invalid command line before Fexu builds the plan|

Pending migrations produce a recommendation to run `fexu migrate up`; they are neither a file conflict nor a step performed by upgrade.

## Limitations

Upgrade does not update templates to version `0.25`. A project using template version `0.21` receives `project.template.outdated` and keeps its files. Upgrade does not convert project layouts or add `src/project/` or `src/apps.rs`. See [manual migration](project-generator.md#migration-from-old-clilayouts). Modified and unrecognized files require manual changes; the command reports their codes and paths without replacing them.
