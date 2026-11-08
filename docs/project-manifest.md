# Projektmanifest

[Zur Übersicht](index.md) · Upgrade: [project-upgrade.md](project-upgrade.md) · Legacy: [legacy-projects.md](legacy-projects.md)

**IST:** `crates/ferrioxid-migrations/src/manifest.rs`. Datei: `.ferrum/project.json`. Die Cargo-Paketversion bleibt `0.1.0`. Die Template-Version ist unabhängig davon und steht im Manifest. Aktuell ist das `0.21`.

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
| `format` | Nur `1` wird gelesen. Ein anderes Format wird nicht umgeschrieben |
| `template_version` | Generatorstand. `legacy`, wenn das Manifest nachträglich für ein Bestandsprojekt entstand |
| `origin` | `generated` oder `legacy` |
| `template` | `api` oder `fullstack`, bei Legacy `null` |
| `files` | Relative Pfade und SHA-256 der Bytes zum Zeitpunkt der Erzeugung |

Das Manifest selbst steht nicht in `files`. Ein SHA-256 gilt nur für den aufgezeichneten Inhalt. Der Dateiname allein ist kein Nachweis, dass eine Datei unverändert ist.

## Erzeugung

`ferrioxid new` schreibt das Manifest für `api` und `fullstack`, nachdem die Projektdateien liegen. Es ist keine Laufzeitabhängigkeit und verlangt kein Node.js. Die Ferrioxid-Abhängigkeit bleibt ein Pfad, wenn `--ferrioxid-path` gesetzt ist.

## Legacy

Fehlt die Datei, ist das Projekt nicht fehlerhaft. `ferrioxid upgrade --yes` legt dann genau dieses Manifest an:

```json
{
  "format": 1,
  "template_version": "legacy",
  "origin": "legacy",
  "template": null,
  "files": []
}
```

Die leere Dateiliste bedeutet: die Herkunft der vorhandenen Dateien ist unbekannt. Sie werden nicht ersetzt. Ein späterer Lauf sieht das Legacy-Manifest und schreibt nichts mehr.

## Kein Freibrief

Ein passender Hash erlaubt keine stillschweigende Ersetzung durch eine neuere Vorlage. v0.21 ersetzt verfolgte Dateien nicht automatisch. Weicht der Inhalt ab, lautet der Code `project.file.modified` und die Datei bleibt. Fehlt eine verfolgte Datei, lautet der Code `project.file.missing`. Sie wird nicht neu erzeugt. Ein ungültiges oder manipuliertes Manifest lautet `upgrade.conflict` und bleibt liegen.
