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

Auth-Dateien liegen in `crates/ferrum-auth/migrations/` und schreiben die Historie `_ferrum_auth_migrations`. `ferrum migrate up` wendet dieses Verzeichnis vor den Anwendungsdateien an. Die Anwendungshistorie bleibt `_ferrum_migrations`. Dadurch kollidiert `0001_auth_schema` nicht mit `0001_initial`. Der Serverstart migriert nicht. `0001_initial.json` bleibt unverändert.

| Aufgabe | Datei |
|---|---|
| Snapshot und Validierung | `crates/ferrum-migrations/src/snapshot.rs` |
| Diff und Risiko | `crates/ferrum-migrations/src/diff.rs` |
| Dateiformat und Checksum | `crates/ferrum-migrations/src/store.rs` |
| Historientabelle | `crates/ferrum-migrations/src/history.rs` |
| Ausführung | `crates/ferrum-migrations/src/runner.rs` |
| Snapshot-Simulation | `crates/ferrum-migrations/src/simulate.rs` |
| SQLite-Umbau und PostgreSQL-ALTER | `crates/ferrum-migrations/src/rebuild.rs` |
| SQLite-Introspektion | `crates/ferrum-migrations/src/introspection.rs` |
| Baseline | `crates/ferrum-migrations/src/baseline.rs` |
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

Zusätzlich kann eine Tabelle `unique_constraints`, `indexes` und `foreign_keys` enthalten. Leere Listen werden nicht serialisiert. Dateien der Formatversion 1 ohne diese Schlüssel bleiben gültig und behalten ihre Checksum. Die Bedeutung bereits angewendeter Migrationen ändert sich dadurch nicht. Aufbau, NULL-Verhalten und die blockierten Foreign-Key-Änderungen stehen in [constraints.md](constraints.md).

Gleiche fachliche Schemas gelten auch dann als unverändert, wenn nur die Spaltenreihenfolge abweicht. Dafür wird keine Migration geschrieben.

## 4. Diff

`plan_change` vergleicht den letzten bestätigten Ziel-Snapshot mit dem aktuellen Modell. Er erzeugt keine Umbenennung.

| Änderung | Operation | Ausführung |
|---|---|---|
| Neue Tabelle | `create_table` | sicher |
| Neue nullable Spalte oder `bool` mit Default | `add_column` | sicher |
| Neue `NOT NULL`-Spalte ohne Default | `add_column` | blockiert |
| Neue Primärschlüsselspalte | `add_column` | blockiert |
| Tabelle löschen | `drop_table` | nur mit `--allow-destructive` |
| Spalte löschen, kein Primärschlüssel | `drop_column` | nur mit `--allow-destructive` |
| Primärschlüssel löschen | `drop_column` | blockiert |
| `string` ↔ `i64` | `alter_column` | nur nach Wertprüfung |
| Dieselbe logische Art, andere Nullbarkeit oder anderer Bool-Default | `alter_column` | nur nach Wertprüfung |
| Andere Typpaare, Primärschlüssel-Flag | `alter_column` | blockiert |
| Spalte weg und andere Spalte neu | Drop plus Add | blockiert, keine Umbenennung |
| Tabelle weg und andere Tabelle neu | Drop plus Create | blockiert, keine Umbenennung |
| Explizite Umbenennung | `rename_column`, `rename_table` | nur über den Rename-Befehl; betroffene Indizes werden neu angelegt |
| Neuer Unique Index | `add_unique_constraint` | sicher; vorhandene Duplikate lassen die Migration fehlschlagen |
| Unique Index entfernen | `drop_unique_constraint` | sicher für die Zeilen; danach sind Duplikate erlaubt |
| Neuer Index | `add_index` | sicher |
| Index entfernen | `drop_index` | sicher |
| Foreign Key auf einer neuen Spalte | `add_column` mit `foreign_key` | sicher, sofern die Spalte nullable ist oder einen Bool-Default hat |
| Foreign Key auf einer bestehenden Spalte, Entfernen oder neue Löschregel | `add_foreign_key`, `drop_foreign_key` | blockiert, keine Datei |

`migrate make` schreibt bei einer vermuteten Umbenennung keine Datei. Die Meldung enthält „nicht als Umbenennung“ und verweist auf `rename-column` oder `rename-table`.

## 4.1 Explizite Umbenennung

Zuerst das Modell auf den neuen Namen ändern. Danach, im Workspace-Root:

```powershell
cargo run -p ferrum-cli -- migrate rename-column --table products --from name --to title
cargo run -p ferrum-cli -- migrate rename-table --from products --to items
```

Die CLI lädt das registrierte Schema. Die eine Operation muss dieses Schema aus dem letzten Migrationsziel vollständig erklären. Weitere Abweichungen führen dazu, dass keine Datei geschrieben wird. Der folgende `migrate make` sieht dann keine Änderung mehr.

Bezeichner müssen dem Snapshot-Format entsprechen: Buchstabe oder Unterstrich, danach Buchstaben, Ziffern oder Unterstriche. Sie werden in SQL gequotet. HTTP-Eingaben sind keine Quelle für diese Namen.

Beide Dialekte erhalten dasselbe SQL:

```sql
ALTER TABLE "products" RENAME COLUMN "name" TO "title"
ALTER TABLE "products" RENAME TO "items"
```

Eine Umbenennung kopiert die Tabelle nicht. Hängt ein Unique Index oder ein sekundärer Index an der umbenannten Spalte oder Tabelle, folgen `DROP INDEX` und `CREATE INDEX` mit dem neuen deterministischen Namen. Foreign-Key-Definitionen folgen der Spaltenumbenennung des Dialekts. Ihre Löschregel wird dabei nicht geändert. Ein vorhandener Index, Trigger, View oder Foreign Key blockiert einen späteren Tabellenumbau.

Eine noch nicht angewendete Datei darf bearbeitet werden. Danach:

```powershell
cargo run -p ferrum-cli -- migrate reseal migrations\0002_rename_title.json
```

`reseal` prüft, dass die Operationen den Zielsnapshot erzeugen, schreibt die Checksum neu und verweigert die Datei, wenn ihre ID schon in `_ferrum_migrations` steht. Eine angewendete Datei mit geänderter Checksum stoppt `migrate up`.

## 5. Dateiformat

```text
examples/myshop/migrations/0001_initial.json
```

Jede Datei enthält Formatversion, ID `0001_name`, Namen, Ausgangssnapshot, Zielsnapshot, Operationen, Risiken und eine SHA-256-Checksum. Die Checksum deckt den kanonischen JSON-Körper ohne das Checksum-Feld ab. Sie erkennt inhaltliche Änderungen. Sie beweist nicht, dass die Datenbank noch genau diesem Schema entspricht.

Die Operationsvarianten sind extern getaggt mit `type` in `snake_case`: `create_table`, `drop_table`, `add_column`, `drop_column`, `alter_column`, `rename_column`, `rename_table`, `add_unique_constraint`, `drop_unique_constraint`, `add_index`, `drop_index`, `add_foreign_key`, `drop_foreign_key`. Die beiden Foreign-Key-Varianten werden erkannt, aber nicht geschrieben und nicht ausgeführt. `add_column` darf ein optionales `foreign_key` tragen. Fehlt es, bleibt das JSON der älteren Dateien gültig. Beim Laden muss die Simulation der Operationen auf dem Ausgangssnapshot den Zielsnapshot ergeben. Die automatische Diff-Liste muss nicht identisch sein. Deshalb kann eine Datei eine explizite Umbenennung enthalten, obwohl `plan_change` dieselbe Änderung ablehnen würde.

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
cargo run -p ferrum-cli -- migrate rename-column --table products --from name --to title
cargo run -p ferrum-cli -- migrate rename-table --from products --to items
cargo run -p ferrum-cli -- migrate reseal pfad\zur\datei.json
cargo run -p ferrum-cli -- migrate baseline
cargo run -p ferrum-cli -- migrate baseline --confirm
```

Die installierte Binary heißt `ferrum`. `make` legt ohne Schemaänderung keine Datei an. Riskante Löschungen stehen in der Datei und laufen erst mit:

```powershell
cargo run -p ferrum-cli -- migrate up --allow-destructive
```

`status` zeigt Backend, Dateien, angewendet oder ausstehend, Checksum-Konflikte und, wenn das Modellschema geladen werden konnte, ob es zum letzten Migrationsziel passt. Die Datenbank-URL wird nicht ausgegeben. `--database-url` und `--migrations-dir` überschreiben Projekt und Umgebung. Ohne URL gilt `DATABASE_URL`, sonst `sqlite://db.sqlite3?mode=rwc` im Arbeitsverzeichnis.

Löschrisiken (`drop_table`, `drop_column`) weist die CLI vor `up` auf `--allow-destructive` hin. Ein `rebuild_column`-Risiko ist keine Löschfreigabe. Eine fehlgeschlagene Prüfung endet mit Exit-Code 1 und wird nicht als Erfolg ausgegeben. `baseline` ohne `--confirm` ist so ein Fall, auch wenn das Schema passt.

## 8. SQLite

SQLite ist das getestete Backend. `connect` hält eine Verbindung und setzt `PRAGMA foreign_keys=ON`. Tests nutzen temporäre Dateien. `PRAGMA foreign_keys` wird in einer Transaktion nicht umgeschaltet. SQLite würde diese Änderung dort ignorieren.

Ausgeführt werden `CREATE TABLE` inklusive Foreign Keys, `CREATE UNIQUE INDEX`, `CREATE INDEX`, sicheres `ADD COLUMN` optional mit `REFERENCES`, `DROP INDEX`, `RENAME TABLE`, `RENAME COLUMN`, sowie `DROP TABLE` und `DROP COLUMN` nur nach ausdrücklicher Freigabe. Jede Migration läuft in einer Transaktion zusammen mit dem Historieneintrag. Ein Unique Index, der vorhandene Zeilen verletzt, rollt diese Transaktion zurück.

### Tabellenumbau

`string` ↔ `i64` sowie Änderungen von Nullbarkeit und Bool-Default laufen auf SQLite als Kopie der Tabelle, weil SQLite diese Änderungen nicht allgemein per `ALTER COLUMN` kann. Der Ablauf in `rebuild.rs`:

1. Ausgangs- und Zielspalte prüfen. Nicht unterstützte Paare, Primärschlüsseländerungen und Typänderungen ohne geprüfte Konvertierung werden abgelehnt.
2. Fremdschlüssel, explizite Indizes, Trigger und Views der Tabelle prüfen. Ist eines davon vorhanden, wird vor jedem DDL abgebrochen.
3. Bei Verschärfung auf `NOT NULL` die vorhandenen `NULL`-Werte zählen. Ist die Zahl größer als 0, wird abgebrochen. Es wird kein Ersatzwert eingesetzt.
4. Nur die betroffene Spalte lesen und in Rust prüfen. `NULL` bleibt `NULL`. Gleiche Typen müssen zum Speicherwert passen. Bool muss `0` oder `1` sein. `i64` nach `string` ist die dezimale Darstellung. `string` nach `i64` muss exakt `i64::to_string` entsprechen. `01`, `+1`, `1.0`, `-0` und leerer Text werden abgelehnt. Ein erfolgreiches SQL-`CAST` allein gilt nicht.
5. `ferrum_rebuild_<tabelle>` mit dem Zielschema anlegen.
6. `INSERT INTO ... SELECT` kopiert die übrigen Spalten. Nur die geprüfte Typänderung wird als `CAST` eingesetzt. `name` wird nicht als `title` erraten.
7. `COUNT(*)` von Quelle und Kopie vergleichen.
8. Die alte Tabelle löschen und die Kopie auf den alten Namen umbenennen.
9. Die Zeilenzahl erneut vergleichen.
10. `PRAGMA integrity_check` muss `ok` liefern. `PRAGMA foreign_key_check` muss leer sein.
11. Den Historieneintrag schreiben und die Transaktion festschreiben.

Die übrigen Spalten werden nicht in den Prozessspeicher geladen. Die geprüfte Spalte wird weiterhin vollständig gelesen, damit ungültige Werte vor dem `CAST` auffallen.

### Rollback

In dieser Transaktion sind für SQLite vollständig zurückrollbar: `CREATE TABLE`, `INSERT`, `DROP TABLE`, `ALTER TABLE ... RENAME` und der Historieneintrag. Schlägt ein Schritt fehl, wird die Transaktion verworfen. Die fehlgeschlagene Migration gilt nicht als angewendet. `PRAGMA foreign_keys` wird dabei nicht verändert. Änderungen außerhalb dieser Transaktion, insbesondere ein schon festgeschriebenes `ensure_history` beim normalen `migrate up`, bleiben bestehen. Die Baseline legt die Historientabelle erst in der Bestätigungstransaktion an.

## 9. PostgreSQL

Dieselbe Operationsliste wird ausgeführt, aber nicht über den SQLite-Umbau. `CREATE TABLE`, `CREATE INDEX`, sicheres `ADD COLUMN`, `RENAME` und freigegebene Löschungen nutzen den PostgreSQL-Dialekt. Ein verletzender Unique Index oder Foreign Key lässt die Transaktion fehlschlagen. `add_foreign_key` und `drop_foreign_key` bleiben auch hier blockiert, weil dieselbe Migrationsdatei auf SQLite laufen können muss. Eine unterstützte Spaltenänderung prüft die Werte ebenfalls in Rust und führt danach `ALTER COLUMN` aus: Typ über `TYPE ... USING`, Default über `SET DEFAULT` oder `DROP DEFAULT`, Nullbarkeit über `SET NOT NULL` oder `DROP NOT NULL`. Nicht unterstützte Änderungen werden auch hier abgelehnt.

Baseline auf PostgreSQL bricht sofort ab und schreibt keine Historie. Es gibt keine stille Teilübernahme.

Der Test `postgres_migration_roundtrip` ist `#[ignore]` und verlangt `DATABASE_URL`. Er wurde hier nicht gegen einen Server ausgeführt. Ein ignorierter Test ist kein Erfolg.

Nicht implementiert: Advisory-Locks gegen parallele Migratoren und `down`.

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

`ferrum migrate baseline` übernimmt eine SQLite-Datei, deren Schema bereits dem letzten Migrationsziel entspricht. Der Befehl ändert keine Tabelle und keine Zeile. Ohne `--confirm` wird auch keine Historie geschrieben, und der Prozess endet mit einem Fehler, der die erfolgreiche Prüfung und die fehlende Bestätigung nennt.

Ablauf:

1. Verbindung öffnen. PostgreSQL wird abgelehnt.
2. Schema über `sqlite_master`, `PRAGMA table_info`, `PRAGMA index_list`, `PRAGMA index_info` und `PRAGMA foreign_key_list` lesen. `_ferrum_migrations`, `sqlite_%` und der Primärschlüssel-Autoindex werden ignoriert.
3. Vorhandene Historieneinträge lehnen die Baseline ab.
4. Die Migrationskette muss vom leeren Schema aus konsistent sein.
5. Enthält eine Datei `drop_table`, `drop_column`, `drop_index`, `drop_unique_constraint`, `drop_foreign_key`, `add_foreign_key` oder eine `alter_column`-Operation mit geändertem logischen Typ, wird die ganze Kette abgelehnt. Das Endschema belegt diese Schritte nicht. Die Meldung verlangt eine manuelle Übernahme.
6. Erlaubte Kettenglieder sind `create_table`, `add_column`, `add_unique_constraint`, `add_index`, `rename_column`, `rename_table` und `alter_column` mit derselben logischen Art.
7. Verglichen werden Tabellen, Spalten, SQL-Typen, Nullbarkeit, Primärschlüssel, Defaults, Unique-Index-Namen, sekundäre Index-Namen und Foreign Keys nach Spalten, Zieltabelle, Zielspalte, `ON DELETE` und `ON UPDATE NO ACTION`. `text` entspricht `uuid` oder `string`. `integer`, `int` und `bigint` entsprechen `i64` oder, beim erwarteten Bool, `bool`. Bool-Defaults `1`/`true` und `0`/`false` gelten als gleich. Klammern und Anführungszeichen am Default werden entfernt. Ein zusätzlicher Nutzer-Index oder Foreign Key blockiert.
8. Eine fehlende oder zusätzliche Spalte oder Tabelle blockiert. Ein passender Tabellenname reicht nicht.
9. Nur `--confirm` schreibt die Historieneinträge in einer Transaktion.

Danach führt `migrate up` die schon markierten Dateien nicht erneut aus.

## 12. Tests

`cargo test --workspace` führt die Snapshot-, Diff- und SQLite-Tests aus, darunter den CLI-Lauf gegen eine temporäre Datei. Abgedeckt sind explizite Spalten- und Tabellenumbenennung, erfolgreicher und fehlgeschlagener Umbau, blockiertes `NOT NULL` bei vorhandenen `NULL`-Werten, passende und abweichende Baseline, eine destruktive Kette ohne Baseline und `reseal` für eine unveränderte sowie eine bereits angewendete Datei. Die Entwicklungsdatenbank `db.sqlite3` wird dabei nicht verwendet. PostgreSQL bleibt ignoriert.

## 13. Nicht implementiert

`migrate down`, nachträgliche Foreign-Key-Änderung auf bestehenden Spalten, Baseline für PostgreSQL, Ersatzwerte für `NULL`, Advisory-Lock, automatische Migration beim Serverstart, MySQL, Many-to-Many und Eager Loading. Typänderungen außer `string` ↔ `i64` und außer Nullbarkeit oder Bool-Default bei gleicher Art werden abgelehnt.
