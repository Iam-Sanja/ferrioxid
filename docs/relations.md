# Typed model relationships

[Overview](index.md) · **Implemented** for Belongs-To, Has-Many and explicit batch loading. Many-to-many and automatic eager loading are not implemented.

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
let visible = product.category_scoped(&db, &scope).await?;
```

`category` uses `QueryScope::All`. `category_scoped` applies the object-policy scope. A null foreign key returns `Ok(None)` and does not query. A set key whose parent is missing or outside the scope returns `NotFound`. The two cases are not distinguished. Both methods accept `&impl ConnectionTrait`, including an open transaction.

| Field |Return|
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

The attribute syntax is `methode=Typpfad.spalte` (method name, type path and column). The column must exist on the child model; `find_by_column` rejects unknown columns.

```rust
let products = category.products(&db).await?;
let visible = category.products_scoped(&db, &scope).await?;
```

The return is `Result<Vec<Product>, FerrioxidError>`. No children make an empty vector, no error. One statement orders the lines by `id`. `products` uses `QueryScope::All`. `products_scoped` applies the scope and calls `load_children`. Neither method caps the number of child rows. Page with `Product::query()` when a parent can own a large set.

Mutual module references are allowed because the types are only in method signatures. `myshop` separates `category.rs` and `product.rs`.

## Loading

`product.category(&db)` and `category.products(&db)` each run one query. A loop that calls the lazy method is still N+1. The batch functions are the supported way to avoid that.

```rust
use ferrioxid::db::{col, load_by_ids, load_children, ModelQuery};
use ferrioxid::uuid::Uuid;

// 5. Twenty cart lines load their products in one statement, not twenty.
let lines = CartLine::query()
    .filter(col("cart_id").eq(cart_id))
    .all(&db)
    .await?;
let product_ids: Vec<Uuid> = lines.iter().map(|line| line.product_id).collect();
let products = load_by_ids::<Product>(&db, &product_ids, &scope).await?;

let category_ids: Vec<Uuid> = products.iter().filter_map(|product| product.category_id).collect();
let categories = load_by_ids::<Category>(&db, &category_ids, &scope).await?;

let lines_for_carts = load_children::<CartLine>(&db, "cart_id", &cart_ids, &scope).await?;
```

`scope` is the `QueryScope` from the object policy. `QueryScope::None` returns no rows and does not query. `QueryScope::Equals` is part of the statement. An identifier that is missing or outside the scope is absent from the vector. The caller cannot tell those two cases apart.

`load_by_ids` and `load_children` accept `&impl ConnectionTrait`. They send at most 100 identifiers per statement. Twenty identifiers are one statement. 101 identifiers are two. `load_by_ids` orders by primary key across those statements. `load_children` orders by `id` inside one statement. A has-many result that spans more than one statement is concatenated in parent-id order, not resorted into one global `id` sequence. `load_children` rejects a column that is not a UUID column of the child model.

The returned child set is not limited to 100 rows. A parent with a very large collection should be read with `CartLine::query().filter(...).page(&db)` instead.

Calling `product.category(&db)` inside a loop does not become a batch. There is no many-to-many loader and no join API.

The repository methods `list`, `get`, `create`, `update` and `delete` remain unchanged. The relationship methods are separate methods on the model, not new repository methods.

## HTTP

There are no additional routes for relationships. `GET /categories/{id}/products` does not exist. The CRUD routes of the registered models remain. Unique and foreign key violations become `FerrioxidError::Validation` and thus HTTP 422. The answer contains the fixed English text `unique constraint violated` or `foreign key constraint violated`, not the SQL and not the database message. A missing single line remains 404. An unreleased route remains 403. An unknown database error remains 500.

`DELETE` of a parent line with `on_delete = "restrict"` and existing children answers 422.

## myshop

`Category.name` is unique. `Product.name` has an index. `Product.category_id` is optional and references `restrict` to `categories.id`. `0001_initial.json` remains unchanged. `0002_categories_and_product_relations.json` creates `categories`, adds `category_id` and Foreign Key and creates `ix_products_name`.

Existing product lines remain valid because the new column is nullable and does not need a default.

## Not implemented

- Many-to-many and join tables
- Automatic eager loading when a lazy relation method is called in a loop
- Automatic detection of the opposite direction
- Nested HTTP routes
- Loading a relationship within `list`
