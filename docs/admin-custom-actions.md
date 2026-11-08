# Eigene Admin-Aktionen

[Zur Übersicht](admin.md) · Sammelaktionen: [admin-bulk-actions.md](admin-bulk-actions.md)

## Registrierung

```rust
registry.action(AdminAction {
    model_key: "html_counts",
    name: "feature",
    label_de: "Hervorheben",
    label_en: "Feature",
    permission: "update",
    confirm: true,
    max_ids: 100,
    operation: ActionOperation::SetBool { column: "active", value: true },
});
```

`ferrioxid::admin` exportiert `AdminAction` und `ActionOperation`. `AdminRegistry::action` hängt den Eintrag an dieselbe Registry. Es gibt keine zweite Modellliste.

## Regeln

- Der Name passt auf `[a-z][a-z0-9_]{0,39}`.
- `delete`, `activate` und `deactivate` sind reserviert.
- `confirm` muss wahr sein.
- `max_ids` liegt zwischen 1 und 100.
- `Delete` verlangt das Recht `delete`.
- `SetBool` verlangt `update` und eine sichtbare, nicht schreibgeschützte, nicht sensible Bool-Spalte, die nicht der Primärschlüssel ist.

Eingebaute Aktionen: `delete` immer, `activate` und `deactivate` nur wenn eine solche Spalte `active` existiert. Ein HTTP-Name, der weder eingebaut noch über `try_action` registriert ist, ist 422. Es wird kein SQL aus der Anfrage ausgeführt.

Beide Operationen laufen in der Transaktion aus [admin-bulk-actions.md](admin-bulk-actions.md) und schreiben `bulk_delete` oder `bulk_update`.

## Eigener Handler

```rust
struct MarkProcessed;

#[ferrioxid::async_trait::async_trait]
impl ferrioxid::admin::ActionHandler for MarkProcessed {
    async fn apply(
        &self,
        context: &mut ferrioxid::admin::ActionContext,
    ) -> Result<(), ferrioxid::admin::ActionError> {
        for id in context.ids() {
            context.set_text(id, "status", "processed")?;
        }
        Ok(())
    }
}

registry.try_action(
    ferrioxid::admin::AdminAction::custom("orders", "mark_processed", std::sync::Arc::new(MarkProcessed))
        .labels("Als bearbeitet markieren", "Mark processed")
        .writes(&["status"]),
)?;
```

Der HTTP-Parameter wählt nur den registrierten Namen. Ferrioxid prüft vorher `update`, die Auswahlgrenze, den Query-Scope und die Objektregel. `set_text`, `set_integer` und `set_bool` akzeptieren nur eine ID aus dieser Auswahl und eine Spalte aus `writes`. Die Methode öffnet keine Verbindung. Nach dem Plan schreibt `update_in` in derselben Transaktion wie das Audit. Ein Fehler rollt alles zurück. `Delete` und `SetBool` bleiben die bisherigen Literale.
