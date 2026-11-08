# Passwort zurücksetzen

[Zur Übersicht](index.md) · Auth: [auth.md](auth.md) · E-Mail: [mail.md](mail.md) · Sicherheit: [mail-security.md](mail-security.md)

**Stand:** v0.23. Die Cargo-Version bleibt `0.1.0`. Die Migration ist `crates/ferrioxid-auth/migrations/0004_password_reset.json`. `0001`, `0002` und `0003` bleiben unverändert.

## Routen

`App::with_auth` hängt die Seiten an. Sie verlangen keinen Staff-Status.

| Methode | Pfad | Ergebnis |
|---|---|---|
| `GET` | `/auth/password-reset` | Formular, Cookie `ferrioxid_reset_csrf` |
| `POST` | `/auth/password-reset` | Dieselbe Annahme für bekannte und unbekannte Adressen |
| `GET` | `/auth/password-reset/confirm?token=` | Tauscht einen gültigen Token gegen Cookie `ferrioxid_reset` und antwortet 303 auf `/auth/password-reset/choose` |
| `GET` | `/auth/password-reset/choose` | Passwort und Wiederholung |
| `POST` | `/auth/password-reset/choose` | Setzt das Passwort oder zeigt einen sichtbaren Fehler |

Die öffentliche Annahme lautet: „Wenn ein Konto mit dieser Adresse existiert, wurde eine E-Mail mit weiteren Schritten angefordert.“ Englisch: „If an account with this address exists, an email with further steps was requested.“ Unbekannte Adresse, deaktiviertes Konto, ungültige Adresse, Rate-Limit, fehlendes Mail-Backend, ungeeigneter Origin und Versandfehler liefern dieselbe Seite. Ferrioxid kennt kein separates „nicht verifiziert“. `is_active = false` wird wie eine unbekannte Adresse behandelt und erhält keine E-Mail. CSRF- oder Origin-Fehler sind 403 und nennen die Adresse nicht. Ohne Auth-Dienst antwortet POST mit 404. Ein Körper über 8 KiB ist 403.

Die Seiten sind serverseitiges HTML ohne externe Datei und ohne JavaScript. `Referrer-Policy` ist `no-referrer`. Die Content-Security-Policy erlaubt kein externes Asset. Der Admin-Login bleibt unverändert und enthält keinen zusätzlichen Link.

## Token

32 Byte aus `getrandom`, kodiert als Base64-URL ohne Padding. Das sind 256 Bit. Gespeichert wird nur SHA-256-Hex in `_ferrum_password_resets`. Der Rohwert steht im Prozessspeicher und in der E-Mail. Audit, Session-Log und die Outbox speichern ihn nicht.

Der Token ist an `user_id` gebunden, einmal verwendbar und läuft nach `reset_ttl_secs` ab. Ein GET verbraucht ihn nicht, damit ein Mailscanner den Link nicht entwertet. Er legt `_ferrum_password_reset_exchanges` an. Das Austauschgeheimnis ist ebenfalls 32 Byte, nur als Hash gespeichert, und lebt höchstens zehn Minuten, nie länger als der Reset. Das Formular sendet danach das Cookie, nicht den Mail-Token.

Höchstens drei unbenutzte, nicht abgelaufene Tokens bleiben je Benutzer. Eine neue Anfrage verbraucht ältere überzählige Tokens in derselben Transaktion. Das Rate-Limit liegt im Prozess, getrennt vom Login: `reset-ip:{adresse}` mit dem Vierfachen von `login_max_attempts`, `reset-mail:{adresse}` mit `login_max_attempts`, Fenster `login_window_secs`. Jede Anfrage zählt, auch eine unbekannte Adresse.

## Abschluss

`complete_reset` prüft Gleichheit, `validate_new_password` und dass das neue Passwort nicht dem aktuellen Argon2id-Hash entspricht. Der neue Hash entsteht vor `begin`. Danach gilt ein bedingtes `UPDATE` auf `used_at IS NULL AND expires_at > now`. Nur `rows_affected = 1` setzt das Passwort, widerruft alle Sitzungen, verbraucht die übrigen Reset-Tokens, löscht die Austauschzeilen und schreibt das Audit. Sonst wird zurückgerollt. Zwei parallele Abschlüsse können höchstens einmal erfolgreich sein. Das Audit nutzt Modell `_ferrum_users`, Aktion `password_reset`, Ergebnis `ok` und die Zusammenfassung `password reset completed`.

Die Reset-URL entsteht aus `public_origin`, nie aus dem Host-Header.

```rust
use ferrioxid::auth::{complete_reset, exchange_token, request_reset};
use ferrioxid::mail::MailService;

let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
request_reset(&auth, Some(&mail), "person@example.com", "203.0.113.10", "de").await;
let token = inbox.messages()[0].text.lines().find_map(|line| line.split("token=").nth(1)).unwrap().trim();
let opened = exchange_token(&auth, token).await?;
complete_reset(&auth, Some(&mail), &opened.secret, "replacement-horse-1", "replacement-horse-1", "de").await?;
```

Die Anwendung registriert denselben Ablauf über `with_mail` und `with_auth`. Ein separates Mount ist nicht nötig.
