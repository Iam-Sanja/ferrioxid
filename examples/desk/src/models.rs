//! Desk models. The migration snapshot is built from these definitions.

use fexu::prelude::*;
use fexu::serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, FexuModel, Serialize, Deserialize)]
#[fexu(table = "desk_owners")]
pub struct Owner {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(required, min_length = 1, max_length = 80)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, FexuModel, Serialize, Deserialize)]
#[fexu(table = "desk_categories")]
pub struct Category {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(required, min_length = 1, max_length = 80)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, FexuModel, Serialize, Deserialize)]
#[fexu(table = "desk_products")]
pub struct Product {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(required, min_length = 1, max_length = 150)]
    pub name: String,
    #[field(min = 0)]
    pub stock: i64,
    pub active: bool,
    pub price_cents: i64,
    #[field(foreign_key = "desk_owners.id", on_delete = "restrict")]
    pub owner_id: Uuid,
    #[field(foreign_key = "desk_categories.id", on_delete = "restrict")]
    pub category_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, FexuModel, Serialize, Deserialize)]
#[fexu(table = "desk_carts")]
pub struct Cart {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(unique, foreign_key = "desk_owners.id", on_delete = "restrict")]
    pub owner_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, FexuModel, Serialize, Deserialize)]
#[fexu(table = "desk_lines")]
pub struct Line {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(foreign_key = "desk_carts.id", on_delete = "restrict")]
    pub cart_id: Uuid,
    #[field(foreign_key = "desk_owners.id", on_delete = "restrict")]
    pub owner_id: Uuid,
    #[field(required, min_length = 1, max_length = 40)]
    pub sku: String,
    #[field(min = 0)]
    pub qty: i64,
    pub moved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, FexuModel, Serialize, Deserialize)]
#[fexu(table = "desk_addresses")]
pub struct Address {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(foreign_key = "desk_owners.id", on_delete = "restrict")]
    pub owner_id: Uuid,
    #[field(required, min_length = 1, max_length = 120)]
    pub label: String,
}
