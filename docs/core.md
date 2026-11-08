# ferrioxid-core – contracts and model metadata

[Overview](index.md) · **Implemented file:** `crates/ferrioxid-core/src/lib.rs`.

## Authentication (v0.7)

`permission` is `None`, `"authenticated"` or a resource name. `requires_object_policy` marks models whose record access requires a registered `ObjectPolicy`. `FerrioxidError` includes `Denied(String)` and `Unauthorized(String)`; see [api.md](api.md) for HTTP mappings.

## Responsibility

`ferrioxid-core` contains the common contract types independent of HTTP and ORM. The Crate may **not** assume a dependency on `ferrioxid-api`, Axum or a specific database connection. Since v0.12, the form field rules are in `validation.rs`. The model macro does not call them yet. The description is in [forms-validation.md](forms-validation.md).

## Actual Public Types (Implemented)

```rust
pub struct ModelDescriptor {
    pub name: &'static str,
    pub table: &'static str,
    pub api_path: &'static str,
    pub allow_anonymous: bool,
    pub permission: Option<&'static str>,
    pub requires_object_policy: bool,
}
pub enum DatabaseKind { Sqlite, Postgres }
pub enum ColumnKind { Uuid, Text, BigInt, Boolean, Date, Time, DateTime }
pub enum ReferentialAction { Restrict, Cascade, SetNull }
pub struct ColumnSpec {
    pub name: &'static str,
    pub kind: ColumnKind,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_bool: Option<bool>,
    pub unique: bool,
    pub indexed: bool,
    pub foreign_table: Option<&'static str>,
    pub foreign_column: Option<&'static str>,
    pub on_delete: Option<ReferentialAction>,
}
pub struct TableConstraints {
    pub unique_groups: &'static [&'static [&'static str]],
    pub index_groups: &'static [&'static [&'static str]],
}
pub trait FerrioxidModel: Send + Sync + 'static {
    fn descriptor() -> &'static ModelDescriptor;
    fn columns() -> &'static [ColumnSpec];
    fn constraints() -> &'static TableConstraints;
    fn schema_sql(kind: DatabaseKind) -> String;
}
pub enum FerrioxidError {
    Validation(String),
    Forbidden,
    NotFound,
    Database(Box<dyn Error + Send + Sync>),
}
```

The public-type excerpt above is abbreviated: `std::error::Error` is written in full in the source, and `FerrioxidError` also includes `Denied(String)` and `Unauthorized(String)` as described above. `FerrioxidError::database(error)` wraps compatible errors in `Database`.

### Semantics

- `name`: Rust type name, generated with `stringify!`.
- `table`: already macro-side validated SQL table name.
- `api_path`: static path, e.g. `/api/v1/products`.
- `allow_anonymous`: explicit release for demo routes; Standard `false`.
- `columns`: dialect-independent column metadata from the macro.
- `schema_sql(kind)`: Bootstrap DDL for SQLite or PostgreSQL. The text comes from SeaQuery, not from a stored SQL string.

`unique`, `indexed` and the foreign key fields are in `ColumnSpec`. Composite groups are in `TableConstraints`. `constraints()` provides empty groups for old models. `Date`, `Time` and `DateTime` are `ColumnKind`. Policies are not included. Migrations build the snapshot from this metadata. The name helpers `unique_index_name`, `secondary_index_name` and `foreign_key_name` are in the same crate. Details are available in [constraints.md](constraints.md) and [relations.md](relations.md).

## Error Semantics (Implemented)

| Error | Meaning | HTTP-Mapping |
|---|---|---|
| `Validation` |Invalid Entries, Unique or Foreign Key Violation| 422 |
| `Forbidden` | Access denied | 403 |
| `NotFound` | Record is missing | 404 |
| `Database` |Database, query or data deserialization|500, internal detail only in the log|

The conversion to HTTP takes place **not** in `ferrioxid-core`, but exclusively in `ferrioxid-api`; see [api.md](api.md).

## Draft extension (planned)

1. Richer field metadata beyond `ColumnSpec`: constraints, API visibility and defaults other than Bool.
2. `RelationDescriptor` with relation type and foreign-key reference.
3. `ModelSchema` with table, PK, fields, indices and constraints.
4. `Registry` for **explicitly registered** models.
5. Descriptor Schema version for stable further development of the format.

Design rules: column metadata remains the canonical representation. Generate SQL for a specific `DatabaseKind`. MySQL support requires its own test environment and tests.

## Testing and acceptance

- Implemented: compile types in the workspace (check on site).
- REQUIREMENT: Unit tests for unique table/field names, stable registry order, and unique model IDs.
- For new public types: document SemVer impact and migration of old descriptor calls.

**Do not implement here:** SQL execution, Axum handler, CLI parsing or HTML templates.

## NotFound (Implemented)

`FerrioxidError::NotFound` is the error for missing resources. The Axum adapters provide `404` with the same JSON format as for `Validation` and `Forbidden`.

## Admin metadata

`FerrioxidModel::admin_meta` provides `None` as standard. The macro overrides the method only at `admin = true`. `AdminModelMeta` and `AdminFieldMeta` are next to `ModelDescriptor` and do not change a column. Sensitive fields remain recognizable by the name and are removed from the admin API. Tests: `examples/myshop/tests/admin_http.rs`. Limit: the metadata describes the interface, not a second schema snapshot.
