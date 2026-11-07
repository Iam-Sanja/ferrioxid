//! Typisierte Formulare. Sie sind nicht an eine Datenbanktabelle gebunden.
//!
//! Feldregeln kommen aus `ferrum_core`. HTML wird escaped. Unbekannte Felder werden abgelehnt.

use ferrum_core::{parse_bool, parse_i64, parse_uuid, text_code};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldFailure {
    pub field: String,
    pub code: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormErrors {
    pub fields: Vec<FieldFailure>,
}

impl FormErrors {
    pub fn push(&mut self, field: impl Into<String>, code: &'static str) {
        self.fields.push(FieldFailure {
            field: field.into(),
            code,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn for_field(&self, name: &str) -> Option<&'static str> {
        self.fields
            .iter()
            .find(|failure| failure.field == name)
            .map(|failure| failure.code)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormValues {
    values: BTreeMap<String, String>,
}

impl FormValues {
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.values.insert(name.into(), value.into());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    /// `application/x-www-form-urlencoded`. Zusätzliche Felder bleiben sichtbar für `reject_unknown`.
    pub fn parse_urlencoded(body: &str) -> Result<Self, FormErrors> {
        if body.len() > 65_536 {
            let mut errors = FormErrors::default();
            errors.push("", "request.rejected");
            return Err(errors);
        }
        let mut values = Self::default();
        if body.is_empty() {
            return Ok(values);
        }
        for pair in body.split('&') {
            let (raw_name, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
            let name = decode(raw_name);
            let value = decode(raw_value);
            if !valid_field_name(&name) {
                let mut errors = FormErrors::default();
                errors.push(&name, "validation.invalid_type");
                return Err(errors);
            }
            values.insert(name, value);
        }
        Ok(values)
    }
}

pub fn reject_unknown(values: &FormValues, allowed: &[&str]) -> Result<(), FormErrors> {
    let mut errors = FormErrors::default();
    for name in values.names() {
        if name != "csrf_token" && !allowed.contains(&name) {
            errors.push(name, "validation.unknown_field");
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TextSpec {
    pub name: &'static str,
    pub required: bool,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub email: bool,
    pub url: bool,
}

pub fn required_text(values: &FormValues, spec: TextSpec) -> Result<String, FieldFailure> {
    let value = values.get(spec.name).unwrap_or("").trim();
    match text_code(
        value,
        spec.required,
        spec.min_length,
        spec.max_length,
        spec.email,
        spec.url,
    ) {
        Some(code) => Err(FieldFailure {
            field: spec.name.to_owned(),
            code,
        }),
        None => Ok(value.to_owned()),
    }
}

pub fn optional_text(values: &FormValues, spec: TextSpec) -> Result<Option<String>, FieldFailure> {
    let value = values.get(spec.name).unwrap_or("").trim();
    if value.is_empty() {
        return Ok(None);
    }
    let mut spec = spec;
    spec.required = true;
    required_text(values, spec).map(Some)
}

pub fn required_i64(
    values: &FormValues,
    name: &'static str,
    min: Option<i64>,
    max: Option<i64>,
) -> Result<i64, FieldFailure> {
    match parse_i64(values.get(name).unwrap_or("").trim(), min, max) {
        Ok(value) => Ok(value),
        Err(code) => Err(FieldFailure {
            field: name.to_owned(),
            code,
        }),
    }
}

pub fn optional_i64(
    values: &FormValues,
    name: &'static str,
    min: Option<i64>,
    max: Option<i64>,
) -> Result<Option<i64>, FieldFailure> {
    let raw = values.get(name).unwrap_or("").trim();
    if raw.is_empty() {
        return Ok(None);
    }
    required_i64(values, name, min, max).map(Some)
}

pub fn required_bool(values: &FormValues, name: &'static str) -> Result<bool, FieldFailure> {
    match parse_bool(values.get(name).unwrap_or("").trim()) {
        Ok(value) => Ok(value),
        Err(code) => Err(FieldFailure {
            field: name.to_owned(),
            code,
        }),
    }
}

pub fn optional_bool(
    values: &FormValues,
    name: &'static str,
) -> Result<Option<bool>, FieldFailure> {
    let raw = values.get(name).unwrap_or("").trim();
    if raw.is_empty() {
        return Ok(None);
    }
    required_bool(values, name).map(Some)
}

pub fn required_uuid(values: &FormValues, name: &'static str) -> Result<uuid::Uuid, FieldFailure> {
    match parse_uuid(values.get(name).unwrap_or("").trim()) {
        Ok(value) => Ok(value),
        Err(code) => Err(FieldFailure {
            field: name.to_owned(),
            code,
        }),
    }
}

pub fn optional_uuid(
    values: &FormValues,
    name: &'static str,
) -> Result<Option<uuid::Uuid>, FieldFailure> {
    let raw = values.get(name).unwrap_or("").trim();
    if raw.is_empty() {
        return Ok(None);
    }
    required_uuid(values, name).map(Some)
}

pub trait FerrumForm: Sized {
    fn from_form(values: &FormValues) -> Result<Self, FormErrors>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactForm {
    pub name: String,
    pub email: String,
    pub message: String,
    pub website: Option<String>,
}

impl FerrumForm for ContactForm {
    fn from_form(values: &FormValues) -> Result<Self, FormErrors> {
        let mut errors = FormErrors::default();
        if let Err(unknown) = reject_unknown(values, &["name", "email", "message", "website"]) {
            errors.fields.extend(unknown.fields);
        }
        let name = required_text(
            values,
            TextSpec {
                name: "name",
                required: true,
                min_length: Some(3),
                max_length: Some(100),
                email: false,
                url: false,
            },
        );
        let email = required_text(
            values,
            TextSpec {
                name: "email",
                required: true,
                min_length: None,
                max_length: Some(254),
                email: true,
                url: false,
            },
        );
        let message = required_text(
            values,
            TextSpec {
                name: "message",
                required: true,
                min_length: None,
                max_length: Some(2000),
                email: false,
                url: false,
            },
        );
        let website = optional_text(
            values,
            TextSpec {
                name: "website",
                required: false,
                min_length: None,
                max_length: Some(2048),
                email: false,
                url: true,
            },
        );
        let name = take(name, &mut errors);
        let email = take(email, &mut errors);
        let message = take(message, &mut errors);
        let website = take(website, &mut errors).flatten();
        if let (Some(name), Some(email)) = (&name, &email) {
            let local = email.split('@').next().unwrap_or("");
            if name.eq_ignore_ascii_case(local) {
                errors.push("", "validation.mismatch");
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Self {
            name: name.unwrap(),
            email: email.unwrap(),
            message: message.unwrap(),
            website,
        })
    }
}

fn take<T>(result: Result<T, FieldFailure>, errors: &mut FormErrors) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(failure) => {
            errors.fields.push(failure);
            None
        }
    }
}

pub fn message(locale: &str, code: &str) -> &'static str {
    let german = locale == "de";
    match (german, code) {
        (true, "validation.required") => "Dieses Feld ist erforderlich.",
        (false, "validation.required") => "This field is required.",
        (true, "validation.min_length") => "Der Text ist zu kurz.",
        (false, "validation.min_length") => "The text is too short.",
        (true, "validation.max_length") => "Der Text ist zu lang.",
        (false, "validation.max_length") => "The text is too long.",
        (true, "validation.email") => "Die E-Mail-Adresse ist ungültig.",
        (false, "validation.email") => "The email address is invalid.",
        (true, "validation.url") => "Die Adresse muss mit http:// oder https:// beginnen.",
        (false, "validation.url") => "The address must start with http:// or https://.",
        (true, "validation.invalid_type") => "Der Wert hat den falschen Typ.",
        (false, "validation.invalid_type") => "The value has the wrong type.",
        (true, "validation.min_value") => "Der Wert ist zu klein.",
        (false, "validation.min_value") => "The value is too small.",
        (true, "validation.max_value") => "Der Wert ist zu groß.",
        (false, "validation.max_value") => "The value is too large.",
        (true, "validation.uuid") => "Die Kennung ist ungültig.",
        (false, "validation.uuid") => "The identifier is invalid.",
        (true, "validation.unknown_field") => "Dieses Feld ist nicht erlaubt.",
        (false, "validation.unknown_field") => "This field is not allowed.",
        (true, "validation.mismatch") => {
            "Name und E-Mail dürfen nicht denselben lokalen Teil haben."
        }
        (false, "validation.mismatch") => "The name and email must not share the same local part.",
        (true, "form.csrf") => "Die Sicherheitsprüfung ist fehlgeschlagen.",
        (false, "form.csrf") => "The security check failed.",
        (true, "form.thanks") => "Danke. Die Nachricht wurde geprüft und nicht versendet.",
        (false, "form.thanks") => "Thank you. The message was checked and was not sent.",
        _ => "missing message",
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InputSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub input_type: &'static str,
    pub required: bool,
}

pub fn render_form(
    action: &str,
    csrf: Option<&str>,
    values: &FormValues,
    errors: &FormErrors,
    fields: &[InputSpec],
    locale: &str,
) -> String {
    let mut html = format!("<form method=\"post\" action=\"{}\">", escape(action));
    if let Some(token) = csrf {
        html.push_str(&format!(
            "<input type=\"hidden\" name=\"csrf_token\" value=\"{}\">",
            escape(token)
        ));
    }
    if let Some(code) = errors.for_field("") {
        html.push_str(&format!(
            "<p role=\"alert\">{}</p>",
            escape(message(locale, code))
        ));
    }
    for field in fields {
        let value = values.get(field.name).unwrap_or("");
        html.push_str(&format!(
            "<label for=\"{name}\">{label}",
            name = escape(field.name),
            label = escape(field.label)
        ));
        if field.required {
            html.push_str("<span aria-hidden=\"true\">*</span>");
        }
        html.push_str("</label>");
        let required = if field.required { " required" } else { "" };
        html.push_str(&format!(
            "<input id=\"{name}\" name=\"{name}\" type=\"{kind}\" value=\"{value}\"{required}>",
            name = escape(field.name),
            kind = escape(field.input_type),
            value = escape(value),
        ));
        if let Some(code) = errors.for_field(field.name) {
            html.push_str(&format!(
                "<p id=\"{name}-error\" role=\"alert\">{text}</p>",
                name = escape(field.name),
                text = escape(message(locale, code))
            ));
        }
    }
    html.push_str("<button type=\"submit\">");
    html.push_str(if locale == "de" { "Senden" } else { "Send" });
    html.push_str("</button></form>");
    html
}

pub fn render_page(title: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"de\"><head><meta charset=\"utf-8\"><title>{}</title><link rel=\"stylesheet\" href=\"/static/css/app.css\"></head><body><main>{}</main></body></html>",
        escape(title),
        body
    )
}

pub fn contact_fields(locale: &str) -> [InputSpec; 4] {
    if locale == "de" {
        [
            InputSpec {
                name: "name",
                label: "Name",
                input_type: "text",
                required: true,
            },
            InputSpec {
                name: "email",
                label: "E-Mail",
                input_type: "email",
                required: true,
            },
            InputSpec {
                name: "message",
                label: "Nachricht",
                input_type: "text",
                required: true,
            },
            InputSpec {
                name: "website",
                label: "Website",
                input_type: "url",
                required: false,
            },
        ]
    } else {
        [
            InputSpec {
                name: "name",
                label: "Name",
                input_type: "text",
                required: true,
            },
            InputSpec {
                name: "email",
                label: "Email",
                input_type: "email",
                required: true,
            },
            InputSpec {
                name: "message",
                label: "Message",
                input_type: "text",
                required: true,
            },
            InputSpec {
                name: "website",
                label: "Website",
                input_type: "url",
                required: false,
            },
        ]
    }
}

pub fn escape(value: &str) -> String {
    let mut output = String::new();
    for char in value.chars() {
        match char {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(char),
        }
    }
    output
}

fn valid_field_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn decode(value: &str) -> String {
    let mut output = Vec::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    output.push(byte);
                    index += 3;
                } else {
                    output.push(b'%');
                    index += 1;
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{escape, required_i64, required_uuid, ContactForm, FerrumForm, FormValues};

    #[test]
    fn contact_form_collects_field_errors_and_escapes() {
        let mut values = FormValues::default();
        values.insert("name", "Al");
        values.insert("email", "keine");
        values.insert("message", "<script>");
        values.insert("extra", "1");
        let errors = ContactForm::from_form(&values).expect_err("ungültig");
        assert!(errors.for_field("name").is_some());
        assert_eq!(errors.for_field("email"), Some("validation.email"));
        assert_eq!(errors.for_field("extra"), Some("validation.unknown_field"));
        assert!(escape("<script>").contains("&lt;script&gt;"));
    }

    #[test]
    fn contact_form_rejects_matching_name_and_email() {
        let mut values = FormValues::default();
        values.insert("name", "ada");
        values.insert("email", "ada@example.com");
        values.insert("message", "Hallo");
        let errors = ContactForm::from_form(&values).expect_err("kreuzfeld");
        assert_eq!(errors.for_field(""), Some("validation.mismatch"));
    }

    #[test]
    fn numbers_and_uuids_are_typed() {
        let mut values = FormValues::default();
        values.insert("count", "12");
        values.insert("id", "00000000-0000-0000-0000-000000000001");
        assert_eq!(
            required_i64(&values, "count", Some(1), Some(20)).unwrap(),
            12
        );
        assert!(required_uuid(&values, "id").is_ok());
        values.insert("count", "3.5");
        assert_eq!(
            required_i64(&values, "count", None, None).unwrap_err().code,
            "validation.invalid_type"
        );
    }

    #[test]
    fn urlencoded_roundtrip_keeps_only_named_fields() {
        let values =
            FormValues::parse_urlencoded("name=Ada+Lovelace&email=ada%40example.com").unwrap();
        assert_eq!(values.get("name"), Some("Ada Lovelace"));
        assert_eq!(values.get("email"), Some("ada@example.com"));
    }
}
