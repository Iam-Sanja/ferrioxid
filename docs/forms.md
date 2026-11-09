# Forms

**Milestone:** v0.19, with widget and model-binding additions in review. The Cargo version remains `0.1.0`. `required_date`, `optional_date`, `required_time`, `optional_time`, `required_datetime_utc` and `required_datetime_local` check the canonical strings. The local DateTime field is considered UTC.

## Responsibility

`ferrioxid-forms` checks URL-encoded input and provides a typed Rust object or field error. Forms are not bound to a database table. Contact, search and filter can thus be described without creating a `FerrioxidModel`.

## Crates and files

- `crates/ferrioxid-forms/src/lib.rs`
- Shared validation rules: `crates/ferrioxid-core/src/validation.rs`
- Example: `examples/myshop/src/contact.rs`
- Generator: `crates/ferrioxid-cli/src/scaffold.rs`, function `contact_handlers`

The facade exports the module as `ferrioxid::forms`.

## Public API

`FerrioxidForm::from_form` accepts `FormValues`; `ContactForm` is the supplied example. Helpers include `required_text`, `optional_text`, `required_i64`, `optional_i64`, `required_bool`, `optional_bool`, `required_uuid`, `optional_uuid`, `required_choice`, `optional_choice` and `required_postal`. `render_form` still accepts `InputSpec`. `render_fields` renders `Field` widgets: text, email, password, number, URL, checkbox, textarea and select. Labels, help, placeholders, the `required` attribute and field errors are escaped. `aria-describedby` points at `{name}-error` and `{name}-help` when those texts exist. A selected option is marked only when its value equals the current field value. Password values are not written back into the page. `message` maps stable codes to German and English text. `FormValues::parse_urlencoded` limits the body to 65536 bytes and rejects unknown fields through `reject_unknown`. `csrf_token` is allowed as a control field rather than form data.

`model_fields` builds fields from `ColumnSpec` and `AdminFieldMeta`, but only for names in the explicit allow-list. A name that is not a column, a duplicate, a primary key, or an admin field marked sensitive, readonly or hidden returns `FormSetupError` and creates no field. `bind_model` distinguishes `SubmitMode::Create`, `Replace` and `Patch`. Create and Replace require every required field and turn a missing optional field into `BoundValue::Null`. Patch keeps a missing key as `BoundValue::Unchanged`. Posted names outside the field list produce `validation.unknown_field`, including when a client adds a column the allow-list omitted. `choices` rejects values that are not in the supplied list with `validation.choice`.

A `#[derive(FerrioxidForm)]` is not implemented. The trait and `model_fields` are the compilable API. The HTML `required` attribute does not replace the server check.

## Data flow

The handler reads the body, builds `FormValues` and calls `from_form`. Valid values are typed. Errors remain codes per field. `render_form` escaped names, values and messages. The success page of the contact example does not save the message and does not send an email.

## Configuration

Forms do not have their own `ferrioxid.toml` section. The maximum URL-encoded size is in `FormValues::parse_urlencoded`. Larger requests are previously rejected by the body limit middleware if `ferrioxid::web::prepare` or `ferrioxid::middleware::apply` is used.

## Security

HTML output is escaped. Unknown fields return `validation.unknown_field` instead of being silently accepted. Validation runs on the server. A cookie-authenticated form submits the same HMAC token used by `x-csrf-token` in its `csrf_token` field. Public forms without a session do not add a separate CSRF mechanism. Obtain the token with `AuthService::csrf_from_request`.

## Error cases

Codes include `validation.required`, `validation.min_length`, `validation.max_length`, `validation.min_value`, `validation.max_value`, `validation.email`, `validation.url`, `validation.invalid_type`, `validation.uuid`, `validation.unknown_field`, `validation.choice`, `validation.postal`, `validation.mismatch` and `form.csrf`. `ContactForm` returns `validation.mismatch` when the name equals the local part of the email address. `topic` is optional and must be `order`, `product` or `other`. For unknown codes, `message` returns `missing message`. Setup codes are `form.unknown_column`, `form.duplicate_column` and `form.protected_column`.

## Example

```rust
let values = ferrioxid::forms::FormValues::parse_urlencoded(&body).expect("form body");
let form = ferrioxid::forms::ContactForm::from_form(&values).expect("contact");
let admin = Product::admin_meta().map(|meta| meta.fields).unwrap_or(&[]);
let fields = ferrioxid::forms::model_fields(Product::columns(), admin, &["title"]).expect("fields");
let bound = ferrioxid::forms::bind_model(
    ferrioxid::forms::SubmitMode::Replace,
    &fields,
    &values,
    &[],
)
.expect("bound");
let _checked = &form.name;
let _title = &bound[0].value;
```

## Tests

`cargo test -p ferrioxid-forms`. Covered are mandatory fields, length, email, URL, number, UUID, optional fields, cross validation, unknown fields, escaping, select choices, postal codes, prefilled values, password suppression and the model allow-list.

## Limitations

There is no form set, no file fields in the form API, and no proc macro. `model_fields` does not read per-column min/max attributes, because those are not part of `ColumnSpec`. Numeric ranges stay on `required_i64`. The model macro does not yet call the common core. Formsets remain absent.

## Extension

A future derive macro can call the same functions from `ferrioxid-core`. File fields and formsets remain planned.
