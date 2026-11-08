# Fieldsets

[Overview](admin.md) · Templates: [admin-templates.md](admin-templates.md)

**Milestone:** v0.17. The Cargo version remains `0.1.0`.

Without `admin_fieldset`, the form remains a single unnamed group.

A model declares the sections. Each visible field names exactly one of them.

```rust
#[ferrioxid(
    admin = true,
    admin_fieldset = "Allgemein|Name und Kategorie",
    admin_fieldset = "Preis und Verfügbarkeit",
    admin_fieldset = "System|Nur lesen|optional"
)]
struct Offer {
    #[admin(fieldset = "System")]
    id: Uuid,
    #[admin(fieldset = "Allgemein")]
    title: String,
}
```

A section has a name, an optional description and the optional flag `optional`. Names contain 1–80 characters and must be unique; descriptions contain at most 160. Once a model declares sections, empty sections and unassigned visible fields are compiler errors. Hidden fields need no section, and a field can appear only once.

Named sections render as `fieldset` with `legend`. An `optional` section uses `details` and `summary` without JavaScript. Empty sections, such as an ID-only section during creation, are omitted. Fields follow struct order within each section.

`readonly` fields and `id` remain text. Hidden fields are not submitted. Permissions, CSRF and `reject_locked` apply unchanged.
