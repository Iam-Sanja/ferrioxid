# Inline-Relationen

[For overview](admin.md) · Relations: [relations.md](relations.md)]

## Deklaration

The attribute must match an existing `has_many`. The child table is explicitly in the attribute so that two models do not coincide with the same column.

```rust
#[ferrioxid(
    has_many = "products=super::product::Product.category_id",
    admin_inline = "products=products.category_id"
)]
```

`AdminInline` is `name`, `child_table` and `column`. At term, the child must be registered, the column must exist and `foreign_table` must be the parent table. Otherwise the section will not appear. There is no second relation engine.

## Anzeige

The detail page loads the children via `child.page` with equality filter on the foreign key and the child's reading scope. The page is at most `min(per_page, 25)` lines. The query parameter is `inline.{name}`. Shown is the first `list_display` field, which is not the foreign key and not `id`. There is no additional query per line for the display name.

A link opens the child record. Editing uses the normal child page. The foreign key can be changed there. Only the creation from the inline enforces the parent key.

## Anlegen

`GET` and `POST /admin/models/{parent}/{id}/inline/{name}/new`.

The parental sentence must be legible. The child checks the Create right and `can_create`. Before `child.create`, the server sets the foreign key to the ID from the URL. Another value sent along is replaced. Further foreign keys run through `ensure_relations`.

Without `|form`, parents and children are not stored in a form. With `|form`, storing is a transaction. Syntax, rollback and limits: [admin-transactional-inlines.md](admin-transactional-inlines.md)].

Example without a common form: `examples/myshop/src/models/category.rs`. Example with form: `examples/myshop/src/models/order.rs`. Tests: `admin_v018_drafts_inlines_bulk_and_audit` and `examples/myshop/tests/orders.rs`.
