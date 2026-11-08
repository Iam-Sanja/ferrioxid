# Modelle

[Zur Übersicht](index.md) · Komponenten: [model-components.md](model-components.md) · Zeitstempel: [model-timestamps.md](model-timestamps.md) · **IST:** `FerrioxidModel` in `crates/ferrioxid-macros`.

Ein Modell ist eine Rust-Struct mit `#[derive(FerrioxidModel)]`. Dieselbe Struct liefert Spalten, Repository, JSON, REST und Admin. Es gibt kein zweites Model-System.

```rust
use ferrioxid::prelude::*;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "products",
    api = "/api/v1/products",
    allow_anonymous = true
)]
pub struct Product {
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1, format = "slug")]
    pub name: String,
}
```

`id` bleibt `Uuid` mit `#[field(primary_key, auto)]`. Unterstützte Feldtypen sind `Uuid`, `String`, `i64`, `bool`, `NaiveDate`, `NaiveTime`, `DateTime<Utc>` und `Option` davon, außer bei `id`.

`#[field(format = "slug")]` und `#[field(format = "email")]` prüfen den Wert serverseitig beim Anlegen und beim PATCH. Ein unbekannter `format`- oder `managed`-Wert ist ein Compile-Fehler. `#[field(min = 1)]` bleibt die Regel für positive Zahlen. `#[field(required)]` lehnt leere Strings ab.

JSON ist flach. Ein wiederverwendetes Feld heißt im JSON so wie die Spalte, nicht wie die Feldgruppe.
