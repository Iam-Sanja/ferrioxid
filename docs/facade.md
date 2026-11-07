# ferrum – Öffentliche Framework-Fassade

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum/src/lib.rs`.

## Aufgabe (IST)

Die `ferrum`-Crate ist der öffentliche Einstiegspunkt. Sie exportiert `App`, `FerrumModel` (Derive-Makro) und Wrapper-Namensräume für die benutzten Libraries. Der `prelude`-Block exportiert `App`, `FerrumModel`, `Uuid`.

### Aktuelle Exporte

- `ferrum::App`, `ferrum::pages`, `ferrum::web`
- `ferrum::forms`, `ferrum::static_files`, `ferrum::storage`, `ferrum::middleware`
- `ferrum::core` (`ferrum-core`)
- `ferrum::db` (`ferrum-db`)
- `ferrum::FerrumModel` (Makro)
- `ferrum::sea_orm`, `ferrum::serde`, `ferrum::serde_json`, `ferrum::uuid`, `ferrum::async_trait`
- `ferrum::prelude::{App, FerrumModel, Uuid}`

### Developer-Usage

```rust
use ferrum::prelude::*;
#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)] pub id: Uuid,
    #[field(required)] pub name: String,
}
```

**SOLL:** Langfristig nach Möglichkeit nur diese Fassade als direkte Dependency benötigen; benötigte interne Types gezielt re-exportieren. Keine heimliche semantische Änderung am öffentlichen Makro.

## Kompatibilitätsregeln

1. Das Makro referenziert generierte Pfade wie `::ferrum::db::...`: Änderungen an Re-Exports wirken direkt auf jedes Nutzermodell.
2. Neuer Trait-Bound oder neue Pflichtfelder können Build-breaking sein.
3. Öffentliches DTO-Namensschema `ProductCreate` ist bereits vom Makro erzeugt; nicht ohne Migrationsplan ändern.
4. `prelude` nicht als Sammelstelle sämtlicher interner Implementierungsdetails nutzen.
5. Beispiele und Tests müssen **nur über die öffentliche Fassade** funktionieren.

## Abnahme

Die `myshop`-App kompiliert gegen die öffentlichen Exporte; ein neues Beispielprojekt darf keine privaten Crate-Innereien benötigen. Belege dies durch `cargo check -p myshop`.
