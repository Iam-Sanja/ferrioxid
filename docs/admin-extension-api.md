# Admin-Erweiterung

[Zur Übersicht](admin.md) · Seiten: [admin-custom-views.md](admin-custom-views.md) · Aktionen: [admin-custom-actions.md](admin-custom-actions.md) · Filter: [admin-custom-filters.md](admin-custom-filters.md) · Widgets: [admin-custom-widgets.md](admin-custom-widgets.md)

## IST

The public API is in `ferrioxid::admin`. It is attached to the same `AdminRegistry` as the models. There is no second admin engine and no global registration. myshop shows them in `examples/myshop/src/admin.rs`.

`seal` checks pages, filters, widgets and own actions against the already registered models. Without `seal`, ferrioxide answers its own page with an error and does not apply the extension as a success.

## Example

```rust
use ferrioxid::admin::{AdminFilter, AdminRegistry, AdminView, AdminWidget, FilterOption};
use std::sync::Arc;

const STATUS: &[FilterOption] = &[
    FilterOption::new("open", "Offen", "Open"),
    FilterOption::new("processed", "Bearbeitet", "Processed"),
];

let registry = AdminRegistry::new()
    .add::<Order>()
    .template(
        "orders_report.html",
        include_str!("../templates/admin/orders_report.html"),
    )?
    .view(
        AdminView::new("orders")
            .titles("Bestellungen", "Orders")
            .permission("orders.read")
            .navigation(true)
            .template("orders_report.html")
            .on_get(OrderReport)
            .on_post(OrderReport),
    )?
    .try_action(
        ferrioxid::admin::AdminAction::custom(
            "orders",
            "mark_processed",
            Arc::new(MarkProcessed),
        )
        .labels("Als bearbeitet markieren", "Mark processed")
        .writes(&["status"]),
    )?
    .filter(
        AdminFilter::choice("orders", "order_status", "status", STATUS)
            .labels("Status", "Status"),
    )?
    .widget(AdminWidget::textarea("orders", "customer", 120))?
    .seal()?;
```

`OrderReport` and `MarkProcessed` are available in [admin-custom-views.md](admin-custom-views.md) and [admin-custom-actions.md](admin-custom-actions.md)]. The model key is the table name.

## Grenzen

- The namespace is `ferrioxid::admin`, not `ferrioxid::prelude`.
- An own action uses the model verb `update`. A page uses a full code such as `orders.read`.
- Templates cannot replace `base.html`.
- A widget does not open a hidden or sensitive field and does not replace a foreign key select.
- The trader of one side receives the connection only after staff and rights check. He may not reproduce the list. The directory routes continue to check superusers themselves.
- The navigation of the own pages appears on the overview, in model lists, forms and on the own page, only if `holds` confirms the code.
