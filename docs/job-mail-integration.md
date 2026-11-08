# Jobs und Mail

[Zur Übersicht](index.md) · Mail: [mail.md](mail.md) · Reset: [password-reset.md](password-reset.md) · Jobs: [jobs.md](jobs.md)

**IST.** Die Outbox `_ferrum_mail_outbox` aus `0004_password_reset` bleibt unverändert. Der Passwort-Reset versendet weiter genau einmal nach dem Commit und speichert den Roh-Token nur im Speicher und in der Nachricht.

## Was der Worker nicht wiederholt

Der Reset-Token steht nicht in der Outbox und nicht in `_ferrum_jobs`. `enqueue_notice` lehnt `Notice::PasswordResetRequested` mit `job.mail.reset_refused` ab. Der eingebaute Handler `ferrioxid_mail_notice` versendet `password_reset` nicht, auch wenn eine Zeile diesen Kind-Namen trägt.

Bestehende Outbox-Zeilen liest der Worker nicht. `sweep` in `ferrioxid-auth` markiert veraltete Reset-Zeilen weiter als `mail.expired` und verbraucht den Token. Ein stiller Worker erzeugt daraus keine zweite E-Mail.

## Gewöhnliche Hinweise

`PasswordChanged`, `AccountDeactivated` und `SecurityNotice` können nach einem erfolgreichen Commit als Job eingetragen werden:

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

Der Payload enthält Art, Empfänger, Sprache und bei einer Sicherheitsnotiz eine kurze Zusammenfassung. Keinen Reset-Link und kein SMTP-Passwort. `mail.timeout` und `mail.unavailable` sind Wiederholungen. `mail.disabled`, `mail.invalid` und `mail.rejected` sind endgültig. Erfolg heißt: das konfigurierte Backend hat die Nachricht angenommen. Das ist keine Zustellbestätigung.

Tests nutzen `MailService::memory` und öffnen keine SMTP-Verbindung.
