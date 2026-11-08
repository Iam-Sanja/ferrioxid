# Job retries

[Overview](index.md) · Jobs: [jobs.md](jobs.md) · Worker: [workers.md](workers.md)

**Implemented.** Attempts are limited. An unlimited repetition is rejected.

## Error categories

`JobError::retry("code")` sets `retry` while `attempts < max_attempts`; exhausted attempts produce `failed` with `job.attempts`. `JobError::permanent("code")` fails immediately. Handler timeouts are retryable and use `mail.timeout` or `job.timeout`.

Stored codes contain only lowercase letters, digits, periods and underscores, up to 64 characters. Invalid codes become `job.error`. Codes contain no payload.

## Backoff

The waiting time in seconds is `min(300, 2^(attempt-1))`. The first failed attempt waits one second, the second two, then four, until 300 seconds. `jobs.jitter = true` adds a portion from the UTC second. Tests set `JobPolicy.jitter` to `false` and advance `ManualClock`. There is no real waiting time in the test.

`next_attempt_at` and `available_at` must both be reached before the job is claimed again.

## Operator

```powershell
ferrioxid jobs list
ferrioxid jobs inspect <job-id>
ferrioxid jobs retry <job-id> --yes
ferrioxid jobs cancel <job-id> --yes
```

`list` and `inspect` open SQLite with `mode=ro` and do not write. `retry` applies to `failed` and `cancelled`. It sets `pending`, `attempts = 0` and deletes lease and error code. `cancel` applies to `pending` and `retry`. A `running` job remains untouched and delivers `job.running`.

Without `--yes`, the command exits with code 2 and leaves the database unchanged. Authorization relies on the local operator’s access, the same trust boundary as `ferrioxid migrate up`. There is no HTTP endpoint.

## Intervals

Only successful completion schedules the next interval. A permanently failed recurring job creates no further rows. `ferrioxid jobs retry --yes` reopens the same row. Enqueuing twice with the same `schedule_key` returns `EnqueueOutcome::Existing` on the second call.
