# Job-Sicherheit

[Zur Übersicht](index.md) · Jobs: [jobs.md](jobs.md) · Mail: [job-mail-integration.md](job-mail-integration.md)

**IST.** Job names and payloads from an HTTP request are not executed. The registry only contains types that the Worker process has explicitly registered.

## Grenzen

- An unknown trader remains `pending` and is not executed.
- A different payload version ends as `failed` with `job.version`.
- No JSON object, an unknown field at `deny_unknown_fields` and more than 16 KiB end before or upon execution as `job.payload.invalid` or `job.payload.large`.
- Secret keys are not stored.
- Error texts and tracing call codes, not the payload and not `FERRUM_SMTP_PASSWORD`.
- A trader code with spaces, capitals or `password=` becomes `job.error`. In `_ferrum_jobs.error_code` is this code, not the detailed text.
- `JobError::message` withholds a message if it contains `password`, `secret`, `token`, `authorization`, `cookie` or `reset_url`.
- SQL identifiers come from fixed column names. Search columns are only `idempotency_key` and `schedule_key`. Values are bound.
- There is no way to turn a job line into a shell command.
- `max_attempts` over 20 is not accepted.
- A lost lease must not overwrite the new owner.
- Two workers do not get the same due job as a valid claim.
- Reading CLI commands do not create an SQLite file or a history.

`ferrioxid jobs inspect <id>` shows status, handler, version, trials, times and error code. The payload appears as `payload [redigiert, N Bytes]`. `--show-payload` outputs the stored JSON and warns stderr with `job.payload.visible`. This expressly requested payload may also contain e-mail addresses and other personal data that the trader has stored as normal fields. The lease token is not issued in either case.

Ordinary logs and tracing do not write the payload. The CLI worker and the application worker do not output them.

The tests are in `crates/ferrioxid-jobs/src/tests.rs` and `examples/notes/tests/jobs.rs`.
