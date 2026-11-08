# Transaktionale Inlines

[For overview](admin-inlines.md) · Transactions: [transactions.md](transactions.md) · Drafts: [admin-form-drafts.md](admin-form-drafts.md)]

**Stand:** v0.20. The cargo version remains `0.1.0`.

`admin_inline` without further options remains the list on the detail page plus the own creation form. The common form is opt-in.

## Syntax

```rust
admin_inline = "lines=order_lines.order_id|form|add|change|delete|label=Positionen|max=20|extra=1|fields=product,quantity,unit_price"
```

The part before `|` remains `name=tabelle.spalte`. Options:

| Option | Bedeutung |
|---|---|
| `form` |Children are in parental form|
| `add`, `change`, `delete` |Create, change, remove. `form` alone bets all three. If one of the three is there, the others start with wrong|
| `label` |1-40 characters, without control signs and without `\|` |
| `fields` |editable child fields. Parent key is prohibited|
| `readonly` |Only fields from `fields` if `fields` is set|
| `min`, `max`, `extra` |Minimum number, maximum number, blank additional lines. `max` is a maximum of 50. `form` without `max` sets 20. `extra` becomes 1 if `add` applies and `extra` is missing|
| `order` | Sortierspalte, sonst `id` |

Unknown options are compile errors. `form` without `add`, `change` or `delete` as well. `examples/myshop` uses this to `Order` and `OrderLine`. The category remains with the old attribute.

## Speichern

`save=rows` only increases the additional lines and writes a draft form. There is no model line. `save=detail`, `save=edit` and `save=new` if at least one inline `form` carries:

1. Check structure: known fields, no duplicate child ID, index under `max`, child belongs to the parent object, minimum and maximum number.
2. Check field values, duty, unique and policies.
3. Start transaction and re-read Actor, records and fingerprint.
4. Parental object writing. Put each child’s parent foreign key on this ID.
5. Create new children, change existing ones, delete ones marked with `remove=1` if `delete` is allowed.
6. Write audit lines with the same `operation_id`.
7. Commit.

One mistake rolls everything back. The answer remains on the form. `409` with `conflict.stale` means that the fingerprint no longer fits the stock. There is no silent overwriting.

The fingerprint is a process local hash of the parent and child fields at the time of render, hidden as `basis`. This is not a version column and not a framework-wide optimistic locking. A restart of the process can hash the same stock differently. Two SQLite writers do not run in parallel because of `max_connections(1)`. The check is the re-reading in the transaction.

## Grenzen

- A has-many level. Not a many-to-many. No nested inlines.
- Further foreign keys of a child are not resolved in this transaction via `ensure_relations`. `OrderLine` has only the parent key.
- An empty Boolean `false` counts as content. An otherwise empty line with ticks is not skipped.
- An unchanged child line is rewritten and audited. A second submission of the same valid form can therefore generate another audit line as long as the fingerprint still fits.
- Remove the checkbox before saving. There is no own recovery action. A deselected check mark before sending leaves the line.
- Child `DateTime` in the inline is not converted into the value for `datetime-local`. Order positions have no such field.
