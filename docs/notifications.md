# Benachrichtigungen

[Zur Übersicht](index.md) · E-Mail: [mail.md](mail.md) · Passwort: [password-reset.md](password-reset.md)

**Stand:** v0.23. This is an internal, typed interface. There is no notification center in the admin and no event names from HTTP parameters.

## Ereignisse

`ferrioxid::mail::Notice` knows `PasswordResetRequested`, `PasswordChanged`, `AccountDeactivated` and `SecurityNotice`. Each variant has the destination address and `de` or `en`. An unknown language value becomes German. `PasswordResetRequested` also carries the reset URL and the minutes. `SecurityNotice` has a summary of a maximum of 200 characters without tax characters. No variant carries a password, a hash or a token as its own field. The reset URL contains the token because the email has to deliver it. It is not written into tracing.

`MailService::notify` renders text and HTML with Tera and calls `send`. HTML templates escape inserted values. Subject lines are fixed by type and language: “Reset password”, “changed password”, “Account deactivated”, “Security Note” and the English counterparts.

The reset sequence sends `PasswordResetRequested` and after the commit `PasswordChanged`. `AccountDeactivated` and `SecurityNotice` are callable and not yet attached to `set_user_active` or the directory. This avoids an additional email in existing admin tests.

```rust
use ferrioxid::mail::{MailService, Notice};

let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
mail.notify(Notice::SecurityNotice {
    to: "person@example.com".into(),
    locale: "en".into(),
    summary: "A staff role was removed.".into(),
}).await?;
assert_eq!(inbox.messages()[0].subject, "Security notice");
```

The same method can later call a background program. She herself does not start a queue.
