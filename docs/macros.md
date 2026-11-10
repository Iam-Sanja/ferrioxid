# ferrioxid-macros – compiler-based model generation

[Overview](index.md) · **Implemented file:** `crates/ferrioxid-macros/src/lib.rs`.

## Responsibility and Processing Steps (Implemented)

1. `#[derive(FerrioxidModel)]` receives `syn::DeriveInput`.
2. `#[ferrioxid(...)]` reads `table`, `api`, `allow_anonymous`, `admin`, `admin_label` and `admin_permission`. Fields also read `#[admin(...)]`. The admin data generates `admin_meta()` and no additional column. Details: [admin-customization.md](admin-customization.md).
3. Only Structs with named fields are allowed.
4. `#[field(...)]` is checked per field.
5. SQL data type, constraints and generated Create DTO are determined.
6. `quote!` generates `FerrioxidModel` and `ModelRepository` implementation plus `ProductCreate`-like type.
7. Incorrect attributes generate `syn::Error` as a compiler diagnosis.

### Model syntax (Implemented, path `examples/myshop/src/models/product.rs`)

```rust
#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)] pub id: Uuid,
    #[field(required, index, min_length = 3, max_length = 150)] pub name: String,
    #[field(min = 0)] pub price_cents: i64,
    #[field(default = true)] pub active: bool,
    #[field(foreign_key = "categories.id", on_delete = "restrict", belongs_to = "super::category::Category")]
    pub category_id: Option<Uuid>,
}
```

The macro generates `pub struct ProductCreate` and `pub struct ProductUpdate` in the model’s module. Neither DTO contains `id`. With `#[field(default = true)]`, the create DTO uses `Option<bool>` and `#[serde(default)]` for `active`, replacing `None` with `true`. Read-only fields are omitted from update DTOs. Every writable update field uses `ferrioxid::db::PatchField<T>`. The model itself must derive `Serialize` and `Deserialize`.

## Permitted types (Implemented)

| Rust-Type | SQL | Rules | Create-DTO |
|---|---|---|---|
| `Uuid` | `UUID NOT NULL` | The primary key uses `primary_key, auto`; other UUID fields can reference models | `Uuid`; auto-generated primary keys are omitted |
| `String` | `TEXT NOT NULL` | `required`, `min_length`, `max_length` | `String`; Update `PatchField<String>` |
| `i64` | `BIGINT NOT NULL` |`min` as a non-negative integer literal| `i64`; Update `PatchField<i64>` |
| `bool` | `BOOLEAN NOT NULL` | `default = true/false` | `Option<bool>` when a default is set; updates use `PatchField<bool>` |
| `Option<String/i64/bool/Uuid>` | Type without `NOT NULL` | Length and `min` apply only to present values | `Option<T>` with `#[serde(default)]`; update uses `PatchField<T>` |

`id` cannot be nullable. `required` and `default` cannot be combined with `Option<T>`. `#[field(readonly)]` keeps a field in the create DTO and removes it from the update DTO; managed timestamp fields have their own rules in [model-timestamps.md](model-timestamps.md). Temporal model types `NaiveDate`, `NaiveTime` and `DateTime<Utc>` are supported; see [date-time-types.md](date-time-types.md). `Decimal` and `Choice` fields are supported. JSON fields, generic models and arbitrary custom field types are not implemented. `min = -5` is not an accepted literal because the attribute parser reads an integer token. The derive also emits `FieldContract` for OpenAPI: create and update membership, `min_length`, `max_length` and `format`. See [relations.md](relations.md) and [constraints.md](constraints.md) for implemented relationships and constraints.

## Validation (Implemented)

- `required` on `String`: `.trim().is_empty()` → `Validation`.
- `min_length`/`max_length`: `.trim().chars().count()`; Number of Unicode scalar values, **no** grapheme clusters and **no** bytes.
- `min` on `i64`: numerical comparison, not automatically SQL-CHECK-Constraint.
- `required` on `Uuid`/`i64`/`bool`: compiler error.
- `default` for bool: database DDL `DEFAULT TRUE`/`FALSE` and DTO fallback `unwrap_or(...)`.
- `null` at `active` is treated as `None` and uses the same default as a missing field.
- `trim()` checks the input, but does not save a trimmed version of the string.
- `min_length` must not be larger than `max_length`.
- PATCH: `PatchField::Missing` skips the column. `PatchField::Null` is a validation error `cannot be null` for `NOT NULL` and a SQL-`NULL` for nullable columns. `PatchField::Value` goes through the same string and `min` rules.

## SQL generation (Implemented)

- Identifiers: ASCII letter or underscore first, followed by letters, digits or underscores. SQL names are quoted with `"`.
- API path: static sections with the same identifier rules; no placeholders, `*` or `?`.
- DDL for the legacy bootstrap: `schema_sql(DatabaseKind)` calls `render_schema`. Migrations use the same column metadata, but the Crate `ferrioxid-migrations`.
- INSERT, LIST, GET, UPDATE and DELETE call `ferrioxid::db`. SeaQuery binds values and quotes identifiers.
- LIST reads 101 rows, returns at most 100 and sets `truncated`.
- UPDATE writes only fields whose `PatchField` is not `Missing`. An empty patch reads the current row.
- DELETE filters over the bound UUID and evaluates `rows_affected`.
- The database layer builds JSON internally and deserializes it into the model type. `to_jsonb` is not used.

## Limits and Known Risks (Implemented)

- The SQL text generated by the macro is not compared to the specific database schema version.
- Validation runs in `create` and `update`, not for any direct SQL access.
- Serde JSON HTTP parsing errors are from Axum, not `FerrioxidError`.
- Without a version field, there is no detection of lost parallel updates.
- Strict name checking excludes many special characters, not an SQL audit.

## Extension strategy (planned)

1. Parse into neutral internal `ParsedModel` instead of assembling SQL directly when parsing.
2. Generate normalized metadata (fields, SQL, API visibility, validation).
3. Separate code generation for ORM metadata, repository methods, DTOs and schema.
4. Version the attributes so that future defaults or relationships do not trigger silent change of meaning.
5. Compile fail tests (`trybuild` only after conscious dependency decision) for invalid combinations.
6. The three PATCH states are implemented. A common metadata structure remains open instead of directly generated SQL.
7. `syn::Error` to exact source spans, not `panic!`.

## Test cases and acceptance

- `String` with only `required`: `""`, `"   "` → 422.
- string length based on trimmed characters; Unicode tests.
- `default = true/false`: Check SQL and POST return.
- unknown attributes, multiple PK, missing `id`, invalid SQL names → compile errors.
- DTO serialization limit: Type-correctness without `Value` in the public repository signature.

**Do not implement here:** Network routing, database connections, admin interfaces.
