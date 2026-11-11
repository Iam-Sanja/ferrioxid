# ORM-Fähigkeiten

Stand: 11.11.2026, Branch `fix/postgres-portability-v2`.

Die Tabelle beschreibt die öffentliche Ferrioxid-API für gewöhnliche Modelloperationen. SeaQuery bleibt im Framework. Anwendungs- und Geschäftslogik für die genannten Standardfälle braucht keine selbst geschriebenen SQL-Texte. Die Angaben sind an den Tests dieses Branch festgemacht, nicht an einer Produktionsfreigabe.

`SQLite getestet` bezieht sich auf die normale Workspace-Suite, einschließlich `write::tests::sqlite_shop_cases` und `examples/desk` `sqlite_same_operations`. `PostgreSQL getestet` bezieht sich auf den lokalen Lauf gegen PostgreSQL 18.6 mit isolierten Datenbanken `ferrioxid_test_` plus 32 Hexzeichen. Die PostgreSQL-16-CI enthält dieselben ignorierten Tests in `.github/workflows/ci.yml` und ist durch diesen Branch nicht ausgeführt worden.

| Fähigkeit | Stand | SQLite getestet | PostgreSQL getestet |
|---|---|---|---|
| Anlegen einer Zeile über `ModelRepository::create` | Vollständig. Feldregeln gelten. Der Primärschlüssel wird erzeugt. | Ja | Ja, über vorhandene Laufzeittests und `examples/desk` |
| Lesen, Filtern, Sortieren | Vollständig über `Select` | Ja | Ja |
| Seite und Keyset | Vollständig. Eine Seite hat höchstens 100 Zeilen. Offset-Seiten sind kein stabiler Snapshot. | Ja | Ja |
| `COUNT` und `SUM` über `annotate` | Teilweise. Eine Aggregation pro Aufruf, kein allgemeines `GROUP BY` | Ja | Ja |
| Relationen `where_related`, `where_has`, `where_linked` | Vollständig für die vorhandenen Beziehungstypen | Ja | Ja |
| Textsuche `contains_ignore_case` | Teilweise. SQLite faltet nur ASCII. PostgreSQL faltet nach der Locale von `LOWER`. | Ja, `München` entspricht nicht `MÜNCHEN` | Ja, auf diesem UTF-8-Cluster entspricht es. Eine `C`-Locale tut das nicht. |
| Sortierung | Teilweise. SQLite vergleicht `BINARY`. PostgreSQL verwendet die Datenbank-Collation. | Ja, `Muenchen` vor `München` | Ja, die Reihenfolge wird nicht als identisch behauptet |
| Einzelupdate und Einzellöschen | Vollständig, einschließlich Transaktion und `QueryScope` | Ja | Ja |
| Gefiltertes Update mehrerer Zeilen | Vollständig. Ohne Filter nur über `update_all`. | Ja | Ja |
| Gefiltertes Löschen mehrerer Zeilen | Vollständig. Das Seitenlimit gilt nicht. Ohne Filter nur über `delete_all`. | Ja, 101 Zeilen | Ja, 101 Zeilen |
| Atomares `set` | Vollständig. Der Wert wird gebunden. | Ja | Ja |
| Atomares `increment` und `decrement` | Vollständig für Ganzzahlspalten. Betrag größer als null. Überlauf, Unterlauf und Modellgrenzen ändern die Zeile nicht. | Ja | Ja |
| Multiplikation und Division | Nicht unterstützt | Nein | Nein |
| Transaktion und Rollback | Vollständig über `begin`, `commit` und `rollback` | Ja | Ja |
| Zeilensperre | Teilweise und nicht gleichbedeutend. PostgreSQL: `SELECT … FOR UPDATE` auf getroffene Zeilen. SQLite: datenbankweite Schreibreservierung durch eine `UPDATE`-Anweisung ohne getroffene Zeile. Trigger und `ON UPDATE CASCADE` laufen dafür nicht. Null Zeilen reservieren nichts. | Ja, dateiweite Schreibsperre | Ja, zeilenweise `FOR UPDATE` |
| Erstanlage gegen Doppelanlage | Vollständig nur mit echtem Unique-Constraint. Ein vorheriges Lesen genügt nicht. `on_conflict_do_nothing` lässt die vorhandene Zeile stehen. | Ja | Ja |
| Upsert mit Überschreiben | Nicht unterstützt | Nein | Nein |
| `rows_affected` | Vollständig bei Insert, Update und Delete dieser API | Ja | Ja |
| `QueryScope` auf Sammeländerungen | Vollständig. `None` trifft nichts. `Equals` ist eine Einschränkung. | Ja | Ja |
| Modell-Feldregeln auf Sammeländerungen | Nicht ausgeführt. Das ist ausdrücklich so. `create` und `update` einer einzelnen Zeile prüfen sie weiterhin. | Ja, der Unterschied ist dokumentiert | Ja, derselbe Code |
| Fremdschlüssel | Vollständig nach der deklarierten Aktion. `RESTRICT` wird als Validierung gemeldet, auf PostgreSQL auch für SQLSTATE 23001. | Ja | Ja |
| SQL-Injection über Spaltennamen oder Werte | Abgewiesen, bevor eine Anweisung entsteht. Werte sind Parameter. | Ja | Ja, derselbe Ablehnungspfad |
| MySQL | Nicht unterstützt | Nein | Nein |
| Optimistisches Locking | Nicht unterstützt | Nein | Nein |

Eine SQLite-Verbindung hat höchstens eine Pool-Verbindung und `busy_timeout` von fünf Sekunden. Das ist keine Zeilensperre und kein Ersatz für einen Unique-Constraint. Zwei Verbindungen auf dieselbe Datei können parallel schreiben. Eine Sperre, die SQLite sofort als belegt meldet, bleibt ein Datenbankfehler.
