# Jobs and Mail

[Overview](index.md) · Mail: [mail.md](mail.md) · Reset: [password-reset.md](password-reset.md) · Jobs: [jobs.md](jobs.md)

**Implemented.** The `_ferrioxid_mail_outbox` from `0004_password_reset` remains unchanged. Password reset makes one delivery attempt after commit, keeping the raw token only in memory and in the message.

## What the worker does not repeat

Reset tokens are stored in neither the outbox nor `_ferrioxid_jobs`. `enqueue_notice` rejects `Notice::PasswordResetRequested` with `job.mail.reset_refused`. The built-in `ferrioxid_mail_notice` handler refuses `password_reset`, even if a job payload requests that notice type.

The worker does not read existing outbox rows. `sweep` in `ferrioxid-auth` marks stale reset rows as `mail.expired` and consumes their tokens. A stopped worker cannot cause a second reset email.

## Typed notifications

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

The payload contains the notice type, recipient, locale and optional short security summary, with no reset link or SMTP password. `mail.timeout` and `mail.unavailable` are retryable; `mail.disabled`, `mail.invalid` and `mail.rejected` are permanent failures. Success means the configured backend accepted the message, not that it reached the inbox.

Tests use `MailService::memory` and do not open an SMTP connection.
