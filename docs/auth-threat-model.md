# Authentication threat model

[Overview](index.md) · Providers: [auth-providers.md](auth-providers.md)

This page records attacks the current code is built to resist, and the protocols that are intentionally absent.

## In scope

| Threat | Control |
|---|---|
| Password guessing | Argon2id, dummy hash for unknown users, five failures per process and client key in 15 minutes |
| Session fixation | Login creates a new session. The anonymous session is revoked only when that session is issued, not when MFA is still pending |
| Stolen database | Session tokens and recovery codes are keyed hashes. TOTP secrets are AES-256-GCM with `FEXU_MFA_KEY` and the user id as AAD. A database copy without that key does not reveal the authenticator secret. Recovery codes are HMAC hashes, not ciphertext |
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

`FEXU_MFA_KEY` is 32 bytes, encoded as 64 hexadecimal characters with no prefix. Those bytes are the AES-256-GCM key. There is no `FEXU_MFA_KEY` alias. Production refuses to start when the variable is missing or not 64 hex characters, and the error does not include the value. Development reads `.fexu/mfa.key` as 32 raw bytes, or creates that file once. `AuthConfig::development` draws a process-local random MFA key. It is not the CSRF development key and it is not a published constant.

New TOTP rows store the nonce as `v2.` plus 24 hex characters. New recovery records store `v2.` plus a 64-hex HMAC-SHA256 over the label `fexu-recovery-v2`, the user id and the normalized code. A legacy nonce is exactly 24 hex characters and opens only with HMAC-SHA256 of the CSRF secret and the label `fexu-mfa-v1`. A legacy recovery record is exactly 64 hex characters and uses the label `fexu-recovery-v1` with that same derived key. A password hash, including an Argon2 PHC string, matches neither format and is not decrypted.

After a legacy TOTP secret decrypts, Fexu seals it again with `FEXU_MFA_KEY` and writes the `v2.` nonce. Confirmed state and the last accepted step stay in place. Recovery hashes are not rewritten. The plaintext code is not stored, so replacing the hash would either invent a new code or destroy the existing one. A legacy code still verifies once with the original CSRF secret and is then consumed. Unused legacy codes remain valid only while that CSRF secret is unchanged. Rotating `FEXU_CSRF_SECRET` after the TOTP row has moved to `v2.` does not make the authenticator unreadable. It does invalidate unused legacy recovery hashes. New `v2.` recovery codes and `v2.` reauthentication proofs do not depend on the CSRF secret. A `v1.` reauthentication proof still checks the CSRF-derived key and therefore fails after that rotation.

Deployments that already confirmed TOTP should keep the current CSRF secret until every unused legacy recovery code has been used. Do not generate a replacement set automatically. Do not add another advisory ignore for this residual.

Account close does not remove files. An application that skips `purge_owner` leaves the owner's bytes on disk.

Header checks on uploads are not a malware scanner. See [storage-remote.md](storage-remote.md).
