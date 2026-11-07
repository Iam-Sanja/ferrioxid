# Bestandsprojekte

[Zur Übersicht](index.md) · Upgrade: [project-upgrade.md](project-upgrade.md) · Diagnose: [migration-diagnostics.md](migration-diagnostics.md)

Ein Projekt ohne `.ferrum/project.json` ist ein Legacy-Projekt. Das ist kein Fehler. `ferrum check` meldet `project.template.legacy` als Warnung. Ohne `--strict` bleibt der Exitcode 0.

## Erkennen

Ferrum liest `Cargo.toml` als Text.

- `path = "…"` ist `project.dependency.path`. Relative Pfade, `D:/…` und `D:\…` bleiben stehen.
- `git = "…"` ist `project.dependency.git`.
- Eine Versionsangabe ohne Pfad ist `project.dependency.registry`.
- `0.1.0` ist die Cargo-Version aller bisherigen Funktionsstände und sagt nicht, ob Auth-Migrationen `0002_admin_journal` und `0003_audit_operation` angewendet sind.

`admin-ui` darf bleiben.

## Datenbank

Die Diagnose verwendet `DATABASE_URL` oder `database.url`. Sie öffnet SQLite nur lesend und nur, wenn die Datei schon existiert. Fehlende Auth-Migrationen erscheinen als `migration.auth.pending`. Die Reparatur bleibt:

```powershell
ferrum migrate up
```

Dieser Schritt ist bewusst nicht Teil von `ferrum upgrade`.

## Dateien

Solange kein Manifest mit Hashes existiert, stuft Ferrum keine Datei als unverändert ein. `ferrum upgrade --yes` schreibt nur `.ferrum/project.json` mit `origin: legacy` und einer leeren Dateiliste. Modelle, Routen, Vorlagen, Staticfiles, Migrationen und eigene Dateien bleiben bytegleich.

Ein zweiter `ferrum upgrade --yes` findet das Manifest und ändert es nicht.

## Nicht automatisch

Diese Punkte bleiben manuell, auch nach dem Legacy-Manifest:

- fehlende oder abweichende Projektdateien an eine neuere Vorlage angleichen
- `schema_command`, Storage-Routen oder Admin-Registrierung im eigenen `main.rs` ändern
- Benutzer, Rollen und Passwörter
- bereits angewendete Migrationsdateien

`ferrum doctor` behauptet nicht, dass Storage-Routen fehlen, nur weil `[storage]` in `ferrum.toml` steht. Es meldet ein fehlendes Verzeichnis als `project.file.missing` und sagt, dass die Routen nicht aus dem Quelltext geschlossen werden.
