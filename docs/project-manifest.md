# Projektmanifest

[Zur Übersicht](index.md) · Upgrade: [project-upgrade.md](project-upgrade.md) · Legacy: [legacy-projects.md](legacy-projects.md)

**IST:** `crates/ferrioxid-migrations/src/manifest.rs`. File: `.ferrum/project.json`. The cargo package version remains `0.1.0`. The template version is independent of it and is in the manifesto. Currently the `0.21` is.

## Schema

```json
{
  "format": 1,
  "template_version": "0.21",
  "origin": "generated",
  "template": "fullstack",
  "files": [
    { "path": "src/main.rs", "sha256": "…" }
  ]
}
```

| Feld | Bedeutung |
|---|---|
| `format` |Only `1` is read. Another format is not rewritten|
| `template_version` |Generator level. `legacy` if the manifest was subsequently created for an existing project|
| `origin` |`generated` or `legacy`|
| `template` |`api` or `fullstack`, at Legacy `null`|
| `files` |Relative paths and SHA-256 of the bytes at the time of generation|

The manifesto itself is not in `files`. An SHA-256 applies only to the recorded content. The filename alone is not proof that a file is unchanged.

## Erzeugung

`ferrioxid new` writes the manifest for `api` and `fullstack` after the project files are located. It is not a runtime dependency and does not require Node.js. Ferrioxide dependence remains a path when `--ferrioxid-path` is set.

## Legacy

If the file is missing, the project is not faulty. `ferrioxid upgrade --yes` then creates exactly this manifesto:

```json
{
  "format": 1,
  "template_version": "legacy",
  "origin": "legacy",
  "template": null,
  "files": []
}
```

The empty file list means: the origin of the existing files is unknown. They will not be replaced. A later run sees the Legacy Manifesto and writes nothing more.

## Kein Freibrief

A suitable hash does not allow tacit replacement by a newer template. v0.21 does not automatically replace tracked files. If the content is different, the code is `project.file.modified` and the file remains. If a tracked file is missing, the code is `project.file.missing`. It is not newly produced. An invalid or manipulated manifest is `upgrade.conflict` and remains.
