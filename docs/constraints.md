# Constraints and Indices

[Overview](index.md) · **Implemented:** Rust models generate unique constraints, composite unique constraints, single-column and composite indexes, and foreign keys. The database enforces these constraints.

## Source

The Rust struct remains the source of truth. `#[field(...)]` and `#[ferrioxid(...)]` generate metadata in `ColumnSpec` and `TableConstraints`. `SchemaSnapshot::from_registered` checks rules across models. The derive macro cannot inspect other files and does not claim to validate foreign models.

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

## Names

| Object | Pattern |Example|
|---|---|---|
| Unique | `uq_{tabelle}_{spalten}` | `uq_categories_name` |
| Index | `ix_{tabelle}_{spalten}` | `ix_products_name` |
| Foreign Key | `fk_{tabelle}_{spalte}_{zieltabelle}_{zielspalte}` | `fk_products_category_id_categories_id` |

The names are deterministic. Identifiers remain limited to letters, numbers and underscores and are quoted in SQL using `"`.

A primary key already generates a unique index. `unique` or `index` on the same column is a compile error. An index with the same column list as a Unique Constraint is not created a second time.

## NULL in unique constraints

SQLite and PostgreSQL use their standard SQL behavior: unique indexes allow multiple `NULL` values because `NULL` values are not considered equal. Ferrioxid does not use `NULLS NOT DISTINCT`. Duplicate non-null values are rejected.

## SQL

Unique constraints are implemented as explicitly named unique indexes, not just Rust validation checks.

In `CREATE TABLE`, Ferrioxid writes the columns and the foreign keys into the table statement. Unique indexes and secondary indexes follow as separate `CREATE UNIQUE INDEX` and `CREATE INDEX` statements. This keeps the name visible in `sqlite_master` and `DROP INDEX` removes it.

A new nullable column can carry a foreign key in the same migration:

```sql
ALTER TABLE "products" ADD COLUMN "category_id" text CONSTRAINT "fk_products_category_id_categories_id" REFERENCES "categories" ("id") ON DELETE RESTRICT ON UPDATE NO ACTION
```

`ON UPDATE` is always `NO ACTION`. There is no attribute for this.

PostgreSQL uses the same `CREATE INDEX` statements and `ALTER TABLE ... ADD COLUMN`. A Unique Index or Foreign Key that violates existing lines makes the statement fail. The migration is rolled back and the history does not store success.

## Foreign-key rules

| `on_delete` | Effect |
|---|---|
| `restrict` |Deleting the parent line fails as long as children exist|
| `cascade` |Children are also deleted|
| `set_null` |The child column becomes `NULL`|

`set_null` is valid only on a nullable column. The macro rejects the combination with `NOT NULL`. Snapshot validation also rejects it.

In addition, `SchemaSnapshot::validate` checks:

- The destination table is registered in this snapshot or in an installed app listed in `depends_on`. A cross-app target requires that direct dependency; a transitive dependency is not enough. Missing apps, cycles, ambiguous table names and a dependency removal that would leave a foreign key unresolved stop schema generation and migration execution. Applied migration files and their checksums stay unchanged.
- The target column exists.
- The logical types agree.
- The target column is a primary key or a single-column unique constraint.
- Foreign keys have exactly one column.
- Names of Unique Constraints, Indices and Foreign Keys are unique in the table.
- `on_update` must be `no_action`.

A foreign key on an existing column, the removal of a foreign key and a changed `ON DELETE` rule create `add_foreign_key` or `drop_foreign_key`. These variants are not executed. SQLite cannot change it without a table conversion, and the conversion remains blocked on Foreign Keys. `migrate make` does not write a file.

Parent tables are created before child tables, and child tables are dropped before their parents. Cycles are rejected.

## Snapshot

The format version remains `1`. `unique_constraints`, `indexes` and `foreign_keys` are missing in the JSON if they are empty. As a result, already applied files such as `0001_initial.json` keep their checksum. Old files without these fields are read with empty lists.

The objects are in deterministic name order. `semantically_eq` compares them regardless of the list position.

## Renaming

`RenameColumn` and `RenameTable` update snapshot column lists and recompute deterministic constraint names. Foreign keys targeting the renamed column also update `referenced_columns`.

In the database, each affected unique or secondary index is dropped and recreated with its new name. The primary-key autoindex is unaffected. SQLite foreign-key constraints are not separate named schema objects; its rename operation updates the table definition. Changing a deletion rule remains blocked.

## Errors

`insert_record`, `update_record` and `delete_record` evaluate `sqlx::ErrorKind`.

|Non-compliance| Mapping |
|---|---|
| `UniqueViolation` | `FerrioxidError::Validation("unique constraint violated")` |
| `ForeignKeyViolation` | `FerrioxidError::Validation("foreign key constraint violated")` |
|SQLite code `1811` with fixed message `FOREIGN KEY constraint failed`|Same foreign key validation|

SQLite implements `ON DELETE RESTRICT` internally with `RAISE`, producing extended code `1811` (`SQLITE_CONSTRAINT_TRIGGER`) rather than `787`. Ferrioxid matches the fixed message from that implementation, not arbitrary error text. Other database errors remain `FerrioxidError::Database` and become HTTP 500 without exposing SQL.

## SQLite table rebuild

Tables with user index, trigger, view, outgoing or incoming foreign key are not rebuilt. The primary key auto index (`sql IS NULL`) does not count as a user index. A supported conversion only checks the changed column and then copies it with `INSERT INTO ... SELECT`. A type change `string` ↔ `i64` uses `CAST`, but only after the previous value check. Invalid values roll back the transaction.

## Not implemented

- Multicolumn foreign keys
- `ON UPDATE` except `NO ACTION`
- `NULLS NOT DISTINCT`
- Subsequent creation, removal or switching of a foreign key on an existing column
- Partial index, expression index, sorting direction
- Automatic index on each field
