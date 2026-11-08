# Date and time

**Stand:** v0.19. The cargo version remains `0.1.0`.

Ferrioxide distinguishes three types. They are not converted into each other.

| Rust | `ColumnKind` | Bedeutung |Canon character string|
|---|---|---|---|
| `chrono::NaiveDate` | `Date` |Calendar day without time and zone| `2026-11-07` |
| `chrono::NaiveTime` | `Time` |Time of day without date| `14:30:00` |
| `chrono::DateTime<Utc>` | `DateTime` | Absoluter Zeitpunkt | `2026-11-07T13:30:00Z` |

`Option<T>` remains nullable. A `NaiveDate` does not jump to the neighbor’s day. `DateTime` must be `DateTime<Utc>`. Other time zone parameters reject the macro.

## JSON

```json
{
  "event_date": "2026-11-07",
  "start_time": "14:30:00",
  "created_at": "2026-11-07T13:30:00Z"
}
```

`+00:00` is allowed for reading and is output as `Z`. Every other offset is `validation.datetime_offset`. An invalid date, including `2023-02-29`, is `validation.date`. An invalid time is `validation.time`. An unreadable time is `validation.datetime`.

## HTML

| Typ | Eingabe |
|---|---|
| Date | `<input type="date">` |
| Time | `<input type="time">` |
| DateTime | `<input type="datetime-local">` |

`datetime-local` does not contain a zone. Ferrioxide treats the value as UTC and says so in the Note. `2026-11-07T13:30` becomes `2026-11-07T13:30:00Z`. There is no conversion to the time zone of the browser and therefore no summer time gap.

## Modell

```rust
#[ferrioxid(table = "events", api = "/api/v1/events", admin = true, admin_date_hierarchy = "event_date")]
pub struct Event {
    pub event_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub created_at: DateTime<Utc>,
}
```

The types are in `ferrioxid::prelude`. Existing models do not have to be used.

## Grenzen

- No duration and no `DateTime<Local>`.
- No silently changing a column of text to a date.
- PostgreSQL column types are implemented. A run against a PostgreSQL server is not executed in this version.
