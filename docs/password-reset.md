# Password reset

[Overview](index.md) · Auth: [auth.md](auth.md) · Email: [mail.md](mail.md) · Security: [mail-security.md](mail-security.md)

**Milestone:** v0.23. The Cargo version remains `0.1.0`. The migration is `crates/fexu-auth/migrations/0004_password_reset.json`. `0001`, `0002` and `0003` remain unchanged.

## Routes

`App::with_auth` attaches the pages. They do not require Staff status.

| Method | Path | Result |
|---|---|---|
| `GET` | `/auth/password-reset` | Form and `fexu_reset_csrf` cookie |
| `POST` | `/auth/password-reset` | Identical acknowledgement for known and unknown addresses |
| `GET` | `/auth/password-reset/confirm?token=` |Exchanges a valid token for cookie `fexu_reset` and responds 303 to `/auth/password-reset/choose`|
| `GET` | `/auth/password-reset/choose` |Password and repetition|
| `POST` | `/auth/password-reset/choose` |Sets the password or shows a visible error|

The public acknowledgement is “If an account with this address exists, an email with further steps has been requested.” Unknown, inactive or invalid addresses, rate limits, disabled mail and delivery failures do not reveal account existence. Inactive users receive no email. CSRF and origin failures return 403 without naming the address. Without an auth service, POST returns 404. Bodies over 8 KiB are rejected with 403.

The pages are server-rendered HTML without external file and without JavaScript. `Referrer-Policy` is `no-referrer`. The content security policy does not allow an external asset. The admin login remains unchanged and does not contain an additional link.

## Token

32 bytes from `getrandom`, encoded as Base64 URL without padding. That's 256 bits. Only SHA-256-Hex is stored in `_fexu_password_resets`. The raw value is in the process memory and in the e-mail. Audit, session log and the outbox do not save it.

The reset token belongs to `user_id`, is single-use and expires after `reset_ttl_secs`. A GET does not consume it, so an email scanner cannot invalidate the link. Instead, it creates an entry in `_fexu_password_reset_exchanges`. The 32-byte exchange secret is stored only as a hash and expires after at most ten minutes, never later than the reset token. The form submits the exchange cookie rather than the email token.

A maximum of three unused, unexpired tokens remain per user. A new request consumes older excess tokens in the same transaction. The rate limit is in the process, separate from the login: `reset-ip:{adresse}` with four times `login_max_attempts`, `reset-mail:{adresse}` with `login_max_attempts`, window `login_window_secs`. Every request counts, even an unknown address.

## Completion

`complete_reset` checks password confirmation, `validate_new_password` and whether the new password differs from the current Argon2id hash. It creates the hash before `begin`, then conditionally updates the reset row with `used_at IS NULL AND expires_at > now`. Only `rows_affected = 1` permits the password change, session revocation, consumption of remaining tokens, deletion of exchange rows and audit entry; otherwise, the transaction rolls back. Concurrent attempts allow at most one success. The audit uses model `_fexu_users`, action `password_reset`, outcome `ok` and summary `password reset completed`.

The reset URL is created from `public_origin`, never from the host header.

```rust
use fexu::auth::{complete_reset, exchange_token, request_reset};
use fexu::mail::MailService;

let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
request_reset(&auth, Some(&mail), "person@example.com", "203.0.113.10", "de").await;
let token = inbox.messages()[0].text.lines().find_map(|line| line.split("token=").nth(1)).unwrap().trim();
let opened = exchange_token(&auth, token).await?;
complete_reset(&auth, Some(&mail), &opened.secret, "replacement-horse-1", "replacement-horse-1", "de").await?;
```

The application registers the same process through `with_mail` and `with_auth`. A separate mount is not necessary.
