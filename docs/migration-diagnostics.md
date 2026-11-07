# Migrationsdiagnose

[Zur Übersicht](index.md) · Migrationen: [migrations.md](migrations.md) · CLI: [cli.md](cli.md)

**IST:** `crates/ferrum-migrations/src/inspect.rs`. Die Cargo-Version bleibt `0.1.0`.

## Aufgabe

Dieselbe lesende Prüfung gilt für `ferrum migrate status`, `ferrum check`, `ferrum doctor`, `ferrum dev` und `ferrum upgrade --check` beziehungsweise `--dry-run`. Sie wendet keine Migration an, legt keine Historientabelle an und erzeugt keine SQLite-Datei.

Auth-Dateien liegen in `crates/ferrum-auth/migrations`. Die Historie heißt `_ferrum_auth_migrations`. App-Dateien liegen im `migrations_dir` des Projekts. Die Historie heißt `_ferrum_migrations`. Deklarierte App-Pakete aus `[app.<name>]` stehen in derselben Historie unter `shop:0001_initial`. Die CLI liest diese Deklaration und nicht `registry.migrations`. Die Zeile `app.migration.runtime` sagt das. Der Prozessstart vergleicht beide Seiten.

## Bericht

Jede Abweichung ist ein `Diagnostic` mit Bereich `Auth` oder `App`, Migrations-ID, Stufe `Ok`, `Warn` oder `Error`, stabilem Code und Text. Angewendete, ausstehende und konfliktbehaftete Dateien bleiben getrennt.

Stabile Codes:

| Code | Stufe | Bedeutung |
|---|---|---|
| `migration.auth.pending` | Warnung | Auth-Datei ist nicht in `_ferrum_auth_migrations` |
| `migration.app.pending` | Warnung | App-Datei ist nicht in `_ferrum_migrations` |
| `migration.history.missing` | Warnung | Die Historientabelle fehlt. Sie wird nicht angelegt |
| `migration.checksum_mismatch` | Fehler | Gespeicherte Checksum und Datei stimmen nicht überein |
| `migration.unknown` | Fehler | Eine angewendete ID hat keine Datei mehr |
| `migration.order` | Fehler | Die Reihenfolge von Dateien und Historie stimmt nicht |
| `migration.schema_conflict` | Warnung | Das geladene Modell entspricht dem Migrationsziel nicht |
| `migration.auth.table_missing` | Fehler | `0002_admin_journal` oder `0003_audit_operation` ist angewendet, die Tabelle oder `operation_id` fehlt |
| `migration.database.missing` | Warnung in check, doctor, dev und upgrade. Fehler in `migrate status` | Die SQLite-Datei existiert nicht |
| `migration.database.unavailable` | Fehler | Die vorhandene Datei ist keine lesbare Datenbank. Das ist kein Erfolg |

`0002_admin_journal` betrifft `_ferrum_admin_audit` und `_ferrum_admin_drafts`. `0003_audit_operation` betrifft die Spalte `operation_id`. Solange diese Dateien nur ausstehen, ist das eine Warnung und kein stilles Anlegen.

## Lesen ohne Schreiben

SQLite-Diagnosen öffnen eine vorhandene Datei mit `mode=ro`. Eine fehlende Datei führt zu keinem `connect`. PostgreSQL wird mit derselben URL verbunden, die Abfragen sind `SELECT`. `ensure_history` bleibt den schreibenden Befehlen vorbehalten.

Die URL ist `DATABASE_URL`, sonst `database.url` aus `ferrum.toml`. Dieselbe Auflösung nutzt das generierte Programm. Eine URL, die nur im Anwendungscode berechnet wird, sieht die CLI nicht.

## Schema

`ferrum migrate status` lädt das Modell über `--schema-file` oder `schema_command`. `ferrum check` tut dasselbe, nachdem die Datenbankdatei vorhanden ist. `FERRUM_SCHEMA_FILE` ersetzt den Befehl. `FERRUM_CHECK_SKIP_SCHEMA=1` überspringt ihn in `check`. `doctor`, `dev` und `upgrade` starten `schema_command` nicht. Ohne geladenes Schema meldet die Diagnose das ausdrücklich und behauptet keine Übereinstimmung.

## Befehle

`migrate status` bleibt bei ausstehenden Migrationen erfolgreich und nennt `ferrum migrate up`. Konflikte, eine fehlende Datei und eine nicht lesbare Datenbank beenden ihn mit Exitcode 1.

`check` beendet Warnungen nur mit `--strict` und Exitcode 2. Ohne `--strict` bleibt der Exitcode 0, solange kein Fehler vorliegt. `cargo check` läuft danach. `FERRUM_CHECK_SKIP_CARGO=1` lässt diesen Schritt aus und ist für Tests gedacht.

`doctor` erklärt dieselbe Lage und schlägt `ferrum migrate up` vor. Warnungen ändern seinen Exitcode nicht. Eine nicht lesbare vorhandene Datenbank ist ein Fehler. Eine noch nicht angelegte SQLite-Datei ist eine Warnung. Storage-Routen werden nicht aus dem Quelltext erraten. Ein fehlendes Storage-Verzeichnis ist nur dann eine Warnung, wenn das Verzeichnis selbst fehlt.

`dev` druckt die Warnung vor dem Build. Ausstehende Migrationen brechen den Start nicht ab. Konnte der Status nicht gelesen werden, steht das so da. Es gibt keine Erfolgsmeldung über eine ungeprüfte Datenbank.

## Beispiel

```text
Backend: sqlite

Auth-Migrationen:
  0001_auth_schema  angewendet  checksum …
  0002_admin_journal  ausstehend  checksum …
  0003_audit_operation  ausstehend  checksum …

App-Migrationen:
  0001_initial  angewendet  checksum …

Konflikte: keine
Ausstehende Migrationen: 2
  migration.auth.pending 0002_admin_journal ist ausstehend.
  migration.auth.pending 0003_audit_operation ist ausstehend.

Nächster Schritt:
  ferrum migrate up
```

`FERRUM_LOCALE=en` schaltet die Überschriften auf Englisch. Die Codes bleiben Englisch.

## PowerShell

```powershell
$env:FERRUM_LOCALE = "de"
ferrum migrate status
ferrum check
ferrum check --strict
ferrum doctor
```
