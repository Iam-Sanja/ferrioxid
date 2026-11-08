# E-Mail

[Zur Übersicht](index.md) · SMTP: [mail-smtp.md](mail-smtp.md) · Sicherheit: [mail-security.md](mail-security.md) · Hinweise: [notifications.md](notifications.md) · Passwort: [password-reset.md](password-reset.md)

**Stand:** v0.23. The cargo version remains `0.1.0`. The crate is `ferrioxid-mail`. The facade is `ferrioxid::mail`.

## Zweck

`MailService` sends a typed message. Sender, recipient, subject, text, optional HTML and optional Reply-To are checked before transport. The service does not open a connection to an external mailbox in the test.

## Backends

| Backend | Verhalten |
|---|---|
| `disabled` |`send` provides `mail.disabled`. This is the standard without `[mail]`.|
| `console` |Writes the message to stderr and logs only `mail accepted by the console transport`.|
| `memory` |Holds the message in the process. `MemoryInbox::messages` returns them.|
| `smtp` |An attempt via STARTTLS or implicit TLS. Details in [mail-smtp.md](mail-smtp.md)].|

`FERRUM_ENV=production` rejects `console` and `memory` and falls back to `disabled`. The password is never in `ferrum.toml`. SMTP reads `FERRUM_SMTP_USERNAME` and `FERRUM_SMTP_PASSWORD`.

## Konfiguration

```toml
[mail]
backend = "disabled"
public_origin = "http://127.0.0.1:3000"
reset_ttl_minutes = 30
```

For SMTP additionally `host`, `port`, `security = "starttls"` or `"tls"`, `from` and `timeout_secs` between 1 and 30. `reset_ttl_minutes` is between 5 and 1440. An active backend needs a plausible sender address and a trusted `public_origin`: `http://127.0.0.1`, `http://localhost` or `https` without user info, path, query and fragment. Keys `mail.password`, `mail.secret` and `mail.smtp_password` are an error.

Umgebungsvariablen überschreiben die Datei: `FERRUM_MAIL_BACKEND`, `FERRUM_MAIL_FROM`, `FERRUM_MAIL_REPLY_TO`, `FERRUM_PUBLIC_ORIGIN`, `FERRUM_RESET_TTL_MINUTES`, `FERRUM_SMTP_HOST`, `FERRUM_SMTP_PORT`, `FERRUM_SMTP_SECURITY`, `FERRUM_SMTP_TIMEOUT_SECS`, `FERRUM_SMTP_USERNAME`, `FERRUM_SMTP_PASSWORD`.

`ferrioxid check` and `ferrioxid doctor` do not open a socket. With `smtp` without `FERRUM_SMTP_PASSWORD`, the warning `mail.credentials` arises. `check --strict` ends this warning with Exitcode 2. `ferrioxid upgrade` does not rewrite an existing `ferrum.toml`. The template version remains `0.21`.

## API

```rust
use ferrioxid::mail::{MailConfig, MailMessage, MailService, Notice};
use std::time::Duration;

let mail = MailService::open(&MailConfig {
    backend: "disabled".into(),
    host: String::new(),
    port: 587,
    security: "starttls".into(),
    from: String::new(),
    reply_to: String::new(),
    timeout_secs: 15,
    public_origin: "http://127.0.0.1:3000".into(),
    reset_ttl_minutes: 30,
});

let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
mail.send(MailMessage {
    from: "noreply@example.com".into(),
    to: "person@example.com".into(),
    reply_to: None,
    subject: "Hinweis".into(),
    text: "Text".into(),
    html: None,
}).await?;
assert_eq!(inbox.messages().len(), 1);

mail.notify(Notice::PasswordChanged {
    to: "person@example.com".into(),
    locale: "de".into(),
}).await?;

let _ = Duration::from_secs(mail.reset_ttl_secs() as u64);
```

`App::with_mail` and `ferrioxid::mail_for(&project)` are connected to the same service. The generated templates call `mail_for`. A new project compiles with `backend = "disabled"` and does not send an email at start.

Errors are `mail.disabled`, `mail.invalid`, `mail.unavailable`, `mail.timeout` and `mail.rejected`. The display text is the code. SMTP responses and access data are not included. `Debug` for `SmtpSettings` replaces username and password with `[redacted]`.

UTF-8 remains. Subject and addresses must not contain control characters. A subject is at most 120 characters, an address 254, the text 20000 and HTML 40000.
