# Validation – Rules, Types and Errors

[Overview](index.md) · **Implemented:** Rule generation in `crates/fexu-macros/src/lib.rs`; error in `crates/fexu-core/src/lib.rs`.

## Current Rules (Implemented)

| Attribute | Type |Test|
|---|---|---|
| `required` | `String` |`trim().is_empty()` is rejected|
| `min_length` | `String` | Minimum value of `trim().chars().count()` |
| `max_length` | `String` |Maximum number of `trim().chars().count()`|
| `min` | `i64` |Cell value must be ≥ integer literal|
| `default = true/false` | `bool` |Missing field and JSON-`null` in the Create-DTO become the default, plus SQL `DEFAULT`|
| `readonly` |Each supported field|Create sets the value; PATCH cannot send it|
| `unique` |Each supported field except the primary key|database unique index; Duplicate becomes 422|
| `unique_together` |at least two fields on the model|Composite Unique Index|
| `index`, `index_together` |Field or field group|Secondary index, not value check|
| `foreign_key`, `on_delete` |one column|Database foreign key; invalid reference and `restrict` become 422|
| `Option<T>` | Supported scalar and date/time types | Create: missing or `null` stores SQL `NULL`. PATCH: see `PatchField` |

Generated model validators check required fields, string length, `min` and explicitly declared `format = "email"` or `format = "slug"` rules during `create` and `update`. They do not provide a general validator registry or regex validation. Form-level helpers are described in [forms-validation.md](forms-validation.md). The database enforces unique and foreign-key constraints, and the repository maps those sqlx errors to `Validation`. Unique indexes allow multiple `NULL` values. See [constraints.md](constraints.md).

### PATCH null semantics

`fexu::db::PatchField<T>` stands on any changeable update field, with `#[serde(default)]`.

| JSON | State | `NOT NULL` column | Nullable column (`Option<T>`) |
|---|---|---|---|
|Key missing| `Missing` |Previous value remains|Previous value remains|
| `null` | `Null` | HTTP 422, Detail `field:<feld>: validation.null` | SQL `NULL` |
| Value | `Value` |Validate, then store original value|Validate, then store original value|

`{}` is the case where each field is `Missing`. The answer is HTTP 200 with the unchanged record, if the UUID exists. Strings are not stored trimmed. `required` rejects empty and pure whitespace texts in the `Value` arm.

## Example test cases (Implemented)

```json
{"name":"   ","price_cents":10}
{"name":"ok","price_cents":10}
{"name":"Laptop","price_cents":-1}
```

For the current `Product`, the three Create bodies are 422, because Blank, name shorter than 3 or negative price. `active` may be missing in the Create; `active: null` also becomes `true`. In PATCH, however, `{"active": null}` is 422 because the column is `NOT NULL`. A string with leading or following spaces is checked but not stored trimmed.

## Planned common validator interface

1. Store declarative field rules from the macro in neutral metadata.
2. Compile-time type checks (e.g. `min` on numbers only).
3. Runtime checks for create/update with **clear PATCH semantics**: not available vs. explicit `null` vs. Value.
4. Uniform structured errors with field paths and machine codes.
5. Database Unique Constraints as Final Authority: Prechecks are no protection against Race Conditions.
6. Cross-field validators only after determining the order and transaction strategy.
7. Serde parsing errors are cleanly distinguished from domain validation errors.

## Test matrix

`required` without `min_length`; Unicode, spaces, empty strings; bool-default DDL/DTO; Compile-fail type combinations; SQL constraints. For `422` see [api.md](api.md).
