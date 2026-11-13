# Authentication providers

[Overview](index.md) · Threat model: [auth-threat-model.md](auth-threat-model.md)

**Implemented** for the local password provider, TOTP, recovery codes, session revoke, reauthentication and account close. OIDC, OAuth and passkeys are not implemented.

## Provider

`IdentityProvider` verifies an email and a secret against the database and returns a `VerifiedIdentity`. It does not create a session. `PasswordProvider` is the only provider. It keeps the dummy-hash path for unknown emails. A provider is trusted in-process code. It is not loaded from a request or from a database row.

`RemoteProtocol` lists `Oidc`, `OAuthAuthorizationCode` and `Passkey`. `is_implemented` is always `false`. There is no method that reports a remote login as success.

## Second factor

`begin_totp` stores a 20-byte secret with AES-256-GCM. The key is `HMAC-SHA256(csrf_secret, "fexu-mfa-v1")`. The user id is additional authenticated data. `confirm_totp` checks a 6-digit RFC 6238 SHA-1 code in a window of one step and returns eight recovery codes once. Codes look like `XXXXX-XXXXX` and avoid ambiguous characters. Only an HMAC is stored. Each code works once.

Unconfirmed enrollment does not change login. After confirmation, `login` and `login_replacing` return `AuthError::MfaPending` and do not set a session cookie or revoke the anonymous session. `POST /auth/login` answers 401 with `auth.mfa_required`, `challenge` and `expires_at`. The challenge lives for five minutes and allows five attempts. `POST /auth/mfa/complete` is the only route that turns a challenge into a session. The challenge is not a session token. `Debug` redacts it.

Disabling TOTP requires the password and a current code or recovery code, then revokes every session. Closing an account is refused while TOTP is still enabled.

## Sessions and account life

`list_sessions` returns id and timestamps, not token hashes. `revoke_session_for` revokes the actor's own session. Another user's session is `Forbidden` unless the actor is a superuser. Staff is not enough. `revoke_other_sessions` keeps the current session.

`reauthenticate` returns a proof `v1.{expires}.{mac}` bound to the user id and the session id, valid for at most 300 seconds. It is not consumed. A second session cannot reuse it. `require_reauth` checks that binding.

`close_account` checks the password, sets the user inactive, revokes sessions and deletes TOTP, recovery codes and login challenges. The user row remains, so the email stays reserved. It does not delete stored files. The application calls storage `purge_owner`.

Authenticated POSTs still require the session cookie and CSRF header. Login and `POST /auth/mfa/complete` use the existing origin check and do not accept a session cookie as proof of the second factor.

## Routes

| Route | Body | Result |
|---|---|---|
| `POST /auth/mfa/enroll` | `password` | Base32 secret and `otpauth` URI |
| `POST /auth/mfa/confirm` | `code` | Recovery codes, shown once |
| `POST /auth/mfa/disable` | `password`, `code` | TOTP removed, sessions revoked |
| `POST /auth/mfa/complete` | `challenge`, `code` | Session cookie |
| `POST /auth/sessions/revoke` | `session_id` | 204 or 403 |
| `POST /auth/reauthenticate` | `password`, optional `code` | Short-lived proof |
| `POST /auth/account/close` | `password` | User inactive |

Rotating `csrf_secret` makes stored TOTP secrets and reauthentication proofs unreadable. Plan that rotation before changing the key.
