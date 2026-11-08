use ferrioxid::prelude::*;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "order_lines",
    api = "/api/v1/order_lines",
    admin = true,
    admin_label = "Bestellpositionen",
    admin_permission = "orders"
)]
pub struct OrderLine {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(
        foreign_key = "orders.id",
        on_delete = "restrict",
        belongs_to = "super::order::Order"
    )]
    #[admin(label = "Bestellung")]
    pub order_id: Uuid,

    #[field(required, min_length = 1, max_length = 80)]
    #[admin(list_display, list_link, searchable, label = "Produkt")]
    pub product: String,

    #[field(min = 1)]
    #[admin(list_display, label = "Menge")]
    pub quantity: i64,

    #[field(min = 0)]
    #[admin(list_display, label = "Einzelpreis")]
    pub unit_price: i64,
}
