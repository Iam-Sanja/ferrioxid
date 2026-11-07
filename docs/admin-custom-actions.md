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

`ferrum::admin` exportiert `AdminAction` und `ActionOperation`. `AdminRegistry::action` hängt den Eintrag an dieselbe Registry. Es gibt keine zweite Modellliste.

## Regeln

- Der Name passt auf `[a-z][a-z0-9_]{0,39}`.
- `delete`, `activate` und `deactivate` sind reserviert.
- `confirm` muss wahr sein.
- `max_ids` liegt zwischen 1 und 100.
- `Delete` verlangt das Recht `delete`.
- `SetBool` verlangt `update` und eine sichtbare, nicht schreibgeschützte, nicht sensible Bool-Spalte, die nicht der Primärschlüssel ist.

Eingebaute Aktionen: `delete` immer, `activate` und `deactivate` nur wenn eine solche Spalte `active` existiert. Ein HTTP-Name, der nicht in dieser Liste steht, ist 422. Es wird kein Closure und kein SQL aus der Anfrage ausgeführt.

Beide Operationen laufen in der Transaktion aus [admin-bulk-actions.md](admin-bulk-actions.md) und schreiben `bulk_delete` oder `bulk_update`.

Ein freier Rust-Handler je Aktion ist nicht umgesetzt. Wer eine andere Änderung braucht, erweitert `ActionOperation` im Crate und registriert sie weiter über `AdminAction`.
