# E-Mail

[Zur Übersicht](index.md) · SMTP: [mail-smtp.md](mail-smtp.md) · Sicherheit: [mail-security.md](mail-security.md) · Hinweise: [notifications.md](notifications.md) · Passwort: [password-reset.md](password-reset.md)

**Stand:** v0.23. Die Cargo-Version bleibt `0.1.0`. Die Crate ist `ferrum-mail`. Die Fassade ist `ferrum::mail`.

## Zweck

`MailService` versendet eine typisierte Nachricht. Absender, Empfänger, Betreff, Text, optionales HTML und optionales Reply-To werden vor dem Transport geprüft. Der Dienst öffnet im Test keine Verbindung zu einem externen Postfach.

## Backends

| Backend | Verhalten |
|---|---|
| `disabled` | `send` liefert `mail.disabled`. Das ist der Standard ohne `[mail]`. |
| `console` | Schreibt die Nachricht nach stderr und protokolliert nur `mail accepted by the console transport`. |
| `memory` | Hält die Nachricht im Prozess. `MemoryInbox::messages` gibt sie zurück. |
| `smtp` | Ein Versuch über STARTTLS oder implizites TLS. Details in [mail-smtp.md](mail-smtp.md). |

`FERRUM_ENV=production` lehnt `console` und `memory` ab und fällt auf `disabled` zurück. Das Passwort steht nie in `ferrum.toml`. SMTP liest `FERRUM_SMTP_USERNAME` und `FERRUM_SMTP_PASSWORD`.

## Konfiguration

```toml
[mail]
backend = "disabled"
public_origin = "http://127.0.0.1:3000"
reset_ttl_minutes = 30
```

Für SMTP zusätzlich `host`, `port`, `security = "starttls"` oder `"tls"`, `from` und `timeout_secs` zwischen 1 und 30. `reset_ttl_minutes` liegt zwischen 5 und 1440. Ein aktives Backend braucht eine plausible Absenderadresse und einen vertrauenswürdigen `public_origin`: `http://127.0.0.1`, `http://localhost` oder `https` ohne Benutzerinfo, Pfad, Query und Fragment. Schlüssel `mail.password`, `mail.secret` und `mail.smtp_password` sind ein Fehler.

Umgebungsvariablen überschreiben die Datei: `FERRUM_MAIL_BACKEND`, `FERRUM_MAIL_FROM`, `FERRUM_MAIL_REPLY_TO`, `FERRUM_PUBLIC_ORIGIN`, `FERRUM_RESET_TTL_MINUTES`, `FERRUM_SMTP_HOST`, `FERRUM_SMTP_PORT`, `FERRUM_SMTP_SECURITY`, `FERRUM_SMTP_TIMEOUT_SECS`, `FERRUM_SMTP_USERNAME`, `FERRUM_SMTP_PASSWORD`.

`ferrum check` und `ferrum doctor` öffnen keinen Socket. Bei `smtp` ohne `FERRUM_SMTP_PASSWORD` entsteht die Warnung `mail.credentials`. `check --strict` beendet diese Warnung mit Exitcode 2. `ferrum upgrade` schreibt keine vorhandene `ferrum.toml` um. Die Template-Version bleibt `0.21`.

## API

```rust
use ferrum::mail::{MailConfig, MailMessage, MailService, Notice};
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

`App::with_mail` und `ferrum::mail_for(&project)` hängen denselben Dienst an die Anwendung. Die generierten Vorlagen rufen `mail_for` auf. Ein neues Projekt kompiliert mit `backend = "disabled"` und versendet beim Start keine E-Mail.

Fehler sind `mail.disabled`, `mail.invalid`, `mail.unavailable`, `mail.timeout` und `mail.rejected`. Der Anzeigetext ist der Code. SMTP-Antworten und Zugangsdaten stehen nicht darin. `Debug` für `SmtpSettings` ersetzt Benutzername und Passwort durch `[redacted]`.

UTF-8 bleibt erhalten. Betreff und Adressen dürfen keine Steuerzeichen enthalten. Ein Betreff ist höchstens 120 Zeichen, eine Adresse 254, der Text 20000 und HTML 40000.
