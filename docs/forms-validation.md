# Form validation

**Milestone:** v0.19, Implemented. The Cargo version remains `0.1.0`. Additional codes: `validation.date`, `validation.time`, `validation.datetime`, `validation.datetime_offset`.

## Responsibility

`ferrioxid-core::validation` is the common rule kernel for form fields. `FerrioxidModel` retains the previously generated checks so that existing CRUD responses remain unchanged.

## Crates and files

- `crates/ferrioxid-core/src/validation.rs`
- Caller: `crates/ferrioxid-forms/src/lib.rs`
- Model macro, unchanged: `crates/ferrioxid-macros/src/lib.rs`

## Public API

`text_code`, `parse_i64`, `parse_bool`, `parse_uuid`, `is_email` and `is_http_url` return stable codes rather than localized sentences. `ferrioxid::forms::message` translates those codes.

## Data flow

```text
FerrioxidModel-Makro → eigene Meldungen (unverändert)
ferrioxid-forms → text_code / parse_* → Feldcode → message(locale, code)
```

Optional text fields convert empty input to `None` before required-field validation. Missing numbers and UUIDs produce `validation.required` when the required-field helper is used.

## Configuration

None. Boundaries are at the call point, for example `TextSpec.min_length`.

## Security

Rules only see the already decoded field value. They do not run SQL or shell commands. Email and URL checks are deliberately narrow: ASCII, a `@`, a domain with point, `http` or `https`.

## Error cases

| Code | Meaning |
|---|---|
| `validation.required` | Required value is empty |
| `validation.min_length` | Too short |
| `validation.max_length` | Too long |
| `validation.min_value` | Number is too small |
| `validation.max_value` |Number too large|
| `validation.email` |Not a simple email|
| `validation.url` |No `http` or `https` URL|
| `validation.invalid_type` |No number or bool|
| `validation.uuid` | Invalid UUID |

## Example

```rust
ferrioxid::core::text_code("a@b", true, None, Some(254), true, false);
```

## Tests

`ferrioxid-core` checks the codes. `ferrioxid-forms` checks the assignment to fields.

## Limitations

The model macro still duplicates the rules. International email addresses and free URL schemes are rejected.

## Extension

The macro can later call `text_code` without changing the public error texts of the REST API.
