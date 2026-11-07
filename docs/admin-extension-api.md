# Admin-Erweiterung

[Zur Übersicht](admin.md) · Seiten: [admin-custom-views.md](admin-custom-views.md) · Aktionen: [admin-custom-actions.md](admin-custom-actions.md) · Filter: [admin-custom-filters.md](admin-custom-filters.md) · Widgets: [admin-custom-widgets.md](admin-custom-widgets.md)

## IST

Die öffentliche API liegt in `ferrum::admin`. Sie hängt an derselben `AdminRegistry` wie die Modelle. Es gibt keine zweite Admin-Engine und keine globale Registrierung. myshop zeigt sie in `examples/myshop/src/admin.rs`.

`seal` prüft Seiten, Filter, Widgets und eigene Aktionen gegen die schon registrierten Modelle. Ohne `seal` beantwortet Ferrum eine eigene Seite mit einem Fehler und wendet die Erweiterung nicht als Erfolg an.

## Beispiel

```rust
use ferrum::admin::{AdminFilter, AdminRegistry, AdminView, AdminWidget, FilterOption};
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
        ferrum::admin::AdminAction::custom(
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

`OrderReport` und `MarkProcessed` stehen in [admin-custom-views.md](admin-custom-views.md) und [admin-custom-actions.md](admin-custom-actions.md). Der Modellschlüssel ist der Tabellenname.

## Grenzen

- Der Namespace ist `ferrum::admin`, nicht `ferrum::prelude`.
- Eine eigene Aktion nutzt das Modellverb `update`. Eine Seite nutzt einen vollständigen Code wie `orders.read`.
- Vorlagen können `base.html` nicht ersetzen.
- Ein Widget öffnet kein verstecktes oder sensibles Feld und ersetzt kein Fremdschlüssel-Select.
- Der Handler einer Seite erhält die Verbindung erst nach Staff- und Rechteprüfung. Er darf das Verzeichnis nicht nachbauen. Die Verzeichnisrouten prüfen Superuser weiterhin selbst.
- Die Navigation der eigenen Seiten erscheint auf der Übersicht, in Modelllisten, Formularen und auf der eigenen Seite, nur wenn `holds` den Code bestätigt.
