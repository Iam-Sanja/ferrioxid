# Date and time

**Milestone:** v0.19. The Cargo version remains `0.1.0`.

Fexu distinguishes three types. They are not converted into each other.

| Rust | `ColumnKind` | Meaning | Canonical string |
|---|---|---|---|
| `chrono::NaiveDate` | `Date` |Calendar day without time and zone| `2026-11-07` |
| `chrono::NaiveTime` | `Time` |Time of day without date| `14:30:00` |
| `chrono::DateTime<Utc>` | `DateTime` | Absolute instant | `2026-11-07T13:30:00Z` |

A non-zero fraction is part of the canonical form, for example `2026-11-07T13:30:00.123456Z`. Whole seconds stay without a decimal.

`Option<T>` makes the column nullable. `NaiveDate` represents a calendar date without timezone conversion. The macro accepts only `DateTime<Utc>` for timestamp fields; other timezone parameters are rejected.

## JSON

```json
{
  "event_date": "2026-11-07",
  "start_time": "14:30:00",
  "created_at": "2026-11-07T13:30:00Z"
}
```

A UTC `+00:00` offset is accepted on input and serialized as `Z`. Other offsets produce `validation.datetime_offset`. Invalid dates such as `2023-02-29` produce `validation.date`; invalid times produce `validation.time`; invalid timestamps produce `validation.datetime`.

## HTML

| Type | Input |
|---|---|
| Date | `<input type="date">` |
| Time | `<input type="time">` |
| DateTime | `<input type="datetime-local">` |

`datetime-local` does not contain a zone. Fexu treats the value as UTC and says so in the Note. `2026-11-07T13:30` becomes `2026-11-07T13:30:00Z`. There is no conversion to the time zone of the browser and therefore no summer time gap.

## Model

```rust
#[fexu(table = "events", api = "/api/v1/events", admin = true, admin_date_hierarchy = "event_date")]
pub struct Event {
    pub event_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub created_at: DateTime<Utc>,
}
```

The types are exported through `fexu::prelude`. Existing models do not need to adopt them.

## Limitations

- No duration and no `DateTime<Local>`.
- No silently changing a column of text to a date.
- PostgreSQL stores `date`, `time` and `timestamptz`. Filters and fixtures bind those native types. SQLite stores the same logical values as text. `DateTime` remains UTC: `Z` and `+00:00` are accepted, and every other offset is rejected.
- A local PostgreSQL run of the new filter and fixture tests is recorded in [postgres-portability-v2.md](postgres-portability-v2.md). An ignored test that did not execute is not a pass.
