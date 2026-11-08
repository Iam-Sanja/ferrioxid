# Forms

**Milestone:** v0.19, Implemented. The Cargo version remains `0.1.0`. `required_date`, `optional_date`, `required_time`, `optional_time`, `required_datetime_utc` and `required_datetime_local` check the canonical strings. The local DateTime field is considered UTC.

## Responsibility

`ferrioxid-forms` checks URL-encoded input and provides a typed Rust object or field error. Forms are not bound to a database table. Contact, search and filter can thus be described without creating a `FerrioxidModel`.

## Crates and files

- `crates/ferrioxid-forms/src/lib.rs`
- Shared validation rules: `crates/ferrioxid-core/src/validation.rs`
- Example: `examples/myshop/src/contact.rs`
- Generator: `crates/ferrioxid-cli/src/scaffold.rs`, function `contact_handlers`

The facade exports the module as `ferrioxid::forms`.

## Public API

`FerrioxidForm::from_form` accepts `FormValues`; `ContactForm` is the supplied example. Helpers include `required_text`, `optional_text`, `required_i64`, `optional_i64`, `required_bool`, `optional_bool`, `required_uuid` and `optional_uuid`. `render_form` and `render_page` generate HTML. `message` maps stable codes to German and English text. `FormValues::parse_urlencoded` limits the body to 65536 bytes and rejects unknown fields through `reject_unknown`. `csrf_token` is allowed as a control field rather than form data.

A `#[derive(FerrioxidForm)]` is not implemented. The Trait form is the compilable API.

## Data flow

The handler reads the body, builds `FormValues` and calls `from_form`. Valid values are typed. Errors remain codes per field. `render_form` escaped names, values and messages. The success page of the contact example does not save the message and does not send an email.

## Configuration

Forms do not have their own `ferrioxid.toml` section. The maximum URL-encoded size is in `FormValues::parse_urlencoded`. Larger requests are previously rejected by the body limit middleware if `ferrioxid::web::prepare` or `ferrioxid::middleware::apply` is used.

## Security

HTML output is escaped. Unknown fields return `validation.unknown_field` instead of being silently accepted. Validation runs on the server. A cookie-authenticated form submits the same HMAC token used by `x-csrf-token` in its `csrf_token` field. Public forms without a session do not add a separate CSRF mechanism. Obtain the token with `AuthService::csrf_from_request`.

## Error cases

Codes include `validation.required`, `validation.min_length`, `validation.max_length`, `validation.min_value`, `validation.max_value`, `validation.email`, `validation.url`, `validation.invalid_type`, `validation.uuid`, `validation.unknown_field`, `validation.mismatch` and `form.csrf`. `ContactForm` returns `validation.mismatch` when the name equals the local part of the email address. For unknown codes, `message` returns `missing message`.

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

## Extension

A future derive macro can call the same functions from `ferrioxid-core`. File fields and formsets remain planned.
