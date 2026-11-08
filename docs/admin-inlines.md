# Inline relations

[Overview](admin.md) · Relations: [relations.md](relations.md)

## Declaration

The attribute must match an existing `has_many` relation. It explicitly names the child table so models with identically named columns cannot be confused.

```rust
#[ferrioxid(
    has_many = "products=super::product::Product.category_id",
    admin_inline = "products=products.category_id"
)]
```

`AdminInline` contains `name`, `child_table` and `column`. At runtime, the child must be registered, the column must exist, and `foreign_table` must identify the parent table. Otherwise the section is omitted. The admin reuses the existing relation metadata.

## Display

The detail page loads children through `child.page`, filtering by the parent foreign key and the child's read scope. Each page contains at most `min(per_page, 25)` rows; its query parameter is `inline.{name}`. The label is the first `list_display` field other than the foreign key or `id`. Labels do not require an extra query per row.

A link opens the child record. Editing uses the normal child page. The foreign key can be changed there. Only the creation from the inline enforces the parent key.

## Creation

`GET` and `POST /admin/models/{parent}/{id}/inline/{name}/new`.

The parent record must be readable. Creating a child requires create permission and `can_create`. Before `child.create`, the server sets the foreign key from the parent ID in the URL, replacing any submitted value. Other foreign keys are checked by `ensure_relations`.

Without `|form`, parent and child records use separate forms. With `|form`, they save in one transaction. See [admin-transactional-inlines.md](admin-transactional-inlines.md) for syntax, rollback behavior and limits.

Example without a common form: `examples/myshop/src/models/category.rs`. Example with form: `examples/myshop/src/models/order.rs`. Tests: `admin_v018_drafts_inlines_bulk_and_audit` and `examples/myshop/tests/orders.rs`.
