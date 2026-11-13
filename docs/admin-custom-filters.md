# Custom admin filters

[Overview](admin-extension-api.md)

## Implemented

`AdminFilter::equals` and `AdminFilter::choice` attach a GET field `filter.{name}` to the model list. The name must not be a model field. The column is a `'static` name from the registry and must belong to the model. It must not be the primary key and must not be hidden or sensitive.

An empty value omits the filter. A choice value must correspond to one of the fixed options, otherwise the list is 422. An equals value is bound by the column type. It is not used as SQL text. Search, sorting, date hierarchy, page size and the query scope of the object rule remain additionally active.

```rust
const STATUS: &[fexu::admin::FilterOption] = &[
    fexu::admin::FilterOption::new("open", "Offen", "Open"),
    fexu::admin::FilterOption::new("processed", "Bearbeitet", "Processed"),
];

registry.filter(
    fexu::admin::AdminFilter::choice("orders", "order_status", "status", STATUS)
        .labels("Status", "Status"),
)?;
```

The selection does not need JavaScript. A maximum of 20 options are allowed. A date range as its own filter type is not implemented. The existing date hierarchy remains separate from this.
