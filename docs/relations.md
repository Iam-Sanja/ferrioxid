# Typical model relationships

[For overview](index.md) · **IST** for Belongs-To and Has-Many. Many-to-Many, Eager Loading and Batch Loading are not implemented.

A relationship does not only exist in Rust. The field carries a foreign key, and the database checks it. The methods load typed models. You do not return a `serde_json::Value`.

## Belongs-To

```rust
#[field(
    foreign_key = "categories.id",
    on_delete = "restrict",
    belongs_to = "super::category::Category"
)]
pub category_id: Option<Uuid>,
```

`belongs_to` is the Rust path of the parent model. The macro cannot derive this type from the foreign key text because it does not see other modules. The method name is the field name without `_id`. `category_id` becomes `category`.

```rust
let category = product.category(&db).await?;
```

| Feld |Return|
|---|---|
| `Option<Uuid>` | `Result<Option<Category>, FerrioxidError>` |
| `Uuid` | `Result<Category, FerrioxidError>` |

If the optional key is `None`, `Ok(None)` comes and there is no second query. If a key is set and the parent line is missing, `NotFound` comes. With an effective foreign key, this only occurs if the database does not contain the constraint. An invalid key is already rejected when writing.

## Has-Many

The opposite direction must be explicit. The macro does not see the child models.

```rust
#[ferrioxid(
    table = "categories",
    api = "/api/v1/categories",
    allow_anonymous = true,
    has_many = "products=super::product::Product.category_id"
)]
pub struct Category {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(required, unique, max_length = 100)]
    pub name: String,
}
```

The form is `methode=Typpfad.spalte`. The column must exist on the child model. `find_by_column` rejects an unknown column.

```rust
let products = category.products(&db).await?;
```

The return is `Result<Vec<Product>, FerrioxidError>`. No children make an empty vector, no error. The lines are sorted ascending according to `id`.

Mutual module references are allowed because the types are only in method signatures. `myshop` separates `category.rs` and `product.rs`.

## Laden

Each relationship method performs a query when it is called. This is lazy loading. A loop about products that calls `product.category(&db)` each creates one query per product. This is the N+1 problem: one query for the list and N more for the parents.

Eager loading and a `WHERE id IN (...)` for many parents are not implemented. If you invite many children, you should do so with your own query instead of calling `products()` in a loop.

The repository methods `list`, `get`, `create`, `update` and `delete` remain unchanged. The relationship methods are separate methods on the model, not new repository methods.

## HTTP

There are no additional routes for relationships. `GET /categories/{id}/products` does not exist. The CRUD routes of the registered models remain. Unique and foreign key violations become `FerrioxidError::Validation` and thus HTTP 422. The answer contains the fixed English text `unique constraint violated` or `foreign key constraint violated`, not the SQL and not the database message. A missing single line remains 404. An unreleased route remains 403. An unknown database error remains 500.

`DELETE` of a parent line with `on_delete = "restrict"` and existing children answers 422.

## myshop

`Category.name` is unique. `Product.name` has an index. `Product.category_id` is optional and references `restrict` to `categories.id`. `0001_initial.json` remains unchanged. `0002_categories_and_product_relations.json` creates `categories`, adds `category_id` and Foreign Key and creates `ix_products_name`.

Existing product lines remain valid because the new column is nullable and does not need a default.

## Not implemented

- Many-to-many and join tables
- Eager Loading, `prefetch` and Batch Loading
- Automatic detection of the opposite direction
- Nested HTTP routes
- Loading a relationship within `list`
