# Authentication threat model

[Overview](index.md) · Providers: [auth-providers.md](auth-providers.md)

This page records attacks the current code is built to resist, and the protocols that are intentionally absent.

## In scope

| Threat | Control |
|---|---|
| Password guessing | Argon2id, dummy hash for unknown users, five failures per process and client key in 15 minutes |
| Session fixation | Login creates a new session. The anonymous session is revoked only when that session is issued, not when MFA is still pending |
| Stolen database | Session tokens and recovery codes are keyed hashes. TOTP secrets are AES-GCM with the server key and the user id as AAD. A database copy without the server key does not reveal recovery codes |
| TOTP replay | The accepted step is stored. The same step is rejected. The window is the previous, current and next 30-second step |
| Challenge as session | `authenticate_token` rejects the challenge. HTTP login sets no cookie while MFA is pending |
| Cross-user session revoke | A non-superuser receives `Forbidden`. Staff does not bypass this |
| Reauthentication replay on another session | The proof includes the session id. Another session fails closed |
| Privilege through staff | `authorize` still requires the permission code or a stored superuser flag |

## Out of scope until a real implementation exists

OIDC, OAuth authorization-code and passkeys are not implemented. Trusting an email claim, an unverified `id_token`, or a client-supplied user id would be an account takeover. `RemoteProtocol::is_implemented` stays false so callers cannot treat a stub as success.

A future OIDC integration would need an authorization-code flow with PKCE, a fixed issuer and audience, nonce and state, JWKS with key rotation, and a local account link that is not "any verified email". A future passkey integration would need a WebAuthn challenge bound to the session, origin and rpId, and stored public keys. Those designs are not code in this crate.

## Residual risk

The login rate limit and the MFA attempt limit live in process memory. A restart clears them. The database also counts challenge attempts, so one challenge still stops after five tries.

The reauthentication proof can be replayed on the same session until it expires. Password change still revokes sessions.

Account close does not remove files. An application that skips `purge_owner` leaves the owner's bytes on disk.

Header checks on uploads are not a malware scanner. See [storage-remote.md](storage-remote.md).
