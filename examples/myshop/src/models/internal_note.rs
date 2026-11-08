use ferrioxid::prelude::*;

/// Geschützte Ressource. Produkte und Kategorien bleiben ausdrücklich öffentliche Demo-Routen.
#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "internal_notes",
    api = "/api/v1/internal_notes",
    permissions = "notes",
    admin = true,
    admin_label = "Interne Notizen"
)]
pub struct InternalNote {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1, max_length = 200)]
    #[admin(list_display, searchable, sortable, label = "Titel")]
    pub title: String,

    #[admin(list_display, label = "Text")]
    pub body: Option<String>,
}
