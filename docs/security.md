# Security – Threat Model and Security Borders

[For overview](index.md) · **IST + REQUIRED.** **Ferrioxide v0.1 is not ready for production.] ***

## ACTual protection mechanisms

- Without `allow_anonymous = true`, GET, POST, PATCH and DELETE are registered as 403. The JSON body is the same `ApiError` contract.
- Create and update validation checks known fields according to macro rules. `deny_unknown_fields` prevents `id` or `readonly` fields from arriving in JSON.
- INSERT, UPDATE and DELETE bind values via SeaORM. The DELETE and UPDATE statements always contain `WHERE "id" = $1`. Table and column names come only from checked macro attributes.
- `ApiError` logs DB details internally and delivers only generic information to the outside world at 500. Since v0.11, the same answer has a stable `code`. SQL and hashes are also replaced if a free text contained them.
- Axum’s JSON-Extractor uses Serde DTOs with `deny_unknown_fields` on the Create type as standard.
- Demo bind is `127.0.0.1:3000`; Docker Postgres port is published on `127.0.0.1`.
- `can_create` checks the Create input before the INSERT. A foreign owner ID is rejected if the rule requires it. The standard of the method is `false`. Generated projects do not bind to `0.0.0.0` and do not contain a standard password.
- Since v0.12, static files remain in the configured root. Uploads use UUID names, check the file contents and belong to the logged-in user. The request ID comes from the server. Logs do not contain a query string. Details are available in [staticfiles.md](staticfiles.md), [uploads.md](uploads.md) and [middleware.md](middleware.md)].

## Authentication (actual, v0.7)

Users, Argon2id, server-side sessions, HttpOnly cookies, CSRF for cookie write accesses, roles and model rights are implemented. Details are available in [auth.md](auth.md), [sessions.md](sessions.md), [permissions.md](permissions.md) and [policies.md](policies.md)].

`allow_anonymous = true` remains an explicit public release. Product and category in myshop are such demo endpoints. Protected routes refuse without session and right.

## Remaining gaps (actual)

No OAuth, no MFA, no password reset and no JWT for external APIs. The login rate limit is per process. Filter object rules via [query-scopes.md](query-scopes.md)]. Without an explicit scope, the list does not provide a line. `allow_anonymous` does not check CSRF because the access does not depend on the cookie. Parallel PATCH statements do not have a version field. Ferrioxide is thus not described as a public Internet service without additional proxy and operational limits.

## SOLL-Bedrohungsmodell

| Bedrohung |Target action| Nachweis |
|---|---|---|
|Unauthorized Reads/Writes| Deny by default + serverseitige Policies | 401/403-Tests |
| IDOR |Object policy **after** Identity verification for each individual ID| Querzugriffstests |
| Mass Assignment | geteilte DB-Models nie blind als Writable DTO verwenden |Field masking tests|
| CSRF |Protection for cookie-based state changes| Cross-Origin-Tests |
| SQL Injection |bound values, identifier-allowlist| fuzzer-/negativ-Tests |
| Brute Force |Rate limit, lockout by risk, auditing| Login-Stresstests |
| Session-Diebstahl | HttpOnly/Secure/SameSite, Rotation, Ablauf | Session-Tests |
| Secrets in Logs |Redaction, generic API error| Log-Audit |
|Public Demo|Bind and configuration checks, no Prod default| Deployment-Test |

## Implementation targets (reference)

- `auth` and policies are separate concepts: authentication determines identity, authorization decides access.
- RBAC alone is not sufficient for object-level/field-level policies.
- By default, no anonymous CRUD and no automatically exported sensitive fields.
- The interface may only display permissions. The server verification remains relevant.
- Auth context must not be created via client fields or headers with an unchecked trust chain.
- Before each productive release security review, dependency audit and documented threat model assumptions.

**Note:** `SECURITY.md` in the repository currently contains only a brief project information, not an established vulnerability disclosure process. It must be maintained for public releases.

## Admin

The admin checks session, active user, staff, model right, object rule and write access CSRF. Path parameters are not unchecked to SQL identifiers. Sorting, searching and filtering are whitelists. Details and known limits are given in [admin-security.md](admin-security.md)].
