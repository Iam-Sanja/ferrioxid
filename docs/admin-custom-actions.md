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

`ferrioxid::admin` exports `AdminAction` and `ActionOperation`. `AdminRegistry::try_action` attaches the entry to the same registry. There is no second model list.

An action is not a model attribute and not a column. `#[field]` and `#[ferrioxid]` describe stored data. `try_action` registers a named operation that runs against a selection of existing rows. The HTTP form posts `action=mark_shipped`; it does not add a `mark_shipped` field to the model.

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

`mark_shipped` is registered on the order model. The framework checks the session CSRF token before this handler runs, then the action permission and the object policy of each selected row. `set_text` accepts only ids from that selection and columns listed in `writes`. The audit row is written in the same transaction as the update. A handler error rolls the status change and the audit back.

```rust
struct MarkShipped;

#[ferrioxid::async_trait::async_trait]
impl ferrioxid::admin::ActionHandler for MarkShipped {
    async fn apply(
        &self,
        context: &mut ferrioxid::admin::ActionContext,
    ) -> Result<(), ferrioxid::admin::ActionError> {
        for id in context.ids() {
            context.set_text(id, "status", "shipped")?;
        }
        Ok(())
    }
}

registry.try_action(
    ferrioxid::admin::AdminAction::custom("orders", "mark_shipped", std::sync::Arc::new(MarkShipped))
        .labels("Als versendet markieren", "Mark shipped")
        .writes(&["status"]),
)?;
```

`examples/myshop/src/admin.rs` registers `mark_processed` in the same way. The HTTP parameter selects a registered action by name. Before invoking it, Ferrioxid checks `update` permission, the selection limit, query scope and object policy. `set_text`, `set_integer` and `set_bool` accept only IDs from that selection and columns declared in `writes`. The handler does not open a connection: `update_in` writes in the same transaction as the audit. Any error rolls back all changes. `Delete` and `SetBool` remain available as built-in operations.
