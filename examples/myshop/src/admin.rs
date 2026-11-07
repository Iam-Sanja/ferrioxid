//! Öffentliche Admin-Erweiterungen für die Bestellpflege.

use ferrum::admin::{
    ActionContext, ActionError, ActionHandler, AdminAction, AdminFilter, AdminRegistry, AdminView,
    AdminWidget, ExtensionError, FilterOption, ViewHandler, ViewPage, ViewRequest,
};
use ferrum::db::ModelRepository;
use myshop::models::order::Order;
use serde_json::{json, Map, Value};
use std::sync::Arc;

const ORDER_STATUS: &[FilterOption] = &[
    FilterOption::new("open", "Offen", "Open"),
    FilterOption::new("processed", "Bearbeitet", "Processed"),
];

struct OrderReport;

#[ferrum::async_trait::async_trait]
impl ViewHandler for OrderReport {
    async fn handle(&self, request: &ViewRequest<'_>) -> Result<ViewPage, ExtensionError> {
        let listed = Order::list(request.db)
            .await
            .map_err(|_| ExtensionError::new("extension.view", "orders could not be read"))?;
        let mut counts: Vec<(String, u64)> = Vec::new();
        for order in &listed.data {
            if let Some((_, count)) = counts
                .iter_mut()
                .find(|(status, _)| status == &order.status)
            {
                *count += 1;
            } else {
                counts.push((order.status.clone(), 1));
            }
        }
        let rows: Vec<Value> = counts
            .into_iter()
            .map(|(label, value)| json!({ "label": label, "value": value }))
            .collect();
        let german = request.locale == "de";
        let mut data = Map::new();
        data.insert(
            "intro".to_owned(),
            json!(if german {
                "Bestellungen nach Status. Die Liste ist auf die erste Ergebnisseite begrenzt."
            } else {
                "Orders by status. The list stops at the first result page."
            }),
        );
        data.insert(
            "empty".to_owned(),
            json!(if german {
                "Keine Bestellungen."
            } else {
                "No orders."
            }),
        );
        data.insert(
            "refresh".to_owned(),
            json!(if german { "Aktualisieren" } else { "Refresh" }),
        );
        data.insert("rows".to_owned(), Value::Array(rows));
        let mut page = ViewPage::new(data);
        if request.form.contains_key("csrf_token") {
            page.notice = if german {
                "Ansicht aktualisiert.".to_owned()
            } else {
                "View refreshed.".to_owned()
            };
        }
        Ok(page)
    }
}

struct MarkProcessed;

#[ferrum::async_trait::async_trait]
impl ActionHandler for MarkProcessed {
    async fn apply(&self, context: &mut ActionContext) -> Result<(), ActionError> {
        for id in context.ids() {
            context.set_text(id, "status", "processed")?;
        }
        Ok(())
    }
}

pub fn extend(registry: AdminRegistry) -> Result<AdminRegistry, ExtensionError> {
    registry
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
            AdminAction::custom("orders", "mark_processed", Arc::new(MarkProcessed))
                .labels("Als bearbeitet markieren", "Mark processed")
                .writes(&["status"]),
        )?
        .filter(
            AdminFilter::choice("orders", "order_status", "status", ORDER_STATUS)
                .labels("Status", "Status"),
        )?
        .widget(AdminWidget::textarea("orders", "customer", 120))?
        .seal()
}
