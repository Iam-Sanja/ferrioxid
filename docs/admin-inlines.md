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

`AdminInline` contains `name`, `child_table` and `column`. At runtime, the child must be registered, including a child from another installed app. The column must exist, and its `foreign_table` must be the parent table or the parent model key. Otherwise the section is omitted. The admin reuses the existing relation metadata.

## Display

The detail page loads children through `child.page`, filtering by the parent foreign key and the child's read scope. Each page contains at most `min(per_page, 25)` rows; its query parameter is `inline.{name}`. The label is the first `list_display` field other than the parent foreign key or `id`. If that field is itself a foreign key, the names of the visible targets are loaded in one query for the page. A permitted target links to its own admin record. A missing or unreadable target uses the same redacted label and does not link to that target.

The row action opens the child record. Editing uses the normal child page. The foreign key can be changed there. Only the creation from the inline enforces the parent key.

## Creation

`GET` and `POST /admin/models/{parent}/{id}/inline/{name}/new`.

The parent record must be readable. Creating a child requires create permission and `can_create`. Before `child.create`, the server sets the foreign key from the parent ID in the URL, replacing any submitted value. Other foreign keys are checked by `ensure_relations`.

Without `|form`, parent and child records use separate forms. With `|form`, they save in one transaction. See [admin-transactional-inlines.md](admin-transactional-inlines.md) for syntax, rollback behavior and limits.

Example without a common form: `examples/myshop/src/models/category.rs`. Example with form: `examples/myshop/src/models/order.rs`. Tests: `admin_v018_drafts_inlines_bulk_and_audit` and `examples/myshop/tests/orders.rs`.
