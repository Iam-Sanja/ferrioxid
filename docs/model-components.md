# Wiederverwendbare Modellfelder

[Zur Übersicht](index.md) · Modelle: [models.md](models.md) · Zeitstempel: [model-timestamps.md](model-timestamps.md)

**IST.** Funktionsstand v0.26. Eine Feldgruppe ist keine Tabelle, keine REST-Route und keine Admin-Registrierung.

Rust kann die Felder eines anderen Typs in einem Derive nicht zuverlässig nachschlagen. Ferrum fügt die Felder deshalb mit einem Makro in die konkrete Struct ein, bevor `FerrumModel` sie sieht. Die gespeicherten Spalten sind flach.

## Eigene Feldgruppe

```rust
use ferrum::prelude::*;

ferrum::model_fields! {
    pub struct AuditStamp {
        #[field(managed = "created")]
        pub created_at: ferrum::chrono::DateTime<ferrum::chrono::Utc>,
        #[field(managed = "updated")]
        pub updated_at: ferrum::chrono::DateTime<ferrum::chrono::Utc>,
    }
}

ferrum_fields_AuditStamp! {
    #[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
    #[ferrum(table = "products", api = "/api/v1/products", allow_anonymous = true)]
    pub struct Product {
        #[field(primary_key, auto)]
        pub id: Uuid,
        #[field(required, min_length = 1)]
        pub name: String,
    }
}
```

`model_fields!` erzeugt die Struct `AuditStamp` ohne Tabelle und das Makro `ferrum_fields_AuditStamp!` in derselben Crate. Das Makro kopiert die Felder in `Product`. Eine zweite Struct kann dasselbe Makro verwenden und erhält dieselben Spalten in einer anderen Tabelle.

Gleiche Feldnamen in der konkreten Struct und in der Gruppe lehnt der Compiler ab. Ein unbekannter Attributwert ebenfalls. Die Gruppe selbst hat keine Migration.

## Django

| Django | Ferrum |
|---|---|
| abstraktes Model | `model_fields!`, ohne Tabelle |
| Vererbung | Makro, das Felder in die konkrete Struct kopiert |
| `Model._meta` | `FerrumModel::columns` des konkreten Models |
| eigene Tabelle der Basisklasse | gibt es nicht |
| Mehrfachvererbung und MRO | nicht vorgesehen |

Eine Änderung der Feldgruppe ändert `columns()` jedes konkreten Models. Der Schema-Diff sieht dadurch jede betroffene Tabelle. Bereits angewendete Migrationsdateien bleiben unverändert.

Das Beispiel liegt in `examples/notes/tests/components.rs`. myshop und seine Migrationen `0001` bis `0005` bleiben unverändert.
