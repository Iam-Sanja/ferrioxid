# Formularabschnitte

[For overview](admin.md) · Templates: [admin-templates.md](admin-templates.md)]

**Stand:** v0.17. The cargo version remains `0.1.0`.

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

The three parts are name, optional description and the word `optional`. The name has 1 to 80 characters, the description a maximum of 160. Names are clear. An empty section and a field without a section are translation errors once the model has sections. Hidden fields do not need a section. A field may occur only once.

The template draws a named section as `fieldset` with `legend`. `optional` is `details` and `summary` and remains without JavaScript. Empty sections, for example the ID when creating, are not drawn. The field order is the Struct order within the section.

`readonly` and `id` remain text. `hidden` is not sent. Rights, CSRF and `reject_locked` apply unchanged.
