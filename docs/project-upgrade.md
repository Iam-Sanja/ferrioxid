# Projekt-Upgrade

[Zur Übersicht](index.md) · Manifest: [project-manifest.md](project-manifest.md) · Diagnose: [migration-diagnostics.md](migration-diagnostics.md)

**IST:** `crates/ferrum-migrations/src/upgrade.rs` und `ferrum upgrade`. Die Cargo-Version bleibt `0.1.0`. v0.21.1 ändert die Schreibregeln nicht: `--yes` legt weiterhin nur ein fehlendes Legacy-Manifest an.

## Was der Befehl tut

Er prüft ein vorhandenes Projekt und kann eine einzige sichere Datei anlegen: das fehlende Legacy-Manifest `.ferrum/project.json`. Er erzeugt das Projekt nicht neu.

Er tut ohne ausdrückliche spätere Erweiterung nicht:

- Migrationen anwenden oder die Datenbank zurücksetzen
- Passwörter, Rollen oder Daten ändern
- Modelle, Routen, Vorlagen, Styles oder Migrationen überschreiben
- unbekannte Dateien löschen
- Git-Befehle ausführen
- einen vorhandenen Dependency-Pfad ändern

`admin-ui` bleibt ein gültiges Kompatibilitäts-Feature. Der Code `project.dependency.admin_ui` ist eine Information, keine Aufforderung zum Entfernen. Die interne Template-Version wird nicht aus der Cargo-Version `0.1.0` abgeleitet.

## Aufruf

```powershell
ferrum upgrade --check
ferrum upgrade --dry-run
ferrum upgrade --yes
```

`--check` analysiert Abhängigkeit, Manifest und Migrationslage. `--dry-run` zeigt dieselben Codes und die geplanten Dateiänderungen unter der Überschrift `Geplante Dateiänderungen`. Beide schreiben nicht. `--yes` zusammen mit `--check` oder `--dry-run` schreibt ebenfalls nicht.

Ohne `--yes` wird nichts geschrieben. Die Zustimmung ist dieses Flag, keine Rückfrage.

## Plan

| Aktion | Bedeutung |
|---|---|
| `upgrade.plan.add .ferrum/project.json` | Das Legacy-Manifest fehlt und darf angelegt werden |
| `upgrade.unchanged` | Die Datei entspricht dem aufgezeichneten Hash oder das Legacy-Manifest liegt schon |
| `project.template.legacy` | Herkunft unbekannt. Vorhandene Dateien bleiben |
| `project.template.outdated` | Die Template-Version ist nicht `0.24`. Es wird nichts ersetzt |
| `project.file.modified` | Konflikt. Die Datei bleibt |
| `project.file.missing` | Konflikt. Die Datei wird nicht neu angelegt |
| `project.file.unknown` | Eine zusätzliche Datei wird nicht angefasst |
| `project.dependency.path` | Lokaler Pfad, auch `D:\…` und `D:/…` |
| `project.dependency.git` | Git-Abhängigkeit |
| `project.dependency.registry` | Registry-Abhängigkeit |
| `project.dependency.unavailable` | Keine Ferrum-Abhängigkeit in `Cargo.toml` |
| `upgrade.conflict` | Abbruch, keine Ersetzung |
| `upgrade.unsafe_path` | Der Pfad verlässt das Projekt |

## Dateisicherheit

Pfade müssen relativ sein und dürfen nur normale Komponenten enthalten. `..`, absolute Pfade und Laufwerkspräfixe werden `upgrade.unsafe_path`. Existiert das Ziel oder ein Vorfahre, wird der kanonische Pfad mit der kanonischen Projektwurzel verglichen. Eine Junction oder ein Symlink nach außen, auch für `.ferrum`, wird abgelehnt, bevor etwas geschrieben wird.

Die neue Datei entsteht als `.ferrum/project.json.partial` und wird erst danach umbenannt. Liegt schon eine Partial-Datei, wandert sie nach `.ferrum/backup/<zeit>/`. Entsteht die Zieldatei zwischen Plan und Schreiben, bricht der Lauf ab und lässt die vorhandene Datei stehen. Ein Abbruch vor dem Umbenennen hinterlässt keine halbe `project.json`.

Konflikte verhindern auch das Anlegen des Manifests. Der Lauf endet dann mit Exitcode 1.

## Exitcodes

| Code | Bedeutung |
|---|---|
| 0 | Plan ohne Dateikonflikt. Mit `--yes` wurde das Manifest angelegt oder es gab nichts zu tun |
| 1 | Ungültiges Projekt, Konflikt, unsicherer Pfad oder Abbruch |
| 2 | clap meldet eine ungültige Befehlszeile, bevor Ferrum den Plan baut |

Ausstehende Migrationen sind im Upgrade eine Empfehlung `ferrum migrate up`, kein Schreibfehler und kein angewendeter Schritt.

## Grenze

v0.24 migriert Vorlagen nicht automatisch auf den Generatorstand. Ein Projekt mit Template-Version `0.21` erhält `project.template.outdated` und behält seine Dateien. `src/settings.rs` und `src/urls.rs` legt das Upgrade nicht an. Eine geänderte oder fremde Datei bleibt eine manuelle Aufgabe. Der Befehl zeigt den Code und den Pfad. Er schreibt daneben keine zweite Fassung und ersetzt die Datei nicht.
