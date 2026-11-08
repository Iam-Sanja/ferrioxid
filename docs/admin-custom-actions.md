# Custom admin actions

[Overview](admin.md) · Bulk actions: [admin-bulk-actions.md](admin-bulk-actions.md)

## Registration

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

`ferrioxid::admin` exports `AdminAction` and `ActionOperation`. `AdminRegistry::action` attaches the entry to the same registry. There is no second model list.

## Rules

- The name fits `[a-z][a-z0-9_]{0,39}`.
- `delete`, `activate` and `deactivate` are reserved.
- `confirm` must be true.
- `max_ids` is between 1 and 100.
- `Delete` requires the right `delete`.
- `SetBool` requires `update` and a visible, non-write-protected, non-sensitive bool column that is not the primary key.

Built-in actions are `delete` for every model and `activate`/`deactivate` when an `active` column exists. An HTTP action name that is neither built in nor registered through `try_action` returns 422. Requests cannot supply SQL.

Both operations run in the transaction described in [admin-bulk-actions.md](admin-bulk-actions.md) and write `bulk_delete` or `bulk_update` audit entries.

## Custom handler

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

The HTTP parameter selects a registered action by name. Before invoking it, Ferrioxid checks `update` permission, the selection limit, query scope and object policy. `set_text`, `set_integer` and `set_bool` accept only IDs from that selection and columns declared in `writes`. The handler does not open a connection: `update_in` writes in the same transaction as the audit. Any error rolls back all changes. `Delete` and `SetBool` remain available as built-in operations.
