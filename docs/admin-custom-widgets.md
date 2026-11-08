# Own admin widgets

[For an overview](admin-extension-api.md)]

## IST

`AdminWidget` changes the HTML representation of a visible field. It does not change `AdminFieldMeta`. Hidden, sensitive, primary and foreign keys are rejected by `seal`. A field has at most one widget.

| Art | HTML |Additional testing of the POST|
|---|---|---|
| `text` | `input type="text"` |the model rules;|
| `textarea` |`textarea`, 1 to 4000 characters|Character limit of the widget and the model rules|
| `email` | `input type="email"` | `ferrioxid_core::is_email` |
| `url` | `input type="url"` | `ferrioxid_core::is_http_url` |
| `number` | `input type="number"` |Lower and upper limits, then the model rules|
| `readonly` |Text, not sendable field|a co-transmitted value is `field is not writable`|

```rust
registry
    .widget(ferrioxid::admin::AdminWidget::textarea("orders", "customer", 120))?
    .widget(ferrioxid::admin::AdminWidget::email("people", "email"))?
    .widget(ferrioxid::admin::AdminWidget::number("products", "price_cents", Some(0), Some(1_000_000)))?
    .widget(ferrioxid::admin::AdminWidget::readonly("orders", "status"))?;
```

Tera escaped the value. An error remains on the form and does not write. Drafts, CSRF and the three storage targets remain those of the normal form. Boolean fields and foreign keys retain their built-in controls.
