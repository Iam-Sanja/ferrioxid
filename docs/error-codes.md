# Fehlercodes

[For overview](index.md) · **IST file:** `crates/ferrioxid-core/src/problem.rs`.]

## Vertrag

`problem_json` builds the answer for API, Auth and Admin:

```json
{
  "type": "about:blank",
  "code": "validation.required",
  "title": "Validation failed",
  "status": 422,
  "detail": "field:name: Field is required",
  "field": "name",
  "params": {}
}
```

`field` is missing if there is no `field:<name>:` prefix. `name` shall contain only letters, numbers and `_` and shall not exceed 64 characters in length. `detail` is retained for existing clients. The status codes are unchanged.

## Codes

| Code | Bisheriger `detail` |
|---|---|
| `validation.required` | `Field is required`, `active is required`, `email and role are required` |
| `validation.password` | `Password must contain 12 to 128 characters` |
| `validation.field` |Other `field:<name>:` prefix|
| `auth.invalid_credentials` | `Invalid email or password` |
| `auth.required` | `Authentication required` |
| `auth.forbidden` | `Access denied` |
| `auth.csrf` | `csrf token is missing or invalid` |
| `auth.origin` | `origin is not allowed` |
| `auth.not_configured` | `authentication is not configured` |
| `auth.rate_limited` | `Too many login attempts` |
| `auth.rejected` | `request rejected` |
| `resource.not_found` | `Resource not found`, `user was not found` |
| `database.failed` | `Database operation failed` |
| `server.error` | `Internal server error` |
| `policy.unbound` |Unbound object rule|
| `policy.create_rejected` | `create policy rejected the record` |
| `list.sort` | `sort field is not allowed` |
| `list.filter` | `filter field is not allowed` |
| `list.search` | `search is not available` |
| `list.relation` |blocked foreign key selection|
| `directory.self_deactivate` |Deactivate your own account|
| `directory.self_staff` |Remove own staff access|
| `directory.superuser_blocked` | `superuser changes are blocked` |
| `request.rejected` |suspicious or too long text, replaced by `Request rejected`|
| `request.failed` |Other short text|
| `validation.min_length` |Form value too short|
| `validation.max_length` |Form value too long|
| `validation.email` | Formular-E-Mail ungültig |
| `validation.url` | Formular-URL ungültig |
| `validation.invalid_type` |Form number or Bool invalid|
| `validation.uuid` |Form UUID invalid|
| `validation.unknown_field` |Additional form field|
| `validation.mismatch` | Kreuzfeldregel |
| `upload.too_large` |Upload above limit, status 413|
| `upload.type` |Content not allowed, status 415|
| `resource.not_found` |own file missing or belongs to someone else, status 404|

Unknown users and incorrect passwords generate the same code and the same `detail`. Texts with `SELECT`, `INSERT`, `UPDATE`, `DELETE FROM`, `password_hash` or `stack backtrace` as well as texts over 240 characters become `request.rejected`.

## Frontend

`translateError` is looking for `errors.<code>`. If the translation is missing, `detail` will be shown unless it looks like SQL or hash. Otherwise, `errors.unknown` applies. Field errors remain on the input field even if the old `field:<name>:` prefix is still in the `detail`.

## Tests

`crates/ferrioxid-core/src/problem.rs` checks the same login error, field prefix and suppressed SQL. The existing Auth and Admin tests continue to read `detail`.
