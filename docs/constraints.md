# Constraints and Indices

[For overview](index.md) · **IST.** Unique constraints, composite unique constraints, individual and composite indices as well as foreign keys are generated from the Rust model and enforced by the database.]

## Quelle

Die Rust-Struct bleibt die Quelle. `#[field(...)]` und `#[ferrioxid(...)]` schreiben Metadaten in `ColumnSpec` und `TableConstraints`. `SchemaSnapshot::from_registered` prüft modellübergreifende Regeln. Das Derive-Makro sieht andere Dateien nicht und erfindet deshalb keine Prüfung über fremde Modelle.

```rust
#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(table = "categories", api = "/api/v1/categories", allow_anonymous = true)]
pub struct Category {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, unique, max_length = 100)]
    pub name: String,
}

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
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

`unique_together` and `index_together` are on the model, not on the field. Multiple groups are multiple attributes. A group needs at least two columns. Unknown columns, the primary key and a repetition of a unique group as an index are compile errors.

## Namen

| Objekt | Muster |Example|
|---|---|---|
| Unique | `uq_{tabelle}_{spalten}` | `uq_categories_name` |
| Index | `ix_{tabelle}_{spalten}` | `ix_products_name` |
| Foreign Key | `fk_{tabelle}_{spalte}_{zieltabelle}_{zielspalte}` | `fk_products_category_id_categories_id` |

The names are deterministic. Identifiers remain limited to letters, numbers and underscores and are quoted in SQL using `"`.

A primary key already generates a unique index. `unique` or `index` on the same column is a compile error. An index with the same column list as a Unique Constraint is not created a second time.

## NULL bei Unique Constraints

The standard SQL behavior of SQLite and PostgreSQL applies: several `NULL` values are allowed in a unique index. `NULL` is not considered to be `NULL`. Ferrioxide does not set `NULLS NOT DISTINCT`. A second concrete value, which already occurs, is rejected.

## SQL

Unique Constraints are clearly named Unique Indexes, not a pure Rust test.

In `CREATE TABLE`, ferrioxide writes the columns and the foreign keys into the table statement. Unique indexes and secondary indexes follow as separate `CREATE UNIQUE INDEX` and `CREATE INDEX` statements. This keeps the name visible in `sqlite_master` and `DROP INDEX` removes it.

A new nullable column can carry a foreign key in the same migration:

```sql
ALTER TABLE "products" ADD COLUMN "category_id" text CONSTRAINT "fk_products_category_id_categories_id" REFERENCES "categories" ("id") ON DELETE RESTRICT ON UPDATE NO ACTION
```

`ON UPDATE` is always `NO ACTION`. There is no attribute for this.

PostgreSQL uses the same `CREATE INDEX` statements and `ALTER TABLE ... ADD COLUMN`. A Unique Index or Foreign Key that violates existing lines makes the statement fail. The migration is rolled back and the history does not store success.

## Foreign-Key-Regeln

| `on_delete` | Wirkung |
|---|---|
| `restrict` |Deleting the parent line fails as long as children exist|
| `cascade` |Children are also deleted|
| `set_null` |The child column becomes `NULL`|

`set_null` is valid only on a nullable column. The macro rejects the combination with `NOT NULL`. Snapshot validation also rejects it.

In addition, `SchemaSnapshot::validate` checks:

- The destination table is registered in the same snapshot.
- The target column exists.
- The logical types agree.
- The target column is a primary key or a single-column unique constraint.
- Foreign keys have exactly one column.
- Names of Unique Constraints, Indices and Foreign Keys are unique in the table.
- `on_update` must be `no_action`.

A foreign key on an existing column, the removal of a foreign key and a changed `ON DELETE` rule create `add_foreign_key` or `drop_foreign_key`. These variants are not executed. SQLite cannot change it without a table conversion, and the conversion remains blocked on Foreign Keys. `migrate make` does not write a file.

Parent tables are created before child tables. Cycles are rejected. Tables with children are deleted first.

## Snapshot

The format version remains `1`. `unique_constraints`, `indexes` and `foreign_keys` are missing in the JSON if they are empty. As a result, already applied files such as `0001_initial.json` keep their checksum. Old files without these fields are read with empty lists.

The objects are in deterministic name order. `semantically_eq` compares them regardless of the list position.

## Umbenennung

`RenameColumn` and `RenameTable` rewrite the column lists in the snapshot and recalculate the deterministic names. If another foreign key refers to the renamed column, `referenced_columns` is also drawn.

In the database, the renaming of `DROP INDEX` and `CREATE INDEX` follows for each affected Unique Index and Secondary Index. The primary key auto index remains unaffected. Foreign key names do not exist in SQLite as their own object. The definition in `CREATE TABLE` follows the column renaming of the dialect. Subsequently changing the deletion rule remains blocked.

## Fehler

`insert_record`, `update_record` and `delete_record` evaluate `sqlx::ErrorKind`.

|Non-compliance| Abbildung |
|---|---|
| `UniqueViolation` | `FerrioxidError::Validation("unique constraint violated")` |
| `ForeignKeyViolation` | `FerrioxidError::Validation("foreign key constraint violated")` |
|SQLite code `1811` with fixed message `FOREIGN KEY constraint failed`|Same foreign key validation|

SQLite implements `ON DELETE RESTRICT` internally as `RAISE`. The extended code is then `1811` (`SQLITE_CONSTRAINT_TRIGGER`) and not `787`. The message is the fixed text of this implementation, not a free comparison of any mistexts. Other database errors remain `FerrioxidError::Database` and become at the HTTP border to 500 without SQL text.

## SQLite-Umbau

Tables with user index, trigger, view, outgoing or incoming foreign key are not rebuilt. The primary key auto index (`sql IS NULL`) does not count as a user index. A supported conversion only checks the changed column and then copies it with `INSERT INTO ... SELECT`. A type change `string` ↔ `i64` uses `CAST`, but only after the previous value check. Invalid values roll back the transaction.

## Not implemented

- Multicolumn foreign keys
- `ON UPDATE` except `NO ACTION`
- `NULLS NOT DISTINCT`
- Subsequent creation, removal or switching of a foreign key on an existing column
- Partial index, expression index, sorting direction
- Automatic index on each field
