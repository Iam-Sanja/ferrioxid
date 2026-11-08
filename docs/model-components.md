# Reusable model fields

[Overview](index.md) · Models: [models.md](models.md) · Timestamps: [model-timestamps.md](model-timestamps.md)

**Implemented.** Milestone v0.26. A field group creates no table, REST route or admin registration.

A derive macro cannot reliably inspect another type’s fields. Ferrioxid uses a separate macro to insert fields into a concrete struct before `FerrioxidModel` processes it. The resulting database columns are flat.

## Custom field group

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

`model_fields!` generates an `AuditStamp` struct without a table and a `ferrioxid_fields_AuditStamp!` macro in the same crate. The macro copies fields into `Product`; other structs can reuse it to add the same columns to their own tables.

The same field names in the concrete structure and in the group are rejected by the compiler. An unknown attribute value as well. The group itself has no migration.

## Django

| Django | Ferrioxid |
|---|---|
| Abstract model | `model_fields!`, without a table |
| Inheritance | A macro copies fields into the concrete struct |
| `Model._meta` | `FerrioxidModel::columns` on the concrete model |
| Separate base-class table | None |
| Multiple inheritance and MRO | Not supported |

A change in the field group changes `columns()` of each specific model. The schema diff thus sees each affected table. Already applied migration files remain unchanged.

The example is in `examples/notes/tests/components.rs`. myshop and its migrations `0001` to `0005` remain unchanged.
