# Custom admin widgets

[Overview](admin-extension-api.md)

## Implemented

`AdminWidget` changes the HTML representation of a visible field. It does not change `AdminFieldMeta`. Hidden, sensitive, primary and foreign keys are rejected by `seal`. A field has at most one widget.

| Kind | HTML | Additional POST validation |
|---|---|---|
| `text` | `input type="text"` | Model validation rules |
| `textarea` |`textarea`, 1 to 4000 characters|Character limit of the widget and the model rules|
| `email` | `input type="email"` | `ferrioxid_core::is_email` |
| `url` | `input type="url"` | `ferrioxid_core::is_http_url` |
| `number` | `input type="number"` |Lower and upper limits, then the model rules|
| `readonly` | Text rather than an input | A submitted value is rejected with `field is not writable` |

```rust
registry
    .widget(ferrioxid::admin::AdminWidget::textarea("orders", "customer", 120))?
    .widget(ferrioxid::admin::AdminWidget::email("people", "email"))?
    .widget(ferrioxid::admin::AdminWidget::number("products", "price_cents", Some(0), Some(1_000_000)))?
    .widget(ferrioxid::admin::AdminWidget::readonly("orders", "status"))?;
```

Tera escapes the value. Validation errors keep the form open and do not write data. Drafts, CSRF and the three save destinations behave as in standard forms. Boolean and foreign-key fields retain their built-in controls.
