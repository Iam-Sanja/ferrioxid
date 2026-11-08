# Typisierte Modellbeziehungen

[Zur Übersicht](index.md) · **IST** für Belongs-To und Has-Many. Many-to-Many, Eager Loading und Batch Loading sind nicht implementiert.

Eine Beziehung existiert nicht nur in Rust. Das Feld trägt einen Foreign Key, und die Datenbank prüft ihn. Die Methoden laden typisierte Modelle. Sie geben kein `serde_json::Value` zurück.

## Belongs-To

```rust
#[field(
    foreign_key = "categories.id",
    on_delete = "restrict",
    belongs_to = "super::category::Category"
)]
pub category_id: Option<Uuid>,
```

`belongs_to` ist der Rust-Pfad des Elternmodells. Das Makro kann diesen Typ nicht aus dem Foreign-Key-Text ableiten, weil es andere Module nicht sieht. Der Methodenname ist der Feldname ohne `_id`. `category_id` wird zu `category`.

```rust
let category = product.category(&db).await?;
```

| Feld | Rückgabe |
|---|---|
| `Option<Uuid>` | `Result<Option<Category>, FerrioxidError>` |
| `Uuid` | `Result<Category, FerrioxidError>` |

Ist der optionale Schlüssel `None`, kommt `Ok(None)` und es gibt keine zweite Abfrage. Ist ein Schlüssel gesetzt und die Elternzeile fehlt, kommt `NotFound`. Bei einem wirksamen Foreign Key tritt das nur auf, wenn die Datenbank den Constraint nicht enthält. Ein ungültiger Schlüssel wird schon beim Schreiben abgelehnt.

## Has-Many

Die Gegenrichtung muss explizit sein. Das Makro sieht die Kindmodelle nicht.

```rust
#[ferrioxid(
    table = "categories",
    api = "/api/v1/categories",
    allow_anonymous = true,
    has_many = "products=super::product::Product.category_id"
)]
pub struct Category {
    #[field(primary_key, auto)]
    pub id: Uuid,
    #[field(required, unique, max_length = 100)]
    pub name: String,
}
```

Die Form ist `methode=Typpfad.spalte`. Die Spalte muss auf dem Kindmodell existieren. `find_by_column` lehnt eine unbekannte Spalte ab.

```rust
let products = category.products(&db).await?;
```

Die Rückgabe ist `Result<Vec<Product>, FerrioxidError>`. Keine Kinder ergeben einen leeren Vektor, keinen Fehler. Die Zeilen sind nach `id` aufsteigend sortiert.

Gegenseitige Modulverweise sind zulässig, weil die Typen nur in Methodensignaturen stehen. `myshop` trennt `category.rs` und `product.rs`.

## Laden

Jede Beziehungsmethode führt eine Abfrage aus, wenn sie aufgerufen wird. Das ist Lazy Loading. Eine Schleife über Produkte, die jeweils `product.category(&db)` aufruft, erzeugt eine Abfrage pro Produkt. Das ist das N+1-Problem: eine Abfrage für die Liste und N weitere für die Eltern.

Eager Loading und ein `WHERE id IN (...)` für viele Eltern sind nicht implementiert. Wer viele Kinder lädt, sollte das vorerst mit einer eigenen Abfrage tun, statt in einer Schleife `products()` aufzurufen.

Die Repository-Methoden `list`, `get`, `create`, `update` und `delete` bleiben unverändert. Die Beziehungsmethoden sind eigene Methoden am Modell, keine neuen Repository-Methoden.

## HTTP

Es gibt keine zusätzlichen Routen für Beziehungen. `GET /categories/{id}/products` existiert nicht. Die CRUD-Routen der registrierten Modelle bleiben. Unique- und Foreign-Key-Verletzungen werden zu `FerrioxidError::Validation` und damit zu HTTP 422. Die Antwort enthält den festen englischen Text `unique constraint violated` oder `foreign key constraint violated`, nicht das SQL und nicht die Datenbankmeldung. Eine fehlende Einzelzeile bleibt 404. Eine nicht freigegebene Route bleibt 403. Ein unbekannter Datenbankfehler bleibt 500.

`DELETE` einer Elternzeile mit `on_delete = "restrict"` und vorhandenen Kindern antwortet 422.

## myshop

`Category.name` ist eindeutig. `Product.name` hat einen Index. `Product.category_id` ist optional und verweist mit `restrict` auf `categories.id`. `0001_initial.json` bleibt unverändert. `0002_categories_and_product_relations.json` legt `categories` an, fügt `category_id` samt Foreign Key hinzu und erzeugt `ix_products_name`.

Bestehende Produktzeilen bleiben gültig, weil die neue Spalte nullable ist und keinen Default braucht.

## Nicht implementiert

- Many-to-Many und Join-Tabellen
- Eager Loading, `prefetch` und Batch Loading
- Automatisches Entdecken der Gegenrichtung
- Verschachtelte HTTP-Routen
- Laden einer Beziehung innerhalb von `list`
