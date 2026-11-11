# Sicherheitsprüfung der Schreib-API

Stand: 11.11.2026. Branch `fix/postgres-portability-v2`. Geprüft wurden die öffentlichen Sammeländerungen und `lock_existing` vor dem ersten Push. Die Aussagen unten stammen aus ausgeführten Datenbanktests, nicht nur aus dem Lesen des Quelltexts.

## Geprüfte öffentliche APIs

- `Select::update_matching`, `Select::update_all`
- `Select::delete_matching`, `Select::delete_all`
- `ColumnUpdate::set`, `set_null`, `increment`, `decrement`
- `Select::lock_existing`
- `ModelQuery::insert` und `RowInsert::on_conflict_do_nothing`
- `WriteResult.rows_affected`, `LockResult.rows`

Der Filter, das Relationsprädikat und der `QueryScope` werden in `Select::write_filter` mit `AND` verbunden. `update_all` und `delete_all` schalten nur die Ablehnung einer leeren Bedingung ab. Sie entfernen einen gesetzten Scope nicht. `QueryScope::All` ist die ausdrückliche Anfrage ohne Mandantengrenze. `QueryScope::None` trifft keine Zeile.

## Gefundene Risiken

1. `bind_for_column` hat einen Wert akzeptiert, dessen Rust-Typ nicht zur Spalte passte. Ein Text für eine Ganzzahlspalte wurde gebunden. SQLite hat ihn gespeichert.
2. Ein Sammelupdate konnte die Spalte eines `QueryScope::Equals` ändern und die Zeile damit aus dem Scope des Aufrufers herausbewegen.
3. Eine Spalte mit Feldvertrag `in_update = false` (nur lesbar oder vom Framework verwaltet) konnte per Sammelupdate gesetzt werden. `ModelRepository::update` nimmt sie nicht entgegen.
4. `set` hat deklarierte Zahlengrenzen nicht geprüft. `increment` und `decrement` haben sie bereits geprüft.
5. Das SQLite-`lock_existing` hat den Primärschlüssel auf sich selbst geschrieben. Ein `AFTER UPDATE`-Trigger und `ON UPDATE CASCADE` wären für jede getroffene Zeile gelaufen.

## Reproduzierte Sicherheitsprobleme

`sqlite_write_security` hat den falschen Typ zuerst als erfolgreiches Update mit `rows_affected == 1` gesehen. Nach der Korrektur lehnt `bind_for_column` den Wert ab, bevor eine Anweisung entsteht. Dieselbe Prüfung gilt für PostgreSQL.

Ein Scope auf `owner_id` plus `set("owner_id", anderer Besitzer)` wird abgelehnt. Die Zeile bleibt beim ursprünglichen Besitzer. Dieselbe Änderung mit `QueryScope::All` und einem Filter auf die Zeile ist der ausdrückliche privilegierte Weg und bleibt möglich.

`set("sealed", 0)` auf einer Spalte mit `in_update = false` wird abgelehnt. Der gespeicherte Wert bleibt 7.

`set("stock", -1)` und `set("stock", 11)` bei Grenzen 0 bis 10 werden abgelehnt. `increment` an der Obergrenze und `decrement` an der Untergrenze ändern die Zeile nicht und zählen sie nicht.

## Reproduzierte Trigger-Nebenwirkungen

Vor der Korrektur hätte das Selbstupdate des Primärschlüssels den `AFTER UPDATE`-Trigger ausgeführt. Der jetzige SQLite-Pfad schreibt eine `UPDATE`-Anweisung, deren Bedingung keine Zeile trifft, und zählt danach die vom Aufrufer gefilterten Zeilen. Der Test legt einen `AFTER UPDATE`-Trigger und eine Kindtabelle mit `ON UPDATE CASCADE` an.

Nach `lock_existing` und `COMMIT` ist die Audit-Tabelle unverändert, und die Kindzeile verweist weiter auf dieselbe Elternzeile. Ein echtes `set` erhöht die Audit-Tabelle um eins. `ROLLBACK` nimmt den Audit-Eintrag dieses Updates und die Namensänderung zurück. PostgreSQL 18.6 zeigt dasselbe: `SELECT … FOR UPDATE` schreibt keinen Audit-Eintrag, ein echtes Update schreibt einen.

## SQLite-Sperrsemantik

SQLite hat keine Zeilensperre. `lock_existing` nimmt die datenbankweite Schreibreservierung der Datei, bis die Transaktion endet. Eine zweite Verbindung, die eine andere Zeile sperren will, wartet. Nach `COMMIT` der ersten läuft sie weiter. `ROLLBACK` gibt die Reservierung frei. Eine fehlende Zeile ergibt `rows == 0` und legt nichts an.

`busy_timeout` von 200 Millisekunden auf der zweiten Verbindung liefert einen Datenbankfehler, nachdem die Wartezeit abgelaufen ist. Dieselbe Verbindung schreibt danach erfolgreich. Der Fehler wird nicht geschluckt. Die Verbindung aus `connect` setzt `busy_timeout` auf fünf Sekunden. Eine sofort von SQLite gemeldete Belegung bleibt ein Datenbankfehler.

## PostgreSQL-Sperrsemantik

PostgreSQL sperrt nur die von `SELECT … FOR UPDATE` getroffenen Zeilen. Eine zweite Transaktion sperrt eine andere Zeile, ohne auf die erste zu warten. Dieselbe Zeile wartet bis `ROLLBACK` und schreibt danach. Eine fehlende Zeile ergibt `rows == 0`.

## Scope-Isolation

Mit Besitzer A und B und je zwei Zeilen, ausgeführt auf SQLite und PostgreSQL 18.6:

- `update_matching` mit Scope A ändert nur A.
- `update_all` mit Scope A ändert nur A.
- Ein zusätzlicher Filter und ein `OR`, das den anderen Besitzer nennt, bleiben innerhalb von A.
- Ein Update ohne Filter und ohne Scope wird abgelehnt.
- `QueryScope::None` mit `update_all` ändert nichts.
- `delete_matching` mit Scope A löscht nur die gefilterte Zeile von A.
- `delete_all` mit Scope eines dritten Besitzers löscht nur dessen Zeile.

## Filter- und SQL-Injection-Sicherheit

Der Spaltenname `name; DROP TABLE ferrioxid_write_notes` wird abgelehnt. Der Wert `x'; DROP TABLE ferrioxid_write_notes; --` wird als Text gespeichert. Die Tabelle hat danach weiterhin vier Zeilen. Der Primärschlüssel kann nicht gesetzt werden. Ein wiederholtes Anlegen derselben Id ist eine Unique-Validierung. Das Löschen einer Elternzeile mit Kind ist eine Fremdschlüssel-Validierung, einschließlich SQLSTATE 23001 auf PostgreSQL.

## Validierung und Hooks

Es gibt keine separaten Modell-Hooks, die der Einzelupdate zusätzlich aufruft. Die Einzel-API prüft Feldverträge: Pflicht, Länge, E-Mail, Slug, Zahlengrenzen, nur lesbare und verwaltete Spalten.

Der Sammelupdate übernimmt davon die Ablehnung nicht schreibbarer Spalten und die Zahlengrenzen für `set`, `increment` und `decrement`. Er prüft Textlänge, E-Mail und Slug nicht. Der Test speichert den Namen `ab`, obwohl der Vertrag eine Mindestlänge von 5 deklariert. Das ist beabsichtigt und nicht gleichwertig zu `ModelRepository::update`.

Ein Sammelupdate läuft als eine Anweisung. Er lädt die Zeilen nicht einzeln und ruft keine Per-Zeilen-Logik auf.

## Geänderte Dateien

- `crates/ferrioxid-db/src/write.rs`
- `crates/ferrioxid-db/src/query.rs`
- `.github/workflows/ci.yml`
- `docs/database.md`
- `docs/postgres-portability-v2.md`
- `docs/orm-capability-matrix.md`
- `docs/orm-write-security-audit.md`

## Neue Regressionstests

- `write::tests::sqlite_write_security`
- `write::tests::postgres_write_security`

Die vorhandenen `sqlite_shop_cases` und `postgres_shop_cases` sind nach der Sperränderung erneut gelaufen.

## SQLite-Testergebnisse

`sqlite_write_security` und `sqlite_shop_cases` sind bestanden. Der Sicherheitstest wartet auf die Schreibreservierung und auf `busy_timeout`. Er braucht dafür keine prozesslokale Sperre.

## PostgreSQL-18.6-Testergebnisse

Auf `127.0.0.1:5432` mit der Rolle `ferrioxid` und einer Wegwerfdatenbank `ferrioxid_test_` plus 32 Hexzeichen:

- `postgres_write_security` bestanden
- `postgres_shop_cases` bestanden

`ferrioxid` und `ferrioxid_migrations_down_test` wurden nicht beschrieben. Danach war keine `ferrioxid_test_*`-Datenbank mehr vorhanden. Die älteren PostgreSQL-Laufzeittests dieses Branches sind in diesem Audit nicht wiederholt worden.

## PostgreSQL-16-CI

`.github/workflows/ci.yml` ruft `postgres_write_security` zusätzlich auf. Vorhandene Tests bleiben im Workflow. Der Job ist nicht gelaufen, weil dieser Branch nicht gepusht wird.

## Verbleibende Einschränkungen

- Sammelupdates prüfen keine Textlänge, keine E-Mail und keinen Slug.
- SQLite sperrt die Datei für Schreiber, nicht eine Zeile.
- Eine noch nicht vorhandene Zeile lässt sich nicht per `lock_existing` reservieren.
- `on_conflict_do_nothing` überschreibt nicht. Ein Upsert mit Ersetzen gibt es nicht.
- Die PostgreSQL-16-CI ist nicht ausgeführt.

## Verhalten vor und nach der Korrektur

| Aufruf | Vorher | Nachher |
|---|---|---|
| `set` mit falschem Rust-Typ | Wert wurde gebunden | `Validation`, keine Anweisung |
| `set` der Scope-Spalte bei `QueryScope::Equals` | Zeile wechselte den Besitzer | `Validation`, Zeile bleibt |
| `set` einer Spalte mit `in_update = false` | Wert wurde geschrieben | `Validation` |
| `set` außerhalb der Zahlengrenze | Wert wurde geschrieben | `Validation` |
| SQLite `lock_existing` | `UPDATE` der getroffenen Zeilen, Trigger möglich | `UPDATE` ohne getroffene Zeile, dann Zählung. Trigger und `ON UPDATE CASCADE` laufen nicht |

`update_all` und `delete_all` haben den Scope schon vorher behalten. Die Tests halten das fest. Aufrufer, die eine ganze Tabelle ändern wollen, setzen `QueryScope::All` oder lassen den Standard-Scope stehen und rufen `update_all` oder `delete_all` auf.

## Lokale Commit-Hashes

- Korrektur und Regressionstests: `8eefb0edf07538af0333af7953a8da526724fc41`
- Dieser Bericht liegt im unmittelbar folgenden Commit auf `fix/postgres-portability-v2`. Es gibt keinen Push.
