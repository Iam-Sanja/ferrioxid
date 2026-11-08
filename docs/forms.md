# Formulare

**Stand:** v0.19, IST. The cargo version remains `0.1.0`. `required_date`, `optional_date`, `required_time`, `optional_time`, `required_datetime_utc` and `required_datetime_local` check the canonical strings. The local DateTime field is considered UTC.

## Verantwortlichkeit

`ferrioxid-forms` checks URL-encoded input and provides a typed Rust object or field error. Forms are not bound to a database table. Contact, search and filter can thus be described without creating a `FerrioxidModel`.

## Crates and files

- `crates/ferrioxid-forms/src/lib.rs`
- Gemeinsame Regeln: `crates/ferrioxid-core/src/validation.rs`
- Example: `examples/myshop/src/contact.rs`
- Generator: `crates/ferrioxid-cli/src/scaffold.rs`, function `contact_handlers`

The facade exports the module as `ferrioxid::forms`.

## Public API

`FerrioxidForm::from_form` nimmt `FormValues` entgegen. `ContactForm` ist die mitgelieferte Referenz. Hilfen heißen `required_text`, `optional_text`, `required_i64`, `optional_i64`, `required_bool`, `optional_bool`, `required_uuid` und `optional_uuid`. `render_form` und `render_page` erzeugen HTML. `message` liefert deutsche und englische Texte zu stabilen Codes. `FormValues::parse_urlencoded` begrenzt den Body auf 65536 Byte und lehnt unbekannte Felder über `reject_unknown` ab. `csrf_token` ist erlaubt und kein Datenfeld.

A `#[derive(FerrioxidForm)]` is not implemented. The Trait form is the compilable API.

## Datenfluss

The trader reads the body, builds `FormValues` and calls `from_form`. Valid values are typed. Errors remain codes per field. `render_form` escaped names, values and messages. The success page of the contact example does not save the message and does not send an email.

## Konfiguration

Forms do not have their own `ferrum.toml` section. The maximum URL-encoded size is in `FormValues::parse_urlencoded`. Larger requests are previously rejected by the body limit middleware if `ferrioxid::web::prepare` or `ferrioxid::middleware::apply` is used.

## Sicherheit

HTML is escaped. Unknown fields result in `validation.unknown_field` and are not adopted silently. The test runs on the server. A session that sends a CSRF cookie must send the same HMAC token as `x-csrf-token` in the `csrf_token` field. Without a meeting, a public form is left without a second CSRF mechanism. The token comes from `AuthService::csrf_from_request`.

## Error cases

`validation.required`, `validation.min_length`, `validation.max_length`, `validation.min_value`, `validation.max_value`, `validation.email`, `validation.url`, `validation.invalid_type`, `validation.uuid`, `validation.unknown_field`, `validation.mismatch`, `form.csrf`. `ContactForm` setzt `validation.mismatch`, wenn der Name dem lokalen Teil der E-Mail entspricht. Unbekannte Codes ergeben in `message` den Text `missing message`.

## Example

```rust
let values = ferrioxid::forms::FormValues::parse_urlencoded(&body)?;
let form = ferrioxid::forms::ContactForm::from_form(&values)?;
let _checked = form.name;
```

## Tests

`cargo test -p ferrioxid-forms`. Covered are mandatory fields, length, email, URL, number, UUID, optional fields, cross validation, unknown fields and escaping.

## Limitations

There is no form set, no file fields in the form API, and no proc macro. The model macro does not yet call the common core.

## Erweiterung

A later derivative can call the same functions from `ferrioxid-core`. File and Formset fields remain planned.
