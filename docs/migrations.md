# Migrations-Engine

[Zur Übersicht](index.md) · **IST:** `crates/ferrum-migrations/`. **Nicht** der Legacy-Befehl `myshop --bootstrap`.

## 1. Architektur

```text
Rust-Modelle
  → FerrumModel / ColumnSpec
  → explizite Registrierung
  → SchemaSnapshot
  → Diff
  → Migrationsdatei
  → Runner
       → SQLite
       → PostgreSQL
```

Die Engine liest keinen Rust-Quelltext. Sie hängt nicht an Axum und nicht am Admin-Frontend. CRUD und Migrationen nutzen dieselbe Typabbildung in `crates/ferrum-db/src/sql.rs`.

| Aufgabe | Datei |
|---|---|
| Snapshot und Validierung | `crates/ferrum-migrations/src/snapshot.rs` |
| Diff und Risiko | `crates/ferrum-migrations/src/diff.rs` |
| Dateiformat und Checksum | `crates/ferrum-migrations/src/store.rs` |
| Historientabelle | `crates/ferrum-migrations/src/history.rs` |
| Ausführung | `crates/ferrum-migrations/src/runner.rs` |
| `ferrum.toml` und Schema-Befehl | `crates/ferrum-migrations/src/project.rs` |
| CLI | `crates/ferrum-cli/src/main.rs` |
| myshop-Registrierung | `examples/myshop/src/main.rs`, `ferrum.toml` |
| DDL | `crates/ferrum-db/src/sql.rs` |

`ColumnSpec` bleibt die Modellmetadaten. Der Snapshot übersetzt sie in logische Typen. SQL entsteht erst für `DatabaseKind`. Unique-Constraints und Indizes sind in den Modellmetadaten nicht vorhanden und fehlen deshalb im Snapshot. Fremdschlüsselbeziehungen sind nicht implementiert.

## 2. Registrierung

`ferrum.toml` im Projekt oder einem übergeordneten Verzeichnis:

```toml
migrations_dir = "examples/myshop/migrations"
schema_command = "cargo run -p myshop --quiet -- --print-schema"
```

`myshop --print-schema` schreibt das JSON der registrierten Modelle nach stdout. Tests und Skripte können stattdessen `--schema-file` übergeben. Ohne diese Integration kann `migrate make` das Modell nicht kennen.

## 3. Snapshot

Formatversion `1`. Tabellen werden nach Namen sortiert. Spalten behalten die Reihenfolge des Modells. Dieselbe Eingabe erzeugt dasselbe kompakte JSON.

```json
{
  "format_version": 1,
  "tables": [
    {
      "name": "products",
      "columns": [
        {"name": "id", "type": "uuid", "primary_key": true, "nullable": false},
        {"name": "name", "type": "string", "primary_key": false, "nullable": false},
        {"name": "active", "type": "bool", "primary_key": false, "nullable": false, "default": true}
      ]
    }
  ]
}
```

Logische Typen sind `uuid`, `string`, `i64` und `bool`. Unbekannte Formatversionen, doppelte Namen und ungültige Bezeichner werden abgelehnt. Ein Snapshot enthält keine Verbindungsdaten und keine Passwörter. Genau ein nicht-nullable Primärschlüssel ist Pflicht. Defaults gibt es nur für `bool`.

Gleiche fachliche Schemas gelten auch dann als unverändert, wenn nur die Spaltenreihenfolge abweicht. Dafür wird keine Migration geschrieben.

## 4. Diff

`plan_change` vergleicht den letzten bestätigten Ziel-Snapshot mit dem aktuellen Modell.

| Änderung | Operation | Ausführung |
|---|---|---|
| Neue Tabelle | `create_table` | sicher |
| Neue nullable Spalte oder `bool` mit Default | `add_column` | sicher |
| Neue `NOT NULL`-Spalte ohne Default | `add_column` | blockiert |
| Neue Primärschlüsselspalte | `add_column` | blockiert |
| Tabelle löschen | `drop_table` | nur mit `--allow-destructive` |
| Spalte löschen, kein Primärschlüssel | `drop_column` | nur mit `--allow-destructive` |
| Primärschlüssel löschen | `drop_column` | blockiert |
| Typ, Nullbarkeit oder Default | `alter_column` | blockiert |
| Spalte weg und andere Spalte neu | Drop plus Add | blockiert, keine Umbenennung |

Eine Umbenennung wird nicht erraten. `migrate make` schreibt in diesem Fall keine Datei und erklärt, dass eine Rename-Operation nicht implementiert ist.

## 5. Dateiformat

```text
examples/myshop/migrations/0001_initial.json
```

Jede Datei enthält Formatversion, ID `0001_name`, Namen, Ausgangssnapshot, Zielsnapshot, Operationen, Risiken und eine SHA-256-Checksum. Die Checksum deckt den kanonischen JSON-Körper ohne das Checksum-Feld ab. Sie erkennt inhaltliche Änderungen. Sie beweist nicht, dass die Datenbank noch genau diesem Schema entspricht.

Die erste Migration muss von einem leeren Schema ausgehen. Jede folgende Migration muss exakt auf dem Zielsnapshot der vorherigen aufsetzen. IDs sind vierstellig nummeriert, höchstens `9999`.

## 6. Historie

Tabelle `_ferrum_migrations`:

| Spalte | Bedeutung |
|---|---|
| `id` | Primärschlüssel, Migrations-ID |
| `name` | Kurzname |
| `checksum` | Checksum zum Anwendungszeitpunkt |
| `applied_at_unix` | Unix-Sekunden |

Die Zeile wird in derselben Transaktion geschrieben wie die Schemaänderung. Schlägt die Migration fehl, gibt es weder die Schemaänderung noch den Historieneintrag. Es gibt keinen gespeicherten Fehlerstatus. Eine angewendete ID wird nicht erneut ausgeführt. Eine fehlende Datei oder eine abweichende Checksum stoppt `migrate up`.

Ein Historieneintrag garantiert nicht, dass niemand das Schema danach von außen geändert hat. Ferrum vergleicht die Historie mit den Dateien, liest aber nicht das komplette Live-Schema zurück.

## 7. CLI

Im Workspace-Root:

```powershell
cargo run -p ferrum-cli -- migrate make add_note
cargo run -p ferrum-cli -- migrate up
cargo run -p ferrum-cli -- migrate status
```

Die installierte Binary heißt `ferrum`. `make` legt ohne Schemaänderung keine Datei an. Riskante Löschungen stehen in der Datei und laufen erst mit:

```powershell
cargo run -p ferrum-cli -- migrate up --allow-destructive
```

`status` zeigt Backend, Dateien, angewendet oder ausstehend, Checksum-Konflikte und, wenn das Modellschema geladen werden konnte, ob es zum letzten Migrationsziel passt. Die Datenbank-URL wird nicht ausgegeben. `--database-url` und `--migrations-dir` überschreiben Projekt und Umgebung. Ohne URL gilt `DATABASE_URL`, sonst `sqlite://db.sqlite3?mode=rwc` im Arbeitsverzeichnis.

## 8. SQLite

SQLite ist das getestete Backend. `connect` hält eine Verbindung und setzt `PRAGMA foreign_keys=ON`. Tests nutzen temporäre Dateien.

Ausgeführt werden `CREATE TABLE`, sicheres `ADD COLUMN`, sowie `DROP TABLE` und `DROP COLUMN` nur nach ausdrücklicher Freigabe. Jede Migration läuft in einer Transaktion. SQLite kann viele `ALTER`-Formen nicht. Ein Tabellen-Neubau mit Kopieren der Zeilen ist nicht implementiert und wird nicht angefangen.

## 9. PostgreSQL

Dieselbe Operationsliste wird mit dem PostgreSQL-Dialekt von SeaQuery gerendert. Historie und Checksums sind dieselben. `CREATE TABLE`, sicheres `ADD COLUMN` und freigegebene Löschungen sind damit vorgesehen. `ALTER COLUMN` bleibt auch hier blockiert, damit die beiden Backends nicht auseinanderlaufen.

Der Test `postgres_migration_roundtrip` ist `#[ignore]` und verlangt `DATABASE_URL`. Er wurde hier nicht gegen einen Server ausgeführt. Ein ignorierter Test ist kein Erfolg.

Nicht implementiert: Advisory-Locks gegen parallele Migratoren, `down`, Baseline einer schon gefüllten Datenbank.

## 10. Fehler und Transaktionen

Vor dem ersten SQL prüft der Runner Checksums, Reihenfolge und die Zulässigkeit jeder Operation. Eine nicht unterstützte oder nicht freigegebene Operation bricht ab. Bereits festgeschriebene frühere Migrationen bleiben bestehen. Die fehlgeschlagene Migration und alle späteren werden nicht angewendet.

`CREATE TABLE` ohne `IF NOT EXISTS` trifft eine vorhandene Tabelle, rollt zurück und markiert sie nicht als migriert. Die Meldung verweist auf eine neue Datenbank. Vorhandene Dateien werden nicht gelöscht.

## 11. Bootstrap

`cargo run -p myshop -- --bootstrap` ist Legacy. Der Befehl schreibt kein Schema und keine Historie.

- Historie vorhanden: Abbruch, Hinweis auf `migrate status`.
- Tabelle `products` ohne Historie: Abbruch, keine Löschung, keine Markierung als migriert.
- Leere Datenbank: Abbruch, Hinweis auf `migrate make` und `migrate up`.

`App::bootstrap_schema` bleibt ein Testhelfer für leere Datenbanken ohne Historie. Existiert `_ferrum_migrations`, legt auch diese Funktion nichts an. Sie ist keine Migration und erkennt keinen Schema-Drift.

Der HTTP-Server migriert nicht beim Start.

### Übergang einer alten SQLite-Datei

Eine mit dem früheren Bootstrap erzeugte `db.sqlite3` hat Tabellen und keine Historie. Sie wird nicht automatisch übernommen und nicht gelöscht. Für neue Migrationen eine andere Datei verwenden, zum Beispiel über `DATABASE_URL` oder ein neues Arbeitsverzeichnis. Eine Baseline-Funktion, die ein bestehendes Schema nur registriert, gibt es nicht.

## 12. Tests

`cargo test --workspace` führt die Snapshot-, Diff- und SQLite-Tests aus, darunter den CLI-Lauf gegen eine temporäre Datei. Die Entwicklungsdatenbank `db.sqlite3` wird dabei nicht verwendet. PostgreSQL bleibt ignoriert.

## 13. Nicht implementiert

Rename-Ausführung, Tabellen-Neubau, `migrate down`, Unique- und Index-Metadaten, Fremdschlüssel, Baseline, Advisory-Lock, automatische Migration beim Serverstart, MySQL.
