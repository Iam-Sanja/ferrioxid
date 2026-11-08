# Worker

[For an overview](index.md) · Jobs: [jobs.md](jobs.md) · Security: [job-security.md](job-security.md)]

**ACT.** Functional status v0.27.1. `ferrioxid worker` does not start a web server and only runs the mail handler. `ferrioxid dev` does not start a worker. An installed app does not perform jobs through `App::install`. Application handlers start via `WorkerHost` in the project binary.

## Public launch API

```rust
let project = ferrioxid::migrations::discover_project(&root)?;
let mut host = ferrioxid::jobs::WorkerHost::from_project(&project)
    .with_mail(ferrioxid::mail_for(&project));
NotesApp.register_jobs(host.registry_mut())?;
host.run_until_ctrl_c()?;
```

`WorkerHost::new` takes a `WorkerConfig`. `from_project` reads `[jobs]` and the resolved database URL. `register` and `registry_mut` enter handlers into the same `JobRegistry`. A second trader with the same `NAME` provides `job.handler`. `with_mail` supplements `ferrioxid_mail_notice` if this name is still missing. `prepare` checks the SQLite file and `_ferrum_jobs` and returns the existing `Worker`. `run_until_ctrl_c` creates the Tokyo runtime and ends the loop to Ctrl+C.

The API does not migrate and does not open a router. If the SQLite file is missing, `job.database` is created and no file is created. If the table is missing, `job.migration.missing` is created. `jobs.enabled = false` is `job.disabled`.

SQLite uses the existing pool with a connection. `effective_concurrency` is then 1, even if `[jobs].concurrency` is higher. For PostgreSQL, the upper limit remains 4. The loop claims this number successively on the same connection. Simultaneous workers are separate processes.

`ferrioxid generate worker` writes `src/bin/worker.rs`. The file contains a compilable example `app_example`, which is replaced by `register_jobs` of its own app. An existing file is not overwritten. Windows and Linux use the same source code and `tokio::signal::ctrl_c`. A worker can also set up on `WorkerHost` by hand, without the generator.

## Start

```powershell
ferrioxid worker
ferrioxid worker --concurrency 2 --poll-ms 500
```

The CLI worker only registers `ferrioxid_mail_notice`. Handlers of an app start `src/bin/worker.rs`. The CLI does not load any Rust code and no unknown Crate.

If the SQLite file is missing, it is not created. If `_ferrum_jobs` is missing, the process ends with Exitcode 2 and `job.migration.missing`. `jobs.enabled = false` also terminates the worker with Exitcode 2. A clean stop signal ends with 0. A database error ends with 1.

Ctrl+C sets a stop flag on Windows and Linux. The worker does not claim a new job afterwards and ends the current execution. A trader is not rolled back in the middle of an external effect.

## Lease

The claim is a conditional `UPDATE`. The new line gets `lease_owner`, a new `lease_token` and `lease_expires_at`. Only the update that hits exactly one line owns the job. A second worker will not receive the same line until the lease has expired. Then `attempts` increases.

The financial statement examines `lease_token`, `status = running` and `lease_expires_at >= now`. If this is not the case, `Finish::LostLease` remains. The old worker does not put the job on `succeeded`. The lease is at least the trader time limit plus five seconds and a maximum of 600 seconds. The trader limit is between 1 and 120 seconds.

SQLite serializes write accesses. A locked write attempt provides `job.locked`. The worker tries the next run instead of saving the job as a success. The parallelism of a SQLite worker remains 1. PostgreSQL accepts 1 to 4 consecutive claims per pass. The standard is 1.

## Limitations of execution

A trader timeout discards the future at the next `.await`. Code behind this Await does not work. An effect that happened before the Await remains. SMTP that the server has already accepted will not be withdrawn. `JobHandler` does not receive an abort signal. A task started by the trader himself continues.

After the timeout, the worker saves `retry` with `job.timeout`. If the attempts are exhausted, the job `failed` with `job.attempts`.

If the worker loses the lease, `finish_success` provides `Finish::LostLease`. The old token does not set the job on `succeeded`. The trader may have already executed the external effect. The new owner executes it again.

Ctrl+C sets the stop flag between two claims. The current `run_once` runs until the result or until the timeout. The current effect is not rolled back.

Delivery is at least once. An Exactly-once guarantee does not provide ferrioxide. Two processes cannot have the same due job both as a valid claim. After an expired lease, a second run can repeat the same effect.

## Settlements

`[jobs]` in `ferrum.toml` is optional.

|Keys| Standard | Grenze |
|---|---|---|
| `enabled` | `true` |Switches only the worker, not the web server|
| `poll_ms` | `1000` | 50 bis 60000 |
| `concurrency` | `1` |1 to 4; for SQLite, the worker remains at 1|
| `lease_secs` | `60` | 5 bis 600 |
| `max_attempts` | `5` | 1 bis 20 |
| `retention_hours` | `168` | 1 bis 2160 |
| `jitter` | `true` |Scatter the backoff|

A project without `[jobs]` and without Worker starts as before.
