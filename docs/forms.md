# Forms

**Current package version:** `0.1.0` (experimental). Widget and model-binding additions are on `main`; historical `v0.19` is an internal milestone, not a registry version. `required_date`, `optional_date`, `required_time`, `optional_time`, `required_datetime_utc` and `required_datetime_local` check the canonical strings. The local DateTime field is considered UTC.

## Responsibility

`fexu-forms` checks URL-encoded input and provides a typed Rust object or field error. Forms are not bound to a database table. Contact, search and filter can thus be described without creating a `FexuModel`.

## Crates and files

- `crates/fexu-forms/src/lib.rs`
- Shared validation rules: `crates/fexu-core/src/validation.rs`
- Example: `examples/myshop/src/contact.rs`
- Generator: `crates/fexu-cli/src/scaffold.rs`, function `contact_handlers`

The facade exports the module as `fexu::forms`.

## Public API

`FexuForm::from_form` accepts `FormValues`; `ContactForm` is the supplied example. Helpers include `required_text`, `optional_text`, `required_i64`, `optional_i64`, `required_bool`, `optional_bool`, `required_uuid`, `optional_uuid`, `required_choice`, `optional_choice` and `required_postal`. `render_form` still accepts `InputSpec`. `render_fields` renders `Field` widgets: text, email, password, number, URL, checkbox, textarea and select. Labels, help, placeholders, the `required` attribute and field errors are escaped. `aria-describedby` points at `{name}-error` and `{name}-help` when those texts exist. A selected option is marked only when its value equals the current field value. Password values are not written back into the page. `message` maps stable codes to German and English text. `FormValues::parse_urlencoded` limits the body to 65536 bytes and rejects unknown fields through `reject_unknown`. `csrf_token` is allowed as a control field rather than form data.

`model_fields` builds fields from `ColumnSpec` and `AdminFieldMeta`, but only for names in the explicit allow-list. A name that is not a column, a duplicate, a primary key, or an admin field marked sensitive, readonly or hidden returns `FormSetupError` and creates no field. `bind_model` distinguishes `SubmitMode::Create`, `Replace` and `Patch`. Create and Replace require every required field and turn a missing optional field into `BoundValue::Null`. Patch keeps a missing key as `BoundValue::Unchanged`. Posted names outside the field list produce `validation.unknown_field`, including when a client adds a column the allow-list omitted. `choices` rejects values that are not in the supplied list with `validation.choice`.

`FormValues::parse_urlencoded` and `FormValues::parse_json` both feed `bind_submission`. Duplicate keys are `validation.duplicate`. Names in the protected list are `validation.readonly` and are removed before binding, so a forged readonly or disabled field cannot overwrite the stored value. `apply_contracts` copies `min_length`, `max_length` and `format` from `FieldContract`. Lengths count Unicode scalar values. `render_relation_select` renders only the options the caller already limited with `QueryScope`. `inspect_upload` rejects traversal, size, a content type outside the policy, and image bytes that do not match PNG, JPEG, GIF or WebP. `plan_formset` counts creates, updates and deletes. The caller writes those rows in one database transaction and rolls the transaction back when validation fails.

`#[derive(FexuForm)]` with `#[form(model = Type)]` builds a form from the model allow-list. `bind_formset` accepts at most 100 rows named `{prefix}_{index}_{field}`. `store_upload` rejects `..`, slashes and NUL, then stores the bytes through `LocalStorage`. Numeric `min` and `max` come from the model column metadata. The HTML `required` attribute does not replace the server check.

## Data flow

The handler reads the body, builds `FormValues` and calls `from_form`. Valid values are typed. Errors remain codes per field. `render_form` escaped names, values and messages. The success page of the contact example does not save the message and does not send an email.

## Configuration

Forms do not have their own `fexu.toml` section. The maximum URL-encoded size is in `FormValues::parse_urlencoded`. Larger requests are previously rejected by the body limit middleware if `fexu::web::prepare` or `fexu::middleware::apply` is used.

## Security

HTML output is escaped. Unknown fields return `validation.unknown_field` instead of being silently accepted. Validation runs on the server. A cookie-authenticated form submits the same HMAC token used by `x-csrf-token` in its `csrf_token` field. Obtain a user token with `AuthService::csrf_from_request`. A public form uses `ensure_anonymous_session` and `authenticate_browser_form`. myshop `/contact` does that when an `AuthService` is configured. Without an auth service every contact POST is rejected and no session is created. Generated public JSON routes are covered by the app-router cookie check: a session cookie still needs CSRF, and a cookie-less request stays public.

## Error cases

Codes include `validation.required`, `validation.min_length`, `validation.max_length`, `validation.min_value`, `validation.max_value`, `validation.email`, `validation.url`, `validation.invalid_type`, `validation.uuid`, `validation.unknown_field`, `validation.choice`, `validation.postal`, `validation.mismatch` and `form.csrf`. `ContactForm` returns `validation.mismatch` when the name equals the local part of the email address. `topic` is optional and must be `order`, `product` or `other`. For unknown codes, `message` returns `missing message`. Setup codes are `form.unknown_column`, `form.duplicate_column` and `form.protected_column`.

`required_column_choice`, `choice_options` and `render_choice_select` build a select from `ColumnSpec::choices`. `required_choice` still checks a caller-supplied option list. `required_decimal` rejects values that do not fit the declared precision and scale. Labels follow `de` or any other locale as English. Unknown choices use `validation.choice` and bad decimals use `validation.decimal`. See [models.md](models.md).

## Example

```rust
let values = fexu::forms::FormValues::parse_urlencoded(&body).expect("form body");
let form = fexu::forms::ContactForm::from_form(&values).expect("contact");
let admin = Product::admin_meta().map(|meta| meta.fields).unwrap_or(&[]);
let fields = fexu::forms::model_fields(Product::columns(), admin, &["title"]).expect("fields");
let bound = fexu::forms::bind_model(
    fexu::forms::SubmitMode::Replace,
    &fields,
    &values,
    &[],
)
.expect("bound");
let _checked = &form.name;
let _title = &bound[0].value;
```

## Tests

`cargo test -p fexu-forms`. Covered are mandatory fields, length, email, URL, number, UUID, optional fields, cross validation, unknown fields, escaping, select choices, postal codes, prefilled values, password suppression and the model allow-list.

## Limitations

File inputs use `enctype="multipart/form-data"`. The stored path still comes from `store_upload`, which ignores the client file name. A formset plan does not open a transaction by itself. Remote storage is not included. `parse_json` accepts only a flat object of strings, numbers, booleans and null.

## Extension

A future derive macro can call the same functions from `fexu-core`. File fields and formsets remain planned.
