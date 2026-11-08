# Admin anpassen

[For overview](admin.md) · Macro: [macros.md](macros.md)]

## Zweck

The adjustment takes place on the Rust model. There is no runtime toolkit and no second scheme.

## Attribute

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

## Feldregeln

- `id` appears in the list and is not editable.
- `readonly` and the primary key cannot be written via the admin.
- `hidden` is not compatible with list, search, sorting or filtering.
- `searchable` verlangt `String`.
- Sensitive field names must be `hidden`.
- An unknown attribute is a compiler error with the list of allowed names.

`allow_anonymous` and `permissions` together remain a compiler error. `allow_anonymous` with `admin` and `admin_permission` is allowed: the public route remains public, the admin requires the mentioned resource and additional staff.

## Registrierung

```rust
AdminRegistry::new().add::<Product>().add_policy::<InternalNote, NotePolicy>()
```

`add` ignores models without `admin = true`. An object rule must be bound with `add_policy`. Without binding, the single access 403 responds with `object policy is not bound`.

## myshop

`Product` uses `admin_permission = "products"`, `Category` uses `categories`, `InternalNote` takes `notes` from `permissions`. The migrations `0001`, `0002` and `0003` remain unchanged because no column is added.

## Sicherheit

`permissions = "authenticated"` together with Staff allows each Staff user the model. This is explicit and should only be used for internal tools.

## Fehler

Compiler errors for unknown attributes, hidden `id`, searchable non-text column and visible sensitive fields. Runtime errors are present in [admin-baking.md](admin-backend.md)].

## Tests

The admin HTTP test checks `AdminProduct::admin_meta`, the registry and an unregistered model.

## Grenzen

No own widget names, no calculated columns, no multiple relationships in the selection. Has-Many remains a Rust relationship and is not shown as a subtable in the form.

## Later

Declared widget names and an SQL scope as its own attribute as soon as the query can safely do so.
