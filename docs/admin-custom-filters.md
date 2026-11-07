# Eigene Admin-Filter

[Zur Übersicht](admin-extension-api.md)

## IST

`AdminFilter::equals` und `AdminFilter::choice` hängen ein GET-Feld `filter.{name}` an die Modellliste. Der Name darf kein Modellfeld sein. Die Spalte ist ein `'static`-Name aus der Registrierung und muss zum Modell gehören. Sie darf nicht der Primärschlüssel und nicht verborgen oder sensibel sein.

Ein leerer Wert lässt den Filter weg. Ein Choice-Wert muss einer der festen Optionen entsprechen, sonst ist die Liste 422. Ein Equals-Wert wird nach dem Spaltentyp gebunden. Er wird nicht als SQL-Text eingesetzt. Suche, Sortierung, Datumshierarchie, Seitengröße und der Query-Scope der Objektregel bleiben zusätzlich aktiv.

```rust
const STATUS: &[ferrum::admin::FilterOption] = &[
    ferrum::admin::FilterOption::new("open", "Offen", "Open"),
    ferrum::admin::FilterOption::new("processed", "Bearbeitet", "Processed"),
];

registry.filter(
    ferrum::admin::AdminFilter::choice("orders", "order_status", "status", STATUS)
        .labels("Status", "Status"),
)?;
```

Die Auswahl braucht kein JavaScript. Höchstens 20 Optionen sind erlaubt. Ein Datumsbereich als eigener Filtertyp ist nicht umgesetzt. Die vorhandene Datumshierarchie bleibt davon getrennt.
