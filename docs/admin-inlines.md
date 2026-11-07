# Inline-Relationen

[Zur Übersicht](admin.md) · Relationen: [relations.md](relations.md)

## Deklaration

Das Attribut muss zu einem vorhandenen `has_many` passen. Die Kindtabelle steht ausdrücklich im Attribut, damit zwei Modelle mit derselben Spalte nicht zusammenfallen.

```rust
#[ferrum(
    has_many = "products=super::product::Product.category_id",
    admin_inline = "products=products.category_id"
)]
```

`AdminInline` trägt `name`, `child_table` und `column`. Zur Laufzeit muss das Kind registriert sein, die Spalte existieren und `foreign_table` die Elterntabelle sein. Sonst erscheint der Abschnitt nicht. Es gibt keine zweite Relations-Engine.

## Anzeige

Die Detailseite lädt die Kinder über `child.page` mit Gleichheitsfilter auf den Fremdschlüssel und dem Lese-Scope des Kindes. Die Seite ist höchstens `min(per_page, 25)` Zeilen. Der Query-Parameter ist `inline.{name}`. Angezeigt wird das erste `list_display`-Feld, das nicht der Fremdschlüssel und nicht `id` ist. Es gibt keine zusätzliche Abfrage je Zeile für den Anzeigenamen.

Ein Link öffnet den Kinddatensatz. Bearbeiten nutzt die normale Kindseite. Dort kann der Fremdschlüssel geändert werden. Nur das Anlegen aus dem Inline erzwingt den Elternschlüssel.

## Anlegen

`GET` und `POST /admin/models/{parent}/{id}/inline/{name}/new`.

Der Elternsatz muss lesbar sein. Das Kind prüft das Create-Recht und `can_create`. Vor `child.create` setzt der Server den Fremdschlüssel auf die ID aus der URL. Ein mitgesendeter anderer Wert wird ersetzt. Weitere Fremdschlüssel laufen durch `ensure_relations`.

Eltern und Kinder werden nicht in einem Formular gespeichert. Eine solche Transaktion ist nicht umgesetzt.

Beispiel: `examples/myshop/src/models/category.rs`. Test: `admin_v018_drafts_inlines_bulk_and_audit`.
