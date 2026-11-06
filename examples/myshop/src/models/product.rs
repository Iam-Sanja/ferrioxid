use ferrum::prelude::*;

// v0.1: deliberately public for the local demo. Never expose real data this way.
#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 3, max_length = 150)]
    pub name: String,

    #[field(min = 0)]
    pub price_cents: i64,

    #[field(default = true)]
    pub active: bool,
}
