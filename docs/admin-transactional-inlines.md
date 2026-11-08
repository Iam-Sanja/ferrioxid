# Transactional inlines

[Overview](admin-inlines.md) · Transactions: [transactions.md](transactions.md) · Drafts: [admin-form-drafts.md](admin-form-drafts.md)

**Milestone:** v0.20. The Cargo version remains `0.1.0`.

Without further options, `admin_inline` keeps the detail-page list and separate child creation form. A shared parent-child form is opt-in.

## Syntax

```rust
admin_inline = "lines=order_lines.order_id|form|add|change|delete|label=Positionen|max=20|extra=1|fields=product,quantity,unit_price"
```

The part before `|` remains `name=tabelle.spalte`. Options:

| Option | Meaning |
|---|---|
| `form` | Render children within the parent form |
| `add`, `change`, `delete` | Enable creation, editing or removal. `form` alone enables all three. If any is explicitly listed, unlisted operations default to false. |
| `label` |1-40 characters, without control signs and without `\|` |
| `fields` |editable child fields. Parent key is prohibited|
| `readonly` |Only fields from `fields` if `fields` is set|
| `min`, `max`, `extra` |Minimum number, maximum number, blank additional lines. `max` is a maximum of 50. `form` without `max` sets 20. `extra` becomes 1 if `add` applies and `extra` is missing|
| `order` | Sort column, otherwise `id` |

Unknown options are compiler errors. A form with all of `add`, `change` and `delete` disabled is also invalid. `examples/myshop` uses transactional forms for `Order` and `OrderLine`; categories retain the list-only attribute.

## Saving

`save=rows` adds blank child rows and saves a form draft without writing model rows. When at least one inline uses `form`, `save=detail`, `save=edit` and `save=new` use this workflow:

1. Check structure: known fields, no duplicate child ID, index under `max`, child belongs to the parent object, minimum and maximum number.
2. Check field values, required fields, uniqueness and policies.
3. Begin the transaction and reload the acting user, records and fingerprint.
4. Write the parent and set each child's parent foreign key to its ID.
5. Create new children, change existing ones, delete ones marked with `remove=1` if `delete` is allowed.
6. Write audit lines with the same `operation_id`.
7. Commit.

Any error rolls back all changes and keeps the form open. `409` with `conflict.stale` means the fingerprint no longer matches current data; changes are not silently overwritten.

The fingerprint is a process-local hash of parent and child fields at render time, submitted as hidden `basis`. It is neither a version column nor framework-wide optimistic locking. A process restart may hash the same data differently. SQLite writes are serialized by `max_connections(1)`; the data is rechecked within the transaction.

## Limitations

- A has-many level. Not a many-to-many. No nested inlines.
- Further foreign keys of a child are not resolved in this transaction via `ensure_relations`. `OrderLine` has only the parent key.
- An empty Boolean `false` counts as content. An otherwise empty line with ticks is not skipped.
- An unchanged child line is rewritten and audited. A second submission of the same valid form can therefore generate another audit line as long as the fingerprint still fits.
- Uncheck a removal checkbox before submitting to keep that row. There is no separate undo action.
- Child `DateTime` in the inline is not converted into the value for `datetime-local`. Order positions have no such field.
