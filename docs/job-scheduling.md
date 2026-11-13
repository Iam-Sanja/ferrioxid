# Job scheduling

[Overview](index.md) · Jobs: [jobs.md](jobs.md)

**Implemented** for UTC, a fixed offset, and `europe/berlin`. Other IANA zones are rejected. Fexu does not load the IANA database and does not silently fall back to UTC.

## Expressions

`When::cron(schedule_key, expression, zone)` accepts five fields: minute, hour, day of month, month, day of week. Names such as `JAN` or `MON` are rejected. `*` and ranges such as `1-5` and lists such as `0,15,30` are accepted. When both the day of month and the day of week are restricted, a date matches if either field matches.

`next_after` returns a Unix time strictly after the given instant. A gap in local time is skipped. A fold fires once, at the earlier UTC instant. Missed slots are not backfilled, the same rule as intervals. An interval and a cron expression cannot be set together.

Zones:

| Zone | Meaning |
|---|---|
| `utc` | No offset |
| `fixed+HH:MM` or `fixed-HH:MM` | Constant offset |
| `europe/berlin` | CET (`+01:00`) and CEST (`+02:00`). DST starts on the last Sunday of March at 01:00 UTC and ends on the last Sunday of October at 01:00 UTC |

`America/New_York` returns `job.schedule`.

## Idempotency

A cron row stores `cron_expression`, `cron_zone` and `schedule_key`. Caller keys are at most 80 characters. The generated idempotency key `recur:{schedule_key}:{next_at}` may be longer and is accepted up to 120 characters. Success clears the current `schedule_key` and inserts the next row with that key. A crash after the side effect and before the successor insert can run the handler again. Parallel workers claim a row with a conditional update. That is at-least-once, not exactly-once. A lost lease can be claimed again. The previous token cannot finish the job.

## Example

```rust
let when = fexu::jobs::When::cron("notes-morning", "0 9 * * 1-5", "europe/berlin")?;
queue.enqueue::<NotePulseHandler>(&pulse, when).await?;
```

`examples/notes` checks that this expression is accepted and that a process-local cache does not share entries with a second cache.
