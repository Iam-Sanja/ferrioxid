# Job-Retries

[For an overview](index.md) · Jobs: [jobs.md](jobs.md) · Worker: [workers.md](workers.md)]

**IST.** Attempts are limited. An unlimited repetition is rejected.

## Fehlerklassen

`JobError::retry("code")` puts the job on `retry` as long as `attempts < max_attempts`. After that, he becomes `failed` with `job.attempts`. `JobError::permanent("code")` immediately sets `failed`. A trader time limit is a retry with `mail.timeout` or `job.timeout`.

The stored code shall consist of lower case, digit, point and underscore and shall have a maximum of 64 characters. Everything else becomes `job.error`. The code does not contain a payload.

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

Without `--yes`, the exit code is 2 and the database remains unchanged. The permission is that of the local operator, the same confidence limit as `ferrioxid migrate up`. There is no HTTP endpoint for this.

## Intervalle

Only a successful deal plans the next maturity. A permanently failed interval does not generate further lines. `ferrioxid jobs retry --yes` opens the same line again. Two entries with the same `schedule_key` deliver the second call as `EnqueueOutcome::Existing`.
