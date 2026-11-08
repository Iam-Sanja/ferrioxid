use ferrioxid::prelude::*;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "events",
    api = "/api/v1/events",
    admin = true,
    admin_label = "Ereignisse",
    admin_permission = "events",
    admin_ordering = "event_date",
    admin_date_hierarchy = "event_date"
)]
pub struct Event {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1, max_length = 150)]
    #[admin(list_display, list_link, searchable, sortable, label = "Titel")]
    pub title: String,

    #[field(required)]
    #[admin(list_display, sortable, filterable, label = "Datum")]
    pub event_date: NaiveDate,

    #[admin(list_display, label = "Beginn")]
    pub start_time: Option<NaiveTime>,

    #[field(required)]
    #[admin(list_display, sortable, label = "Angelegt")]
    pub created_at: DateTime<Utc>,
}
