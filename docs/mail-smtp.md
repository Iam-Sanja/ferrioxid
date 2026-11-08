# SMTP

[Zur Übersicht](index.md) · Dienst: [mail.md](mail.md) · Sicherheit: [mail-security.md](mail-security.md)

**Stand:** v0.23. Ein Versandversuch je `send`. Es gibt keinen Klartext-Fallback und keine Wiederholung innerhalb des HTTP-Requests.

## Verbindung

`security = "starttls"` nutzt `starttls_relay`. `security = "tls"` nutzt `relay` für implizites TLS. Beides prüft das Zertifikat über die rustls-Standardwurzeln. Ein anderer Wert aktiviert SMTP nicht. Der Timeout ist der kleinere Wert aus Konfiguration und 30 Sekunden, mindestens eine Sekunde. Ein abgelaufener Timeout ist `mail.timeout`. Eine abgelehnte oder fehlgeschlagene Übergabe ist `mail.rejected`. Ein nicht baubarer Transport ist `mail.unavailable`. Die Bibliotheksmeldung wird nicht protokolliert, weil sie Zugangsdaten wiederholen kann.

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

Ein erfolgreiches SMTP-Ergebnis heißt, dass der Server die Nachricht angenommen hat. Es beweist nicht, dass sie im Posteingang liegt.

## Konfiguration

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

Benutzername und Passwort kommen aus `FERRUM_SMTP_USERNAME` und `FERRUM_SMTP_PASSWORD`. Fehlt eines davon, bleibt der Dienst deaktiviert und schreibt `mail.invalid`, ohne das Passwort zu nennen.

Der Workspace-Test spricht keinen öffentlichen SMTP-Server an. Ein Fehlerfall gegen `127.0.0.1:9` prüft nur, dass der Token danach nicht verwendbar ist und der Fehlercode kein Geheimnis enthält.
