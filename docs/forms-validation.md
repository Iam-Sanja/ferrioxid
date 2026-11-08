# Formularvalidierung

**Stand:** v0.19, IST. The cargo version remains `0.1.0`. Additional codes: `validation.date`, `validation.time`, `validation.datetime`, `validation.datetime_offset`.

## Verantwortlichkeit

`ferrioxid-core::validation` is the common rule kernel for form fields. `FerrioxidModel` retains the previously generated checks so that existing CRUD responses remain unchanged.

## Crates and files

- `crates/ferrioxid-core/src/validation.rs`
- Aufrufer: `crates/ferrioxid-forms/src/lib.rs`
- Modellmakro, unverändert: `crates/ferrioxid-macros/src/lib.rs`

## Public API

`text_code`, `parse_i64`, `parse_bool`, `parse_uuid`, `is_email` and `is_http_url`. Returns are stable codes, not translated sentences. Translation makes `ferrioxid::forms::message`.

## Datenfluss

```text
FerrioxidModel-Makro → eigene Meldungen (unverändert)
ferrioxid-forms → text_code / parse_* → Feldcode → message(locale, code)
```

Optional text fields treat an empty value as `None` before the mandatory rule takes effect. Numbers and UUIDs without value remain `validation.required` if the Required help is used.

## Konfiguration

None. Boundaries are at the call point, for example `TextSpec.min_length`.

## Sicherheit

Rules only see the already decoded field value. They do not run SQL or shell commands. Email and URL checks are deliberately narrow: ASCII, a `@`, a domain with point, `http` or `https`.

## Error cases

| Code | Bedeutung |
|---|---|
| `validation.required` | leerer Pflichtwert |
| `validation.min_length` | zu kurz |
| `validation.max_length` | zu lang |
| `validation.min_value` | Zahl zu klein |
| `validation.max_value` |Number too large|
| `validation.email` |Not a simple email|
| `validation.url` |No `http` or `https` URL|
| `validation.invalid_type` |No number or bool|
| `validation.uuid` | keine UUID |

## Example

```rust
ferrioxid::core::text_code("a@b", true, None, Some(254), true, false);
```

## Tests

`ferrioxid-core` checks the codes. `ferrioxid-forms` checks the assignment to fields.

## Limitations

The model macro still duplicates the rules. International email addresses and free URL schemes are rejected.

## Erweiterung

The macro can later call `text_code` without changing the public error texts of the REST API.
