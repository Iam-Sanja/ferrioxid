# ferrioxid – Öffentliche Framework-Fassade

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrioxid/src/lib.rs`.

## Aufgabe (IST)

Die `ferrioxid`-Crate ist der öffentliche Einstiegspunkt. Sie exportiert `App`, `FerrioxidModel` (Derive-Makro) und Wrapper-Namensräume für die benutzten Libraries. Der `prelude`-Block exportiert `App`, `FerrioxidModel`, `Uuid`.

### Aktuelle Exporte

- `ferrioxid::App`, `ferrioxid::pages`, `ferrioxid::web`
- `ferrioxid::forms`, `ferrioxid::static_files`, `ferrioxid::storage`, `ferrioxid::middleware`
- `ferrioxid::core` (`ferrioxid-core`)
- `ferrioxid::db` (`ferrioxid-db`)
- `ferrioxid::FerrioxidModel` (Makro)
- `ferrioxid::sea_orm`, `ferrioxid::serde`, `ferrioxid::serde_json`, `ferrioxid::uuid`, `ferrioxid::async_trait`
- `ferrioxid::prelude::{App, FerrioxidModel, Uuid}`

### Developer-Usage

```rust
use ferrioxid::prelude::*;
#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)] pub id: Uuid,
    #[field(required)] pub name: String,
}
```

**SOLL:** Langfristig nach Möglichkeit nur diese Fassade als direkte Dependency benötigen; benötigte interne Types gezielt re-exportieren. Keine heimliche semantische Änderung am öffentlichen Makro.

## Kompatibilitätsregeln

1. Das Makro referenziert generierte Pfade wie `::ferrioxid::db::...`: Änderungen an Re-Exports wirken direkt auf jedes Nutzermodell.
2. Neuer Trait-Bound oder neue Pflichtfelder können Build-breaking sein.
3. Öffentliches DTO-Namensschema `ProductCreate` ist bereits vom Makro erzeugt; nicht ohne Migrationsplan ändern.
4. `prelude` nicht als Sammelstelle sämtlicher interner Implementierungsdetails nutzen.
5. Beispiele und Tests müssen **nur über die öffentliche Fassade** funktionieren.

## Abnahme

Die `myshop`-App kompiliert gegen die öffentlichen Exporte; ein neues Beispielprojekt darf keine privaten Crate-Innereien benötigen. Belege dies durch `cargo check -p myshop`.
