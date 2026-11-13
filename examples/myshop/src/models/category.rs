use fexu::prelude::*;

#[derive(FexuModel, fexu::serde::Serialize, fexu::serde::Deserialize)]
#[fexu(
    table = "categories",
    api = "/api/v1/categories",
    allow_anonymous = true,
    has_many = "products=super::product::Product.category_id",
    admin = true,
    admin_inline = "products=products.category_id",
    admin_label = "Kategorien",
    admin_permission = "categories"
)]
pub struct Category {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, unique, max_length = 100)]
    #[admin(list_display, searchable, sortable, label = "Name")]
    pub name: String,
}
