# Jobs and Mail

[Zur Übersicht](index.md) · Mail: [mail.md](mail.md) · Reset: [password-reset.md](password-reset.md) · Jobs: [jobs.md](jobs.md)

**IST.** The `_ferrum_mail_outbox` outbox from `0004_password_reset` remains unchanged. The password reset continues to send exactly once after the commit and stores the raw token only in memory and in the message.

## What the worker does not repeat

The reset token is not in the outbox and not in `_ferrum_jobs`. `enqueue_notice` rejects `Notice::PasswordResetRequested` with `job.mail.reset_refused`. The built-in handler `ferrioxid_mail_notice` does not send `password_reset`, even if a line bears this child name.

Existing outbox lines are not read by the worker. `sweep` in `ferrioxid-auth` marks outdated reset lines further than `mail.expired` and consumes the token. A silent worker does not generate a second e-mail from it.

## Ordinary instructions

`PasswordChanged`, `AccountDeactivated` and `SecurityNotice` can be registered as a job after a successful commit:

```rust
ferrioxid::jobs::enqueue_notice(
    &queue,
    &ferrioxid::mail::Notice::PasswordChanged {
        to: "person@example.com".into(),
        locale: "de".into(),
    },
    ferrioxid::jobs::When::now(),
).await?;
```

The payload contains type, recipient, language and a short summary of a security note. No reset link and no SMTP password. `mail.timeout` and `mail.unavailable` are repetitions. `mail.disabled`, `mail.invalid` and `mail.rejected` are final. Success means: the configured backend has accepted the message. This is not a delivery confirmation.

Tests use `MailService::memory` and do not open an SMTP connection.
