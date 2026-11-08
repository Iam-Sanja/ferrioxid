# Background jobs

[Overview](index.md) · Worker: [workers.md](workers.md) · Retries: [job-retries.md](job-retries.md) · Mail: [job-mail-integration.md](job-mail-integration.md)

**Implemented.** Milestone v0.27.1. Cargo version `0.1.0`. The queue is in `crates/ferrioxid-jobs`. There is no redis and no second database engine.

## Delivery

Jobs use **at-least-once** execution: a handler may receive the same job more than once. Ferrioxid provides no exactly-once guarantee. SMTP and other external effects cannot participate in the job’s database transaction. SMTP acceptance does not confirm inbox delivery.

## Public API

```rust
use ferrioxid::jobs::{JobContext, JobError, JobHandler, JobRegistry, Queue, When};
use ferrioxid::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pulse { label: String }

struct PulseHandler;

#[ferrioxid::async_trait::async_trait]
impl JobHandler for PulseHandler {
    const NAME: &'static str = "notes_pulse";
    const VERSION: u32 = 1;
    type Payload = Pulse;

    async fn run(&self, payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        if payload.label.is_empty() {
            return Err(JobError::permanent("notes.empty"));
        }
        Ok(())
    }
}

let mut registry = JobRegistry::new();
registry.register(PulseHandler)?;
queue.enqueue::<PulseHandler>(&Pulse { label: "sofort".into() }, When::now()).await?;
queue.enqueue::<PulseHandler>(&Pulse { label: "spaeter".into() }, When::at(1_800_000_030)).await?;
queue.enqueue::<PulseHandler>(&Pulse { label: "wieder".into() }, When::every("notes-pulse", 60)).await?;
```

`JobRegistry` belongs to the process that builds the worker. There is no global registry and no reloading of programs. `NAME` consists of lower case, numerals and underscores. `VERSION` starts at 1. The payload is a JSON object, maximum 16 KiB. Keys such as `password`, `token`, `secret` and `reset_url` are rejected.

`When::now` is due immediately. `When::at` sets `available_at` as a Unix second in UTC. `When::every` carries an interval of 1 to 2 592 000 seconds and a `schedule_key`. There is no cron syntax.

## Status

| Status | Meaning |
|---|---|
| `pending` | Waiting until `available_at` |
| `running` |A worker keeps the lease|
| `retry` |temporary error, `next_attempt_at` lies in the future|
| `succeeded` | Completed |
| `failed` | Permanent failure or attempts exhausted |
| `cancelled` |Aborted by the operator|

Workers claim due `pending` and `retry` jobs, and may reclaim `running` jobs whose leases expired. They do not automatically resume succeeded, failed or cancelled jobs. CLI retry and cancellation follow the allowed state transitions; see [job-retries.md](job-retries.md) and [workers.md](workers.md).

## Schema

The migration `crates/ferrioxid-jobs/migrations/0001_jobs.json` creates `_ferrum_jobs`, with history in `_ferrum_job_migrations`. Auth migrations `0001` to `0004` remain unchanged. `ferrioxid migrate up` applies the job migration; workers and `ferrioxid dev` do not.

Timestamps are UTC Unix seconds. Reusing an `idempotency_key` returns the existing job. A `schedule_key` belongs to one row at a time and transfers to the next scheduled job after success. Missed intervals are skipped to avoid a backlog when a worker has been stopped.

The worker deletes completed jobs older than `jobs.retention_hours` in batches of 100 rows. Active jobs remain.

The example with Instant, Delay, Interval and Mail Job is in `examples/notes`. `examples/notes/src/bin/worker.rs` executes the same handlers in its own process. Myshop remains unchanged.

## Procedure in a project

1. Defining the job. One type implements `JobHandler` with `NAME`, `VERSION` and a JSON object as a payload.
2. Register handlers. The app implements `FerrioxidApplication::register_jobs`. `App::install` does not call this method.
3. Enqueue a job from the web process with `queue.enqueue::<Handler>(...)`. First create `_ferrum_jobs` with `ferrioxid migrate up`.
4. Starting the worker. `ferrioxid generate worker` creates `src/bin/worker.rs` if the file is missing. The process loads `ferrum.toml`, connects the configured database and calls `WorkerHost::run_until_ctrl_c`. `ferrioxid dev` does not start it.
5. Inspect status with `ferrioxid jobs list` or `ferrioxid jobs inspect <id>`. Output includes the handler, version, attempt count, timestamps and error code. Payloads remain hidden unless `--show-payload` is supplied.
6. Handle failures with `JobError::retry` or `JobError::permanent`. Operators can use `ferrioxid jobs retry <id> --yes` or `ferrioxid jobs cancel <id> --yes` for eligible states.

An unknown handler remains `pending` and does not increase `attempts`. If the migration is missing, the worker ends with `job.migration.missing` and Exitcode 2.
