# Validation – Rules, Types and Errors

[For overview](index.md) · **IST:** Rule generation in `crates/ferrioxid-macros/src/lib.rs`; error in `crates/ferrioxid-core/src/lib.rs`.]

## Current Rules (IST)

| Attribut | Typ |Test|
|---|---|---|
| `required` | `String` |`trim().is_empty()` is rejected|
| `min_length` | `String` | Mindestzahl von `trim().chars().count()` |
| `max_length` | `String` |Maximum number of `trim().chars().count()`|
| `min` | `i64` |Cell value must be ≥ integer literal|
| `default = true/false` | `bool` |Missing field and JSON-`null` in the Create-DTO become the default, plus SQL `DEFAULT`|
| `readonly` |Each supported field|Create sets the value; PATCH cannot send it|
| `unique` |Each supported field except the primary key|database unique index; Duplicate becomes 422|
| `unique_together` |at least two fields on the model|Composite Unique Index|
| `index`, `index_together` |Field or field group|Secondary index, not value check|
| `foreign_key`, `on_delete` |one column|Database foreign key; invalid reference and `restrict` become 422|
| `Option<T>` | `String`, `i64`, `bool`, `Uuid` |Create: missing or `null` stores SQL `NULL`. PATCH: see `PatchField`|

No general validator registry, no regex and no email check. Rust checks length, duty and `min` in `create` and `update`. Unique and Foreign Key checks the database. The repository layer translates the sqlx error type into `Validation`. Multiple `NULL` remain allowed in a unique index. Details are available in [constraints.md](constraints.md)].

### PATCH-Null-Semantik

`ferrioxid::db::PatchField<T>` stands on any changeable update field, with `#[serde(default)]`.

| JSON | Zustand | `NOT NULL`-Spalte | nullable Spalte (`Option<T>`) |
|---|---|---|---|
|Key missing| `Missing` |Previous value remains|Previous value remains|
| `null` | `Null` | HTTP 422, Detail `{feld} cannot be null` | SQL `NULL` |
| Wert | `Value` |Validate, then store original value|Validate, then store original value|

`{}` is the case where each field is `Missing`. The answer is HTTP 200 with the unchanged record, if the UUID exists. Strings are not stored trimmed. `required` rejects empty and pure whitespace texts in the `Value` arm.

## Example test cases (IST)

```json
{"name":"   ","price_cents":10}
{"name":"ok","price_cents":10}
{"name":"Laptop","price_cents":-1}
```

For the current `Product`, the three Create bodies are 422, because Blank, name shorter than 3 or negative price. `active` may be missing in the Create; `active: null` also becomes `true`. In PATCH, however, `{"active": null}` is 422 because the column is `NOT NULL`. A string with leading or following spaces is checked but not stored trimmed.

## Common Validator Interface

1. Deklarative Feldregeln vom Makro in neutralen Metadaten ablegen.
2. Compile-time type checks (e.g. `min` on numbers only).
3. Runtime checks for create/update with **clear PATCH semantics**: not available vs. explicit `null` vs. Value.
4. Uniform structured errors with field paths and machine codes.
5. Database Unique Constraints as Final Authority: Prechecks are no protection against Race Conditions.
6. Cross-field validators only after determining the order and transaction strategy.
7. Serde parsing errors are cleanly distinguished from domain validation errors.

## Testmatrix

`required` without `min_length`; Unicode, spaces, empty strings; bool-default DDL/DTO; Compile-fail type combinations; SQL constraints. For `422` see [api.md](api.md)].
