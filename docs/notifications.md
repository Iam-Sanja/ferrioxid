# Benachrichtigungen

[Zur Übersicht](index.md) · E-Mail: [mail.md](mail.md) · Passwort: [password-reset.md](password-reset.md)

**Stand:** v0.23. Das ist eine interne, typisierte Schnittstelle. Es gibt kein Benachrichtigungscenter im Admin und keine Ereignisnamen aus HTTP-Parametern.

## Ereignisse

`ferrum::mail::Notice` kennt `PasswordResetRequested`, `PasswordChanged`, `AccountDeactivated` und `SecurityNotice`. Jede Variante trägt die Zieladresse und `de` oder `en`. Ein unbekannter Sprachwert wird Deutsch. `PasswordResetRequested` trägt zusätzlich die Reset-URL und die Minuten. `SecurityNotice` trägt eine Zusammenfassung von höchstens 200 Zeichen ohne Steuerzeichen. Keine Variante trägt ein Passwort, einen Hash oder einen Token als eigenes Feld. Die Reset-URL enthält den Token, weil die E-Mail ihn zustellen muss. Sie wird nicht ins Tracing geschrieben.

`MailService::notify` rendert Text und HTML mit Tera und ruft `send` auf. HTML-Vorlagen escapen eingefügte Werte. Betreffzeilen sind fest je Art und Sprache: „Passwort zurücksetzen“, „Passwort geändert“, „Konto deaktiviert“, „Sicherheitshinweis“ und die englischen Gegenstücke.

Der Reset-Ablauf sendet `PasswordResetRequested` und nach dem Commit `PasswordChanged`. `AccountDeactivated` und `SecurityNotice` sind aufrufbar und noch nicht an `set_user_active` oder das Verzeichnis gehängt. Das vermeidet eine zusätzliche E-Mail in bestehenden Admin-Tests.

```rust
use ferrum::mail::{MailService, Notice};

let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
mail.notify(Notice::SecurityNotice {
    to: "person@example.com".into(),
    locale: "en".into(),
    summary: "A staff role was removed.".into(),
}).await?;
assert_eq!(inbox.messages()[0].subject, "Security notice");
```

Dieselbe Methode kann später ein Hintergrundprogramm aufrufen. Sie startet selbst keine Warteschlange.
