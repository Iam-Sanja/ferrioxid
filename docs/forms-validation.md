# Form validation

**Milestone:** v0.19, Implemented. The Cargo version remains `0.1.0`. Additional codes: `validation.date`, `validation.time`, `validation.datetime`, `validation.datetime_offset`.

## Responsibility

`fexu-core::validation` is the common rule kernel for form fields. Generated model checks now emit the same stable codes, with an optional numeric suffix, instead of English sentences.

## Crates and files

- `crates/fexu-core/src/validation.rs`
- Caller: `crates/fexu-forms/src/lib.rs`
- Display catalog: `crates/fexu-core/src/messages.rs`
- Model macro: `crates/fexu-macros/src/lib.rs`

## Public API

`text_code`, `parse_i64`, `parse_bool`, `parse_uuid`, `is_email` and `is_http_url` return stable codes rather than localized sentences. `fexu::forms::message` and `message_for` translate those codes. `FieldOverride` replaces one field and code. Placeholders such as `{min}` are replaced only from the supplied parameters. A parameter longer than 32 characters, or one that contains anything except ASCII letters, digits, space, `.`, `_` and `-`, is dropped. Unknown locales use English. An unknown code returns `missing message`.

## Data flow

```text
FexuModel-Makro → field:<name>: validation.<code>[:<zahl>]
fexu-forms → text_code / parse_* → Feldcode → message(locale, code)
HTML → localize_detail(locale, detail)
JSON → problem_json behält detail und setzt code plus params
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
fexu::core::text_code("a@b", true, None, Some(254), true, false);
```

## Tests

`fexu-core` checks the codes. `fexu-forms` checks the assignment to fields.

## Limitations

The model macro still duplicates the rules. International email addresses and free URL schemes are rejected.

## Extension

The macro can later call `text_code` without changing the public error texts of the REST API.
