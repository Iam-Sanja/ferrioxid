# Project manifest

[Overview](index.md) · Upgrade: [project-upgrade.md](project-upgrade.md) · Legacy: [legacy-projects.md](legacy-projects.md)

**Implemented:** `crates/ferrioxid-migrations/src/manifest.rs`, stored in `.ferrum/project.json`. The Cargo version remains `0.1.0`; the independent current template version is `0.24`. The preserved JSON example below shows the older `0.21` manifest format, which remains readable.

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

| Field | Meaning |
|---|---|
| `format` |Only `1` is read. Another format is not rewritten|
| `template_version` |Generator level. `legacy` if the manifest was subsequently created for an existing project|
| `origin` |`generated` or `legacy`|
| `template` |`api` or `fullstack`, at Legacy `null`|
| `files` |Relative paths and SHA-256 of the bytes at the time of generation|

The manifest itself is not in `files`. An SHA-256 applies only to the recorded content. The filename alone is not proof that a file is unchanged.

## Generation

`ferrioxid new` writes a manifest after generating all API or fullstack project files. The manifest is not required at runtime. Generated projects currently use a local Ferrioxid path dependency.

## Legacy

If the file is missing, the project is not faulty. `ferrioxid upgrade --yes` then creates exactly this manifest:

```json
{
  "format": 1,
  "template_version": "legacy",
  "origin": "legacy",
  "template": null,
  "files": []
}
```

The empty file list means: the origin of the existing files is unknown. They will not be replaced. A later run sees the Legacy Manifest and writes nothing more.

## Explicit limits

A matching hash does not authorize automatic replacement by a newer template. Upgrade leaves modified files in place and reports `project.file.modified`. Missing tracked files produce `project.file.missing` without being recreated. Invalid or manipulated manifests produce `upgrade.conflict` and remain untouched.
