use ferrioxid::prelude::*;
use ferrioxid::FerrioxidChoice;

#[derive(Clone, Copy, Debug, PartialEq, Eq, FerrioxidChoice)]
pub enum ProductStatus {
    #[choice(value = "draft", de = "Entwurf", en = "Draft")]
    Draft,
    #[choice(value = "active", de = "Aktiv", en = "Active")]
    Active,
    #[choice(value = "archived", de = "Archiv", en = "Archived")]
    Archived,
}

// v0.1: deliberately public for the local demo. Never expose real data this way.
#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "products",
    api = "/api/v1/products",
    allow_anonymous = true,
    admin = true,
    admin_label = "Produkte",
    admin_permission = "products"
)]
pub struct Product {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, index, min_length = 3, max_length = 150)]
    #[admin(list_display, searchable, sortable, label = "Name")]
    pub name: String,

    #[field(min = 0)]
    #[admin(list_display, sortable, label = "Preis in Cent")]
    pub price_cents: i64,

    #[field(precision = 12, scale = 2)]
    #[admin(list_display, sortable, filterable, label = "Preis")]
    pub price: Decimal,

    #[field(choice)]
    #[admin(list_display, filterable, sortable, label = "Status")]
    pub status: ProductStatus,

    #[field(default = true)]
    #[admin(list_display, filterable, label = "Aktiv")]
    pub active: bool,

    #[field(
        foreign_key = "categories.id",
        on_delete = "restrict",
        belongs_to = "super::category::Category"
    )]
    #[admin(list_display, filterable, label = "Kategorie")]
    pub category_id: Option<Uuid>,
}
