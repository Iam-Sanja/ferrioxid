# Zeitstempel

[Zur Übersicht](index.md) · Komponenten: [model-components.md](model-components.md)

`ferrum::with_timestamps!` hängt `created_at` und `updated_at` als `DateTime<Utc>` an ein konkretes Model.

```rust
use ferrum::prelude::*;

ferrum::with_timestamps! {
    #[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
    #[ferrum(table = "categories", api = "/api/v1/categories", allow_anonymous = true)]
    pub struct Category {
        #[field(primary_key, auto)]
        pub id: Uuid,
        #[field(required, min_length = 1)]
        pub title: String,
    }
}
```

Beide Felder tragen `#[field(managed = "created")]` beziehungsweise `"updated"`. Das Repository setzt sie mit `ferrum::core::utc_now()`. `created_at` bleibt bei einem Update unverändert. `updated_at` wird bei jedem erfolgreichen Update gesetzt, auch wenn die übrigen Felder gleich bleiben.

Die Felder fehlen in `CategoryCreate` und `CategoryUpdate`. Ein JSON-Körper mit `created_at` ist `deny_unknown_fields`. Im Admin sind sie readonly, weil `managed` das Admin-Feld schreibgeschützt macht. `reject_locked` lehnt sie im Formular ab.

Die Uhr ist `Utc::now()`, außer ein Test setzt `ferrum::core::set_test_clock` für den aktuellen Thread. Das ist keine Model-Registry.

`HasTimestamps` stellt `created_at()` und `updated_at()` bereit, wenn beide Rollen vorhanden sind.

`ferrum generate model Product --timestamps` umschließt das neue Model mit `with_timestamps!`. Ohne das Flag bleibt der bisherige Rumpf. Vorhandene Dateien werden nicht überschrieben.

JSON bleibt flach: `created_at` und `updated_at` stehen neben `title`.
