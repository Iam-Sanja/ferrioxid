# Reusable model fields

[Zur Übersicht](index.md) · Modelle: [models.md](models.md) · Zeitstempel: [model-timestamps.md](model-timestamps.md)

**ACT.** Functional status v0.26. A field group is not a table, a REST route and an admin registration.

Rust cannot reliably look up the fields of another type in a Derive. Ferrioxide therefore inserts the fields into the concrete structure with a macro before `FerrioxidModel` sees them. The stored columns are flat.

## Eigene Feldgruppe

```rust
use ferrioxid::prelude::*;

ferrioxid::model_fields! {
    pub struct AuditStamp {
        #[field(managed = "created")]
        pub created_at: ferrioxid::chrono::DateTime<ferrioxid::chrono::Utc>,
        #[field(managed = "updated")]
        pub updated_at: ferrioxid::chrono::DateTime<ferrioxid::chrono::Utc>,
    }
}

ferrioxid_fields_AuditStamp! {
    #[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
    #[ferrioxid(table = "products", api = "/api/v1/products", allow_anonymous = true)]
    pub struct Product {
        #[field(primary_key, auto)]
        pub id: Uuid,
        #[field(required, min_length = 1)]
        pub name: String,
    }
}
```

`model_fields!` generates the structure `AuditStamp` without table and the macro `ferrioxid_fields_AuditStamp!` in the same crate. The macro copies the fields in `Product`. A second structure can use the same macro and get the same columns in another table.

The same field names in the concrete structure and in the group are rejected by the compiler. An unknown attribute value as well. The group itself has no migration.

## Django

| Django | Ferrioxid |
|---|---|
| abstraktes Model |`model_fields!`, without table|
| Vererbung |Macro that copies fields into the concrete structure|
| `Model._meta` | `FerrioxidModel::columns` des konkreten Models |
|own table of the base class|there are no|
|Multiple inheritance and MRO|not provided for|

A change in the field group changes `columns()` of each specific model. The schema diff thus sees each affected table. Already applied migration files remain unchanged.

The example is in `examples/notes/tests/components.rs`. myshop and its migrations `0001` to `0005` remain unchanged.
