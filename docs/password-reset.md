# Reset the password

[For overview](index.md) · Auth: [auth.md](auth.md) · Email: [mail.md](mail.md) · Security: [mail-security.md](mail-security.md)]

**Stand:** v0.23. The cargo version remains `0.1.0`. The migration is `crates/ferrioxid-auth/migrations/0004_password_reset.json`. `0001`, `0002` and `0003` remain unchanged.

## Routen

`App::with_auth` attaches the pages. They do not require Staff status.

| Methode | Pfad | Ergebnis |
|---|---|---|
| `GET` | `/auth/password-reset` | Formular, Cookie `ferrioxid_reset_csrf` |
| `POST` | `/auth/password-reset` |Same assumption for known and unknown addresses|
| `GET` | `/auth/password-reset/confirm?token=` |Exchanges a valid token for cookie `ferrioxid_reset` and responds 303 to `/auth/password-reset/choose`|
| `GET` | `/auth/password-reset/choose` |Password and repetition|
| `POST` | `/auth/password-reset/choose` |Sets the password or shows a visible error|

The public assumption is: “If an account with this address exists, an email with further steps has been requested.” If an account with this address exists, an email with further steps was requested. Unknown address, deactivated account, invalid address, rate limit, missing mail backend, inappropriate origin and shipping errors deliver the same page. Ferrioxide does not have a separate “not verified”. `is_active = false` is treated like an unknown address and does not receive an email. CSRF or Origin errors are 403 and do not name the address. Without Auth service, POST responds with 404. A body above 8 KiB is 403.

The pages are server-side HTML without external file and without JavaScript. `Referrer-Policy` is `no-referrer`. The content security policy does not allow an external asset. The admin login remains unchanged and does not contain an additional link.

## Token

32 bytes from `getrandom`, encoded as Base64 URL without padding. That's 256 bits. Only SHA-256-Hex is stored in `_ferrum_password_resets`. The raw value is in the process memory and in the e-mail. Audit, session log and the outbox do not save it.

The token is bound to `user_id`, can be used once and expires after `reset_ttl_secs`. A GET does not consume it so that a mail scanner does not devalue the link. He creates `_ferrum_password_reset_exchanges`. The exchange secret is also 32 bytes, stored only as a hash, and lives at most ten minutes, never longer than the reset. The form then sends the cookie, not the mail token.

A maximum of three unused, unexpired tokens remain per user. A new request consumes older excess tokens in the same transaction. The rate limit is in the process, separate from the login: `reset-ip:{adresse}` with four times `login_max_attempts`, `reset-mail:{adresse}` with `login_max_attempts`, window `login_window_secs`. Every request counts, even an unknown address.

## Abschluss

`complete_reset` checks equality, `validate_new_password` and that the new password does not correspond to the current Argon2id hash. The new hash is created before `begin`. After that, a conditional `UPDATE` applies to `used_at IS NULL AND expires_at > now`. Only `rows_affected = 1` sets the password, revokes all sessions, consumes the remaining reset tokens, deletes the exchange lines and writes the audit. Otherwise it will roll back. Two parallel trades may be successful at most once. The audit uses model `_ferrum_users`, action `password_reset`, result `ok` and the summary `password reset completed`.

The reset URL is created from `public_origin`, never from the host header.

```rust
use ferrioxid::auth::{complete_reset, exchange_token, request_reset};
use ferrioxid::mail::MailService;

let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
request_reset(&auth, Some(&mail), "person@example.com", "203.0.113.10", "de").await;
let token = inbox.messages()[0].text.lines().find_map(|line| line.split("token=").nth(1)).unwrap().trim();
let opened = exchange_token(&auth, token).await?;
complete_reset(&auth, Some(&mail), &opened.secret, "replacement-horse-1", "replacement-horse-1", "de").await?;
```

The application registers the same process through `with_mail` and `with_auth`. A separate mount is not necessary.
