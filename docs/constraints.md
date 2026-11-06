# Constraints und Indizes

[Zur Übersicht](index.md) · **IST.** Unique Constraints, zusammengesetzte Unique Constraints, einzelne und zusammengesetzte Indizes sowie Foreign Keys werden aus dem Rust-Modell erzeugt und von der Datenbank durchgesetzt.

## Quelle

Die Rust-Struct bleibt die Quelle. `#[field(...)]` und `#[ferrum(...)]` schreiben Metadaten in `ColumnSpec` und `TableConstraints`. `SchemaSnapshot::from_registered` prüft modellübergreifende Regeln. Das Derive-Makro sieht andere Dateien nicht und erfindet deshalb keine Prüfung über fremde Modelle.

```rust
#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(table = "categories", api = "/api/v1/categories", allow_anonymous = true)]
pub struct Category {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, unique, max_length = 100)]
    pub name: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "products",
    api = "/api/v1/products",
    allow_anonymous = true,
    index_together = "category_id,name"
)]
pub struct Product {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, index, min_length = 3, max_length = 150)]
    pub name: String,

    #[field(foreign_key = "categories.id", on_delete = "restrict")]
    pub category_id: Option<Uuid>,
}
```

`unique_together` und `index_together` stehen am Modell, nicht am Feld. Mehrere Gruppen sind mehrere Attribute. Eine Gruppe braucht mindestens zwei Spalten. Unbekannte Spalten, der Primärschlüssel und eine Wiederholung einer Unique-Gruppe als Index sind Compile-Fehler.

## Namen

| Objekt | Muster | Beispiel |
|---|---|---|
| Unique | `uq_{tabelle}_{spalten}` | `uq_categories_name` |
| Index | `ix_{tabelle}_{spalten}` | `ix_products_name` |
| Foreign Key | `fk_{tabelle}_{spalte}_{zieltabelle}_{zielspalte}` | `fk_products_category_id_categories_id` |

Die Namen sind deterministisch. Bezeichner bleiben auf Buchstaben, Ziffern und Unterstriche beschränkt und werden in SQL mit `"` quotiert.

Ein Primärschlüssel erzeugt bereits einen eindeutigen Index. `unique` oder `index` auf derselben Spalte ist ein Compile-Fehler. Ein Index mit derselben Spaltenliste wie ein Unique Constraint wird nicht ein zweites Mal angelegt.

## NULL bei Unique Constraints

Es gilt das SQL-Standardverhalten von SQLite und PostgreSQL: mehrere `NULL`-Werte sind in einem Unique Index erlaubt. `NULL` gilt nicht als gleich `NULL`. Ferrum setzt kein `NULLS NOT DISTINCT`. Ein zweiter konkreter Wert, der schon vorkommt, wird abgelehnt.

## SQL

Unique Constraints sind eindeutig benannte Unique Indexes, keine reine Rust-Prüfung.

Beim `CREATE TABLE` schreibt Ferrum die Spalten und die Foreign Keys in die Tabellenanweisung. Unique Indexes und sekundäre Indexes folgen als eigene `CREATE UNIQUE INDEX`- und `CREATE INDEX`-Anweisungen. Dadurch bleibt der Name in `sqlite_master` sichtbar und `DROP INDEX` entfernt ihn.

Eine neue nullable Spalte kann in derselben Migration einen Foreign Key tragen:

```sql
ALTER TABLE "products" ADD COLUMN "category_id" text CONSTRAINT "fk_products_category_id_categories_id" REFERENCES "categories" ("id") ON DELETE RESTRICT ON UPDATE NO ACTION
```

`ON UPDATE` ist immer `NO ACTION`. Es gibt kein Attribut dafür.

PostgreSQL verwendet dieselben `CREATE INDEX`-Anweisungen und `ALTER TABLE ... ADD COLUMN`. Ein Unique Index oder Foreign Key, der vorhandene Zeilen verletzt, lässt die Anweisung fehlschlagen. Die Migration wird zurückgerollt und die Historie speichert keinen Erfolg.

## Foreign-Key-Regeln

| `on_delete` | Wirkung |
|---|---|
| `restrict` | Löschen der Elternzeile schlägt fehl, solange Kinder existieren |
| `cascade` | Kinder werden mitgelöscht |
| `set_null` | Die Kindspalte wird `NULL` |

`set_null` ist nur auf einer nullable Spalte gültig. Das Makro lehnt die Kombination mit `NOT NULL` ab. Die Snapshot-Validierung lehnt sie ebenfalls ab.

Zusätzlich prüft `SchemaSnapshot::validate`:

- Die Zieltabelle ist im selben Snapshot registriert.
- Die Zielspalte existiert.
- Die logischen Typen stimmen überein.
- Die Zielspalte ist Primärschlüssel oder ein einspaltiger Unique Constraint.
- Foreign Keys haben genau eine Spalte.
- Namen von Unique Constraints, Indizes und Foreign Keys sind in der Tabelle eindeutig.
- `on_update` muss `no_action` sein.

Ein Foreign Key auf einer schon vorhandenen Spalte, das Entfernen eines Foreign Keys und eine geänderte `ON DELETE`-Regel erzeugen `add_foreign_key` oder `drop_foreign_key`. Diese Varianten werden nicht ausgeführt. SQLite kann sie nicht ohne Tabellenumbau ändern, und der Umbau bleibt bei Foreign Keys blockiert. `migrate make` schreibt dann keine Datei.

Eltern-Tabellen werden vor Kind-Tabellen erzeugt. Zyklen werden abgelehnt. Tabellen mit Kindern werden zuerst gelöscht.

## Snapshot

Die Formatversion bleibt `1`. `unique_constraints`, `indexes` und `foreign_keys` fehlen im JSON, wenn sie leer sind. Dadurch behalten bereits angewendete Dateien wie `0001_initial.json` ihre Checksum. Alte Dateien ohne diese Felder werden mit leeren Listen gelesen.

Die Objekte stehen in deterministischer Namensreihenfolge. `semantically_eq` vergleicht sie unabhängig von der Listenposition.

## Umbenennung

`RenameColumn` und `RenameTable` schreiben die Spaltenlisten im Snapshot um und berechnen die deterministischen Namen neu. Verweist ein anderer Foreign Key auf die umbenannte Spalte, wird auch `referenced_columns` nachgezogen.

In der Datenbank folgt auf die Umbenennung `DROP INDEX` und `CREATE INDEX` für jeden betroffenen Unique Index und sekundären Index. Der Primärschlüssel-Autoindex bleibt unberührt. Foreign-Key-Namen existieren in SQLite nicht als eigenes Objekt. Die Definition in `CREATE TABLE` folgt der Spaltenumbenennung des Dialekts. Ein nachträgliches Ändern der Löschregel bleibt blockiert.

## Fehler

`insert_record`, `update_record` und `delete_record` werten `sqlx::ErrorKind` aus.

| Verstoß | Abbildung |
|---|---|
| `UniqueViolation` | `FerrumError::Validation("unique constraint violated")` |
| `ForeignKeyViolation` | `FerrumError::Validation("foreign key constraint violated")` |
| SQLite-Code `1811` mit der festen Meldung `FOREIGN KEY constraint failed` | dieselbe Foreign-Key-Validierung |

SQLite setzt `ON DELETE RESTRICT` intern als `RAISE` um. Der erweiterte Code ist dann `1811` (`SQLITE_CONSTRAINT_TRIGGER`) und nicht `787`. Die Meldung ist der feste Text dieser Implementierung, kein freier Vergleich beliebiger Fehltexte. Andere Datenbankfehler bleiben `FerrumError::Database` und werden an der HTTP-Grenze zu 500 ohne SQL-Text.

## SQLite-Umbau

Tabellen mit Nutzer-Index, Trigger, View, ausgehendem oder eingehendem Foreign Key werden nicht umgebaut. Der Primärschlüssel-Autoindex (`sql IS NULL`) zählt nicht als Nutzer-Index. Ein unterstützter Umbau prüft nur die geänderte Spalte und kopiert danach mit `INSERT INTO ... SELECT`. Eine Typänderung `string` ↔ `i64` verwendet `CAST`, aber erst nach der bisherigen Wertprüfung. Ungültige Werte rollen die Transaktion zurück.

## Nicht implementiert

- Mehrspaltige Foreign Keys
- `ON UPDATE` außer `NO ACTION`
- `NULLS NOT DISTINCT`
- Nachträgliches Anlegen, Entfernen oder Umschalten eines Foreign Keys auf einer bestehenden Spalte
- Partieller Index, Ausdrucksindex, Sortierrichtung
- Automatischer Index auf jedem Feld
