# Mail and reset security

[Zur Übersicht](index.md) · Passwort: [password-reset.md](password-reset.md) · SMTP: [mail-smtp.md](mail-smtp.md)

**Stand:** v0.23. The cargo version remains `0.1.0`.

## What is stored

| Ort | Inhalt |
|---|---|
| `_ferrum_password_resets.token_hash` |SHA-256-hex, unique|
| `_ferrum_password_reset_exchanges.secret_hash` |SHA-256-hex, unique|
| `_ferrum_mail_outbox` |Type, recipient address, optional `reset_id`, timestamp, trial number, error code|
| `_ferrum_admin_audit` |`password reset completed`, without token and without password|
| Tracing | Codes wie `reset.limited`, `mail.timeout`, `mail.rejected` |

The outbox does not contain the raw token. Console and memory may contain it because they are the delivery itself. Both are rejected in production.

## Shipping and transaction

SMTP cannot be rolled back with SQLite. The order is therefore fixed: hash and outbox line are committed, after which the message is sent from memory. A unique SMTP error consumes the token and sets `failed_at` plus the code. A timeout only sets `mail.timeout` and leaves `sent_at` and `failed_at` empty. The next reset request marks such lines as `mail.expired` after 120 seconds and consumes the associated token. There is no second automatic attempt.

This is not guaranteed delivery. An accepted SMTP shipment may miss the inbox. A process termination after the commit and before the marking can devalue an already accepted token the next time it is cleaned up. The confirmation email after a successful password change is outside the password transaction. If it fails, the new password remains valid.

## Weitere Grenzen

- The rate limit is process-local and does not share between instances.
- `ferrioxid doctor` checks the form and presence of `FERRUM_SMTP_PASSWORD`, not the accessibility of the server.
- Certificate verification is active. An insecure TLS fallback does not exist.
- The reset pages do not load a third-party file and do not set an Analytics call.
- A manipulated, expired or foreign token looks like an invalid link from the outside.
- Parallel trades serialize on one SQLite connection. The conditional update only allows for success.
