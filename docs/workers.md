# Worker

[Overview](index.md) · Jobs: [jobs.md](jobs.md) · Security: [job-security.md](job-security.md)

**Implemented.** Milestone v0.27.1. `fexu worker` does not start a web server and only runs the mail handler. `fexu dev` does not start a worker. An installed app does not perform jobs through `App::install`. Application handlers start via `WorkerHost` in the project binary.

## Public launch API

```rust
let project = fexu::migrations::discover_project(&root)?;
let mut host = fexu::jobs::WorkerHost::from_project(&project)
    .with_mail(fexu::mail_for(&project));
NotesApp.register_jobs(host.registry_mut())?;
host.run_until_ctrl_c()?;
```

`WorkerHost::new` accepts a `WorkerConfig`; `from_project` reads `[jobs]` and the resolved database URL. `register` and `registry_mut` add handlers to the shared `JobRegistry`; duplicate `NAME` values return `job.handler`. `with_mail` adds `fexu_mail_notice` if absent. `prepare` checks the SQLite file and `_fexu_jobs`, then returns a `Worker`. `run_until_ctrl_c` creates the Tokio runtime and runs until Ctrl+C.

The API neither migrates the database nor mounts a router. A missing SQLite file returns `job.database` without creating a file. A missing jobs table returns `job.migration.missing`; `jobs.enabled = false` returns `job.disabled`.

SQLite uses the existing pool with a connection. `effective_concurrency` is then 1, even if `[jobs].concurrency` is higher. For PostgreSQL, the upper limit remains 4. The loop claims this number successively on the same connection. Simultaneous workers are separate processes.

`fexu generate worker` writes `src/bin/worker.rs`. The file contains a compilable example `app_example`, which is replaced by `register_jobs` of its own app. An existing file is not overwritten. Windows and Linux use the same source code and `tokio::signal::ctrl_c`. A worker can also set up on `WorkerHost` by hand, without the generator.

## Start

```powershell
fexu worker
fexu worker --concurrency 2 --poll-ms 500
```

The CLI worker only registers `fexu_mail_notice`. Handlers of an app start `src/bin/worker.rs`. The CLI does not load any Rust code and no unknown Crate.

If the SQLite file is missing, it is not created. If `_fexu_jobs` is missing, the process ends with Exitcode 2 and `job.migration.missing`. `jobs.enabled = false` also terminates the worker with Exitcode 2. A clean stop signal ends with 0. A database error ends with 1.

Ctrl+C sets a stop flag on Windows and Linux. The worker does not claim a new job afterwards and ends the current execution. A handler is not rolled back in the middle of an external effect.

## Lease

The claim is a conditional `UPDATE`. The new line gets `lease_owner`, a new `lease_token` and `lease_expires_at`. Only the update that hits exactly one line owns the job. A second worker will not receive the same line until the lease has expired. Then `attempts` increases.

Job completion checks `lease_token`, `status = running` and `lease_expires_at >= now`. A mismatch returns `Finish::LostLease`, preventing the old worker from marking the job `succeeded`. A lease lasts at least the handler timeout plus five seconds, up to 600 seconds. Handler timeouts range from 1 to 120 seconds.

SQLite serializes write accesses. A locked write attempt provides `job.locked`. The worker tries the next run instead of saving the job as a success. The parallelism of a SQLite worker remains 1. PostgreSQL accepts 1 to 4 consecutive claims per pass. The standard is 1.

## Limitations of execution

A handler timeout drops its future at a yield point. Code that would run after that point is not executed, but earlier external effects remain. Accepted SMTP messages cannot be recalled. `JobHandler` receives no cancellation signal, and independently spawned tasks may continue.

After the timeout, the worker saves `retry` with `job.timeout`. If the attempts are exhausted, the job `failed` with `job.attempts`.

If the worker loses the lease, `finish_success` provides `Finish::LostLease`. The old token does not set the job on `succeeded`. The handler may have already executed the external effect. The new owner executes it again.

Ctrl+C sets the stop flag between two claims. The current `run_once` runs until the result or until the timeout. The current effect is not rolled back.

Delivery is at least once. An Exactly-once guarantee does not provide Fexu. Two processes cannot have the same due job both as a valid claim. After an expired lease, a second run can repeat the same effect.

## Configuration

`[jobs]` in `fexu.toml` is optional.

| Key | Default | Limit |
|---|---|---|
| `enabled` | `true` |Switches only the worker, not the web server|
| `poll_ms` | `1000` | 50 to 60000 |
| `concurrency` | `1` |1 to 4; for SQLite, the worker remains at 1|
| `lease_secs` | `60` | 5 to 600 |
| `max_attempts` | `5` | 1 to 20 |
| `retention_hours` | `168` | 1 to 2160 |
| `jitter` | `true` | Add jitter to retry backoff |

A project without `[jobs]` and without Worker starts as before.
