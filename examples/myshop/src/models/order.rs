use fexu::prelude::*;
use fexu::FexuChoice;

#[derive(Clone, Copy, Debug, PartialEq, Eq, FexuChoice)]
pub enum OrderStatus {
    #[choice(value = "pending", de = "Offen", en = "Pending")]
    Pending,
    #[choice(value = "paid", de = "Bezahlt", en = "Paid")]
    Paid,
    #[choice(value = "shipped", de = "Versendet", en = "Shipped")]
    Shipped,
    #[choice(value = "cancelled", de = "Storniert", en = "Cancelled")]
    Cancelled,
}

#[derive(FexuModel, fexu::serde::Serialize, fexu::serde::Deserialize)]
#[fexu(
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

    #[field(choice)]
    #[admin(list_display, filterable, sortable, label = "Auftragsstatus")]
    pub state: Option<OrderStatus>,
}
