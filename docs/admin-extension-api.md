# Admin extensions

[Overview](admin.md) · Pages: [admin-custom-views.md](admin-custom-views.md) · Actions: [admin-custom-actions.md](admin-custom-actions.md) · Filters: [admin-custom-filters.md](admin-custom-filters.md) · Widgets: [admin-custom-widgets.md](admin-custom-widgets.md)

## Implemented

The public extension API is in `ferrioxid::admin` and uses the same `AdminRegistry` as the models. There is no second admin engine or global registration. The myshop example is in `examples/myshop/src/admin.rs`.

`seal` validates pages, filters, widgets and custom actions against the registered models. If the registry is not sealed, custom-page requests return an error rather than running an unvalidated extension.

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

`OrderReport` and `MarkProcessed` are available in [admin-custom-views.md](admin-custom-views.md) and [admin-custom-actions.md](admin-custom-actions.md). The model key is the table name.

## Limitations

- The namespace is `ferrioxid::admin`, not `ferrioxid::prelude`.
- A custom action requires the model permission `update`. A page uses a complete permission code such as `orders.read`.
- Templates cannot replace `base.html`.
- A widget does not open a hidden or sensitive field and does not replace a foreign key select.
- Page handlers receive the connection only after staff and permission checks. They must not bypass the query scope or object policy. Directory routes continue to check superuser access separately.
- Custom-page navigation appears on the dashboard, model lists, forms and custom pages only when `holds` confirms the required permission.
