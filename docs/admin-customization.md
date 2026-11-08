# Admin customization

[Overview](admin.md) · Macro: [macros.md](macros.md)

## Purpose

Customize the admin through Rust model attributes. There is no runtime schema builder or second schema definition.

## Attributes

```rust
#[ferrioxid(
    table = "products",
    api = "/api/v1/products",
    allow_anonymous = true,
    admin = true,
    admin_label = "Produkte",
    admin_permission = "products"
)]
#[admin(list_display, searchable, sortable, filterable, readonly, hidden, label = "Name")]
```

`admin = true` is required before field attributes are allowed. `admin_label` is 1 to 80 characters and otherwise the table name. `admin_permission` is either `authenticated` or an identifier. If it is missing, the existing `permissions` applies. If both are missing, the model is visible in the admin only for superusers.

## Field rules

- `id` appears in the list and is not editable.
- `readonly` and the primary key cannot be written via the admin.
- `hidden` is not compatible with list, search, sorting or filtering.
- `searchable` requires `String`.
- Sensitive field names must be `hidden`.
- An unknown attribute is a compiler error with the list of allowed names.

`allow_anonymous` and `permissions` together remain a compiler error. `allow_anonymous` with `admin` and `admin_permission` is allowed: the public route remains public, the admin requires the mentioned resource and additional staff.

## Registration

```rust
AdminRegistry::new().add::<Product>().add_policy::<InternalNote, NotePolicy>()
```

`add` ignores models without `admin = true`. Bind an object policy with `add_policy`; otherwise, individual record access returns 403 with `object policy is not bound`.

## myshop

`Product` uses `admin_permission = "products"`, `Category` uses `categories`, `InternalNote` takes `notes` from `permissions`. The migrations `0001`, `0002` and `0003` remain unchanged because no column is added.

## Security

`permissions = "authenticated"` together with staff access grants every staff user access to the model. This is explicit and should be limited to internal tools.

## Errors

Unknown attributes, a hidden `id`, a searchable non-text column and visible sensitive fields produce compiler errors. Runtime errors are described in [admin-backend.md](admin-backend.md).

## Tests

The admin HTTP test checks `AdminProduct::admin_meta`, the registry and an unregistered model.

## Limitations

Custom widgets and has-many inline lists are implemented; see [admin-custom-widgets.md](admin-custom-widgets.md) and [admin-inlines.md](admin-inlines.md). Computed columns and many-to-many selectors remain unimplemented. Transactional child forms are opt-in through [admin-transactional-inlines.md](admin-transactional-inlines.md).

## Later

Further work includes computed columns and richer declarative scopes, while retaining the existing SQL scope checks.
