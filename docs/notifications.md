# Notifications

[Overview](index.md) · E-Mail: [mail.md](mail.md) · Password: [password-reset.md](password-reset.md)

**Milestone:** v0.23. This is an internal, typed interface. There is no notification center in the admin and no event names from HTTP parameters.

## Events

`fexu::mail::Notice` provides `PasswordResetRequested`, `PasswordChanged`, `AccountDeactivated` and `SecurityNotice`. Each variant contains the recipient address and a `de` or `en` locale; unknown locales fall back to German. `PasswordResetRequested` includes the reset URL and validity in minutes. `SecurityNotice` accepts a summary of at most 200 characters with no control characters. No variant has a dedicated password, hash or token field. The reset URL necessarily contains the token for delivery, but is never written to tracing logs.

`MailService::notify` renders text and HTML with Tera, then calls `send`. HTML templates escape inserted values. Subject lines are fixed by notice type and locale, covering password reset, password change, account deactivation and security notices in German and English.

The reset sequence sends `PasswordResetRequested` and after the commit `PasswordChanged`. `AccountDeactivated` and `SecurityNotice` are callable and not yet attached to `set_user_active` or the directory. This avoids an additional email in existing admin tests.

```rust
use fexu::mail::{MailService, Notice};

let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
mail.notify(Notice::SecurityNotice {
    to: "person@example.com".into(),
    locale: "en".into(),
    summary: "A staff role was removed.".into(),
}).await?;
assert_eq!(inbox.messages()[0].subject, "Security notice");
```

The same method can be called by a background worker; it does not start a queue itself.
