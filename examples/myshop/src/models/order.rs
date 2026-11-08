use ferrioxid::prelude::*;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "orders",
    api = "/api/v1/orders",
    admin = true,
    admin_label = "Bestellungen",
    admin_permission = "orders",
    admin_ordering = "ordered_on",
    admin_date_hierarchy = "ordered_on",
    has_many = "lines=super::order_line::OrderLine.order_id",
    admin_inline = "lines=order_lines.order_id|form|add|change|delete|label=Positionen|max=20|extra=1|fields=product,quantity,unit_price"
)]
pub struct Order {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1, max_length = 120)]
    #[admin(list_display, list_link, searchable, label = "Kunde")]
    pub customer: String,

    #[field(required)]
    #[admin(list_display, sortable, filterable, label = "Datum")]
    pub ordered_on: NaiveDate,

    #[field(required, min_length = 1, max_length = 40)]
    #[admin(list_display, label = "Status")]
    pub status: String,
}
