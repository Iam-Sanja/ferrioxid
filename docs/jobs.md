# Hintergrundjobs

[Zur Übersicht](index.md) · Worker: [workers.md](workers.md) · Retries: [job-retries.md](job-retries.md) · Mail: [job-mail-integration.md](job-mail-integration.md)

**IST.** Functional status v0.27.1. Cargo version `0.1.0`. The queue is in `crates/ferrioxid-jobs`. There is no redis and no second database engine.

## Zustellung

Ferrioxide guarantees **at least once**. A trader can see the same job more than once. Exactly once does not exist. SMTP and other external effects cannot be fixed in the same transaction as the job. An accepted SMTP response is not a delivery confirmation.

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

## Zustand

| Zustand | Bedeutung |
|---|---|
| `pending` | wartet auf `available_at` |
| `running` |A worker keeps the lease|
| `retry` |temporary error, `next_attempt_at` lies in the future|
| `succeeded` | abgeschlossen |
| `failed` |Final or trials exhausted|
| `cancelled` |Aborted by the operator|

`pending` and `retry` are claimed. `running` with expired lease also. Success, permanent error and termination are over for the automatic worker. `retry` and `cancel` via the CLI reset the allowed transitions. Details are available in [job-retries.md](job-retries.md) and [workers.md](workers.md)].

## Schema

The migration `crates/ferrioxid-jobs/migrations/0001_jobs.json` creates `_ferrum_jobs`. The story is called `_ferrum_job_migrations`. Auth migrations `0001` to `0004` remain unchanged. `ferrioxid migrate up` applies the job migration. The Worker and `ferrioxid dev` do not do this.

Times are UTC-Unix seconds. A unique `idempotency_key` delivers the same job on the second entry. A `schedule_key` may hang on only one line at a time. After success, it is passed on to the next maturity. Missed intervals are skipped so that a silent worker does not create an avalanche.

Completed jobs older than `jobs.retention_hours` are deleted by the worker in blocks of 100 lines. Continuing jobs.

The example with Instant, Delay, Interval and Mail Job is in `examples/notes`. `examples/notes/src/bin/worker.rs` executes the same traders in its own process. Myshop remains unchanged.

## Procedure in a project

1. Defining the job. One type implements `JobHandler` with `NAME`, `VERSION` and a JSON object as a payload.
2. Register traders. The app implements `FerrioxidApplication::register_jobs`. `App::install` does not call this method.
3. A job line. The web process uses `queue.enqueue::<Handler>(...)`. For this, `_ferrum_jobs` must already exist through `ferrioxid migrate up`.
4. Starting the worker. `ferrioxid generate worker` creates `src/bin/worker.rs` if the file is missing. The process loads `ferrum.toml`, connects the configured database and calls `WorkerHost::run_until_ctrl_c`. `ferrioxid dev` does not start it.
5. Check job status. `ferrioxid jobs list` and `ferrioxid jobs inspect <id>` show status, handler, version, trials, times and error code. The payload remains hidden until `--show-payload` is set.
6. Treat errors. `JobError::retry` is planning a new attempt. `JobError::permanent` terminates the job. `ferrioxid jobs retry <id> --yes` and `ferrioxid jobs cancel <id> --yes` apply to the permitted states.

An unknown trader remains `pending` and does not increase `attempts`. If the migration is missing, the worker ends with `job.migration.missing` and Exitcode 2.
