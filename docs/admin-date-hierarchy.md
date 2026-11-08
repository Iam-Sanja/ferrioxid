# Date hierarchy

**Milestone:** v0.19.

```rust
#[ferrioxid(admin = true, admin_date_hierarchy = "event_date")]
```

The field must be a visible `NaiveDate` or `DateTime<Utc>`. `NaiveTime` is not allowed.

The list shows year, then month, then day as links. The parameters are `year`, `month` and `day`. A month without a year or a day without a month is 422 and does not write anything.

The filter is a parameterized area:

- `Date`: Includes starting day, excluding the following day, month or year.
- `DateTime`: same limits as UTC midnight.

The group counts a maximum of 100 values and uses the same query scope, the same search and the same filters as the list. A record outside the scope does not appear in the count.

For `DateTime`, the calendar is the UTC tag of the canonical string, not the local zone of the browser.

The navigation is HTML. It does not need JavaScript.
