# SMTP

[Overview](index.md) · Service: [mail.md](mail.md) · Security: [mail-security.md](mail-security.md)

**Milestone:** v0.23. One delivery attempt per `send`. There is no plaintext fallback and no repetition within the HTTP request.

## Connection

`security = "starttls"` uses `starttls_relay`. `security = "tls"` uses `relay` for implicit TLS. Both check the certificate on the rustls standard roots. Another value does not activate SMTP. The timeout is the smaller value of configuration and 30 seconds, at least one second. An expired timeout is `mail.timeout`. A rejected or failed handover is `mail.rejected`. A non-constructable transport is `mail.unavailable`. The library message is not logged because it can repeat access data.

```rust
use ferrioxid::mail::{MailService, SmtpSecurity, SmtpSettings};
use std::time::Duration;

let mail = MailService::smtp(
    SmtpSettings {
        host: "smtp.example.com".into(),
        port: 587,
        security: SmtpSecurity::Starttls,
        username: std::env::var("FERRUM_SMTP_USERNAME").unwrap_or_default(),
        password: std::env::var("FERRUM_SMTP_PASSWORD").unwrap_or_default(),
        timeout: Duration::from_secs(15),
    },
    "noreply@example.com",
    None,
    "https://shop.example.com",
    30 * 60,
);
```

A successful SMTP result means that the server has accepted the message. It does not prove that it is in the inbox.

## Configuration

```toml
[mail]
backend = "smtp"
host = "smtp.example.com"
port = 587
security = "starttls"
from = "noreply@example.com"
timeout_secs = 15
public_origin = "https://shop.example.com"
reset_ttl_minutes = 30
```

Username and password come from `FERRUM_SMTP_USERNAME` and `FERRUM_SMTP_PASSWORD`. If one of these is missing, the service remains deactivated and writes `mail.invalid` without giving the password.

The workspace test does not address a public SMTP server. An error case against `127.0.0.1:9` only checks that the token is not usable afterwards and that the error code does not contain a secret.
