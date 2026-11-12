//! Typisierte Formulare. Sie sind nicht an eine Datenbanktabelle gebunden.
//!
//! Feldregeln kommen aus `ferrioxid_core`. HTML wird escaped. Unbekannte Felder werden abgelehnt.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use ferrioxid_core::{
    format_decimal, parse_bool, parse_date, parse_datetime_local, parse_datetime_utc,
    parse_decimal, parse_i64, parse_time, parse_uuid, text_code, AdminFieldMeta, ColumnKind,
    ColumnSpec,
};
use ferrioxid_storage::Storage;
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

pub fn required_date(values: &FormValues, name: &'static str) -> Result<NaiveDate, FieldFailure> {
    temporal(values, name, parse_date)
}

pub fn optional_date(
    values: &FormValues,
    name: &'static str,
) -> Result<Option<NaiveDate>, FieldFailure> {
    optional_temporal(values, name, parse_date)
}

pub fn required_time(values: &FormValues, name: &'static str) -> Result<NaiveTime, FieldFailure> {
    temporal(values, name, parse_time)
}

pub fn optional_time(
    values: &FormValues,
    name: &'static str,
) -> Result<Option<NaiveTime>, FieldFailure> {
    optional_temporal(values, name, parse_time)
}

pub fn required_datetime_utc(
    values: &FormValues,
    name: &'static str,
) -> Result<DateTime<Utc>, FieldFailure> {
    temporal(values, name, parse_datetime_utc)
}

pub fn optional_datetime_utc(
    values: &FormValues,
    name: &'static str,
) -> Result<Option<DateTime<Utc>>, FieldFailure> {
    optional_temporal(values, name, parse_datetime_utc)
}

/// `datetime-local` ohne Offset. Die Zeichenkette gilt als UTC.
pub fn required_datetime_local(
    values: &FormValues,
    name: &'static str,
) -> Result<DateTime<Utc>, FieldFailure> {
    temporal(values, name, parse_datetime_local)
}

fn temporal<T>(
    values: &FormValues,
    name: &'static str,
    parse: fn(&str) -> Result<T, &'static str>,
) -> Result<T, FieldFailure> {
    let value = values.get(name).unwrap_or("");
    if value.is_empty() {
        return Err(FieldFailure {
            field: name.to_owned(),
            code: "validation.required",
        });
    }
    parse(value).map_err(|code| FieldFailure {
        field: name.to_owned(),
        code,
    })
}

fn optional_temporal<T>(
    values: &FormValues,
    name: &'static str,
    parse: fn(&str) -> Result<T, &'static str>,
) -> Result<Option<T>, FieldFailure> {
    let value = values.get(name).unwrap_or("");
    if value.is_empty() {
        return Ok(None);
    }
    parse(value).map(Some).map_err(|code| FieldFailure {
        field: name.to_owned(),
        code,
    })
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

pub fn required_column_choice<'a>(
    values: &FormValues,
    column: &'a ferrioxid_core::ColumnSpec,
) -> Result<&'a str, FieldFailure> {
    let text = values.get(column.name).unwrap_or("").trim();
    if let Some(choice) = column.choices.iter().find(|choice| choice.value == text) {
        Ok(choice.value)
    } else {
        Err(FieldFailure {
            field: column.name.to_owned(),
            code: "validation.choice",
        })
    }
}

pub fn choice_options(
    column: &ferrioxid_core::ColumnSpec,
    locale: &str,
) -> Vec<(&'static str, &'static str)> {
    column
        .choices
        .iter()
        .map(|choice| {
            (
                choice.value,
                if locale == "de" {
                    choice.label_de
                } else {
                    choice.label_en
                },
            )
        })
        .collect()
}

pub fn render_choice_select(
    column: &ferrioxid_core::ColumnSpec,
    locale: &str,
    selected: &str,
    required: bool,
) -> String {
    let mut html = format!(
        "<select id=\"{name}\" name=\"{name}\"{required}>",
        name = escape(column.name),
        required = if required { " required" } else { "" }
    );
    for (value, label) in choice_options(column, locale) {
        let mark = if value == selected { " selected" } else { "" };
        html.push_str(&format!(
            "<option value=\"{value}\"{mark}>{label}</option>",
            value = escape(value),
            label = escape(label),
        ));
    }
    html.push_str("</select>");
    html
}

pub fn required_decimal(
    values: &FormValues,
    name: &'static str,
    precision: u8,
    scale: u8,
) -> Result<ferrioxid_core::Decimal, FieldFailure> {
    let text = values.get(name).unwrap_or("").trim();
    ferrioxid_core::parse_decimal(text, precision, scale).map_err(|_| FieldFailure {
        field: name.to_owned(),
        code: "validation.decimal",
    })
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

pub fn required_choice<'a>(
    values: &'a FormValues,
    name: &'static str,
    options: &[&str],
) -> Result<&'a str, FieldFailure> {
    let value = values.get(name).unwrap_or("").trim();
    if value.is_empty() {
        return Err(FieldFailure {
            field: name.to_owned(),
            code: "validation.required",
        });
    }
    if options.contains(&value) {
        Ok(value)
    } else {
        Err(FieldFailure {
            field: name.to_owned(),
            code: "validation.choice",
        })
    }
}

pub fn optional_choice<'a>(
    values: &'a FormValues,
    name: &'static str,
    options: &[&str],
) -> Result<Option<&'a str>, FieldFailure> {
    let value = values.get(name).unwrap_or("").trim();
    if value.is_empty() {
        return Ok(None);
    }
    required_choice(values, name, options).map(Some)
}

/// Fünf Ziffern. Kürzere oder längere Werte behalten die Längencodes.
pub fn required_postal(values: &FormValues, name: &'static str) -> Result<String, FieldFailure> {
    let value = required_text(
        values,
        TextSpec {
            name,
            required: true,
            min_length: Some(5),
            max_length: Some(5),
            email: false,
            url: false,
        },
    )?;
    if value.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(value)
    } else {
        Err(FieldFailure {
            field: name.to_owned(),
            code: "validation.postal",
        })
    }
}

/// Nur der exakte Checkbox-Wert `1` gilt. Andere Werte sind ungültig, ein fehlender Wert ist keine Bestätigung.
pub fn require_confirmation(values: &FormValues) -> Result<(), FormErrors> {
    let mut errors = FormErrors::default();
    match values.get("confirm") {
        Some("1") => {}
        Some(_) => errors.push("confirm", "validation.invalid"),
        None => errors.push("confirm", "validation.required"),
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub trait FerrioxidForm: Sized {
    fn from_form(values: &FormValues) -> Result<Self, FormErrors>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactForm {
    pub name: String,
    pub email: String,
    pub message: String,
    pub website: Option<String>,
    pub topic: Option<String>,
}

impl FerrioxidForm for ContactForm {
    fn from_form(values: &FormValues) -> Result<Self, FormErrors> {
        let mut errors = FormErrors::default();
        if let Err(unknown) =
            reject_unknown(values, &["name", "email", "message", "website", "topic"])
        {
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
        let topic = take(
            optional_choice(values, "topic", &["order", "product", "other"]),
            &mut errors,
        )
        .flatten()
        .map(str::to_owned);
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
            topic,
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
    ferrioxid_core::message_text(locale, code)
}

pub use ferrioxid_core::{localize_detail, message_for, FieldOverride};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Widget {
    Text,
    Email,
    Password,
    Number,
    Url,
    Checkbox,
    Textarea,
    Select,
}

/// Sichtbares Feld. `required` ist nur ein Hinweis; die Prüfung bleibt serverseitig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub label: &'static str,
    pub widget: Widget,
    pub required: bool,
    pub help: &'static str,
    pub placeholder: &'static str,
    pub options: &'static [(&'static str, &'static str)],
}

impl Field {
    pub const fn new(
        name: &'static str,
        label: &'static str,
        widget: Widget,
        required: bool,
    ) -> Self {
        Self {
            name,
            label,
            widget,
            required,
            help: "",
            placeholder: "",
            options: &[],
        }
    }

    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }

    pub const fn placeholder(mut self, placeholder: &'static str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub const fn options(mut self, options: &'static [(&'static str, &'static str)]) -> Self {
        self.options = options;
        self
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
    let owned: Vec<Field> = fields.iter().map(field_from_input).collect();
    render_fields(action, csrf, values, errors, &owned, locale)
}

pub fn render_fields(
    action: &str,
    csrf: Option<&str>,
    values: &FormValues,
    errors: &FormErrors,
    fields: &[Field],
    locale: &str,
) -> String {
    render_fields_with(action, csrf, values, errors, fields, locale, &[])
}

pub fn render_fields_with(
    action: &str,
    csrf: Option<&str>,
    values: &FormValues,
    errors: &FormErrors,
    fields: &[Field],
    locale: &str,
    overrides: &[FieldOverride],
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
            escape(&message_for(locale, "", code, &[], overrides))
        ));
    }
    for field in fields {
        let value = values.get(field.name).unwrap_or("");
        let name = escape(field.name);
        html.push_str(&format!(
            "<label for=\"{name}\">{label}",
            label = escape(field.label)
        ));
        if field.required {
            html.push_str("<span aria-hidden=\"true\">*</span>");
        }
        html.push_str("</label>");
        html.push_str(&control(field, value, errors));
        if let Some(code) = errors.for_field(field.name) {
            html.push_str(&format!(
                "<p id=\"{name}-error\" role=\"alert\">{text}</p>",
                text = escape(&message_for(locale, field.name, code, &[], overrides))
            ));
        }
        if !field.help.is_empty() {
            html.push_str(&format!(
                "<p id=\"{name}-help\">{text}</p>",
                text = escape(field.help)
            ));
        }
    }
    html.push_str("<button type=\"submit\">");
    html.push_str(if locale == "de" { "Senden" } else { "Send" });
    html.push_str("</button></form>");
    html
}

fn field_from_input(spec: &InputSpec) -> Field {
    let widget = match spec.input_type {
        "email" => Widget::Email,
        "password" => Widget::Password,
        "number" => Widget::Number,
        "checkbox" => Widget::Checkbox,
        "url" => Widget::Url,
        "textarea" => Widget::Textarea,
        _ => Widget::Text,
    };
    Field::new(spec.name, spec.label, widget, spec.required)
}

fn control(field: &Field, value: &str, errors: &FormErrors) -> String {
    let name = escape(field.name);
    let described = described_by(field, errors);
    let invalid = if errors.for_field(field.name).is_some() {
        " aria-invalid=\"true\""
    } else {
        ""
    };
    let required = if field.required { " required" } else { "" };
    let placeholder = if field.placeholder.is_empty() {
        String::new()
    } else {
        format!(" placeholder=\"{}\"", escape(field.placeholder))
    };
    match field.widget {
        Widget::Textarea => format!(
            "<textarea id=\"{name}\" name=\"{name}\"{required}{placeholder}{described}{invalid}>{value}</textarea>",
            value = escape(value)
        ),
        Widget::Select => {
            let mut html = format!(
                "<select id=\"{name}\" name=\"{name}\"{required}{described}{invalid}>"
            );
            if !field.required {
                html.push_str("<option value=\"\"></option>");
            }
            for (option, label) in field.options {
                let selected = if *option == value { " selected" } else { "" };
                html.push_str(&format!(
                    "<option value=\"{option}\"{selected}>{label}</option>",
                    option = escape(option),
                    label = escape(label)
                ));
            }
            html.push_str("</select>");
            html
        }
        Widget::Checkbox => {
            let checked = if matches!(value, "true" | "1" | "on") {
                " checked"
            } else {
                ""
            };
            format!(
                "<input id=\"{name}\" name=\"{name}\" type=\"checkbox\" value=\"on\"{checked}{required}{described}{invalid}>"
            )
        }
        Widget::Password => format!(
            "<input id=\"{name}\" name=\"{name}\" type=\"password\" value=\"\"{required}{placeholder}{described}{invalid}>"
        ),
        other => {
            let kind = match other {
                Widget::Email => "email",
                Widget::Number => "number",
                Widget::Url => "url",
                _ => "text",
            };
            format!(
                "<input id=\"{name}\" name=\"{name}\" type=\"{kind}\" value=\"{value}\"{required}{placeholder}{described}{invalid}>",
                value = escape(value)
            )
        }
    }
}

fn described_by(field: &Field, errors: &FormErrors) -> String {
    let mut ids = Vec::new();
    if errors.for_field(field.name).is_some() {
        ids.push(format!("{}-error", field.name));
    }
    if !field.help.is_empty() {
        ids.push(format!("{}-help", field.name));
    }
    if ids.is_empty() {
        String::new()
    } else {
        format!(" aria-describedby=\"{}\"", escape(&ids.join(" ")))
    }
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

const TOPICS: &[(&str, &str)] = &[
    ("order", "Bestellung"),
    ("product", "Produkt"),
    ("other", "Sonstiges"),
];

const TOPICS_EN: &[(&str, &str)] = &[
    ("order", "Order"),
    ("product", "Product"),
    ("other", "Other"),
];

pub fn contact_widgets(locale: &str) -> [Field; 5] {
    let german = locale == "de";
    let topics = if german { TOPICS } else { TOPICS_EN };
    [
        Field::new("name", "Name", Widget::Text, true).placeholder("Ada"),
        Field::new(
            "email",
            if german { "E-Mail" } else { "Email" },
            Widget::Email,
            true,
        ),
        Field::new(
            "topic",
            if german { "Thema" } else { "Topic" },
            Widget::Select,
            false,
        )
        .options(topics),
        Field::new(
            "message",
            if german { "Nachricht" } else { "Message" },
            Widget::Textarea,
            true,
        )
        .help(if german {
            "Höchstens 2000 Zeichen."
        } else {
            "At most 2000 characters."
        }),
        Field::new("website", "Website", Widget::Url, false),
    ]
}

/// `Create` und `Replace` verlangen jedes Pflichtfeld. `Patch` lässt fehlende Schlüssel unverändert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitMode {
    Create,
    Replace,
    Patch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelField {
    pub name: &'static str,
    pub kind: ColumnKind,
    pub required: bool,
    pub precision: u8,
    pub scale: u8,
    pub choices: &'static [ferrioxid_core::ChoiceValue],
    pub min: Option<i64>,
    pub max: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormSetupError {
    pub code: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundValue {
    Unchanged,
    Null,
    Text(String),
    Integer(i64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundField {
    pub name: &'static str,
    pub value: BoundValue,
}

/// Nur Namen aus `allow` werden Felder. Primärschlüssel sowie sensitive, schreibgeschützte und versteckte Admin-Felder werden abgelehnt.
pub fn model_fields(
    columns: &[ColumnSpec],
    admin: &[AdminFieldMeta],
    allow: &[&str],
) -> Result<Vec<ModelField>, FormSetupError> {
    let mut fields = Vec::new();
    for name in allow {
        if fields.iter().any(|field: &ModelField| field.name == *name) {
            return Err(FormSetupError {
                code: "form.duplicate_column",
            });
        }
        let Some(column) = columns.iter().find(|column| column.name == *name) else {
            return Err(FormSetupError {
                code: "form.unknown_column",
            });
        };
        if column.primary_key {
            return Err(FormSetupError {
                code: "form.protected_column",
            });
        }
        if admin
            .iter()
            .any(|meta| meta.name == *name && (meta.sensitive || meta.readonly || meta.hidden))
        {
            return Err(FormSetupError {
                code: "form.protected_column",
            });
        }
        fields.push(ModelField {
            name: column.name,
            kind: column.kind,
            required: !column.nullable && column.default_bool.is_none(),
            precision: column.precision,
            scale: column.scale,
            choices: column.choices,
            min: None,
            max: None,
        });
    }
    Ok(fields)
}

/// Copies `#[field(min, max)]` onto the allowlisted fields. Browser attributes are not consulted.
pub fn apply_bounds(fields: &mut [ModelField], bounds: &[ferrioxid_core::NumericBound]) {
    for field in fields {
        if let Some(bound) = bounds.iter().find(|bound| bound.column == field.name) {
            field.min = bound.min;
            field.max = bound.max;
        }
    }
}

pub fn model_widget(field: &ModelField, label: &'static str) -> Field {
    let widget = match field.kind {
        ColumnKind::BigInt => Widget::Number,
        ColumnKind::Boolean => Widget::Checkbox,
        _ => Widget::Text,
    };
    Field::new(field.name, label, widget, field.required)
}

/// `choices` begrenzt einzelne Felder auf erlaubte Werte. Zusätzliche Eingabefelder werden abgelehnt.
pub fn bind_model(
    mode: SubmitMode,
    fields: &[ModelField],
    values: &FormValues,
    choices: &[(&str, &[&str])],
) -> Result<Vec<BoundField>, FormErrors> {
    let mut errors = FormErrors::default();
    for name in values.names() {
        if name != "csrf_token" && !fields.iter().any(|field| field.name == name) {
            errors.push(name, "validation.unknown_field");
        }
    }
    let mut bound = Vec::new();
    for field in fields {
        match bind_one(mode, field, values, choices) {
            Ok(value) => bound.push(BoundField {
                name: field.name,
                value,
            }),
            Err(code) => errors.push(field.name, code),
        }
    }
    if errors.is_empty() {
        Ok(bound)
    } else {
        Err(errors)
    }
}

fn bind_one(
    mode: SubmitMode,
    field: &ModelField,
    values: &FormValues,
    choices: &[(&str, &[&str])],
) -> Result<BoundValue, &'static str> {
    let Some(raw) = values.get(field.name) else {
        if mode == SubmitMode::Patch {
            return Ok(BoundValue::Unchanged);
        }
        return if field.required {
            Err("validation.required")
        } else {
            Ok(BoundValue::Null)
        };
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return if field.required {
            Err("validation.required")
        } else {
            Ok(BoundValue::Null)
        };
    }
    if let Some((_, options)) = choices.iter().find(|(name, _)| *name == field.name) {
        if !options.contains(&raw) {
            return Err("validation.choice");
        }
    }
    match field.kind {
        ColumnKind::BigInt => parse_i64(raw, field.min, field.max).map(BoundValue::Integer),
        ColumnKind::Boolean => parse_bool(raw).map(BoundValue::Boolean),
        ColumnKind::Uuid => parse_uuid(raw).map(|id| BoundValue::Text(id.to_string())),
        ColumnKind::Date => parse_date(raw).map(|_| BoundValue::Text(raw.to_owned())),
        ColumnKind::Time => parse_time(raw).map(|_| BoundValue::Text(raw.to_owned())),
        ColumnKind::DateTime => parse_datetime_utc(raw).map(|_| BoundValue::Text(raw.to_owned())),
        ColumnKind::Text => Ok(BoundValue::Text(raw.to_owned())),
        ColumnKind::Json => ferrioxid_core::canonical_json_text(raw)
            .map(BoundValue::Text)
            .map_err(|_| "validation.json"),
        ColumnKind::Decimal => {
            let value = parse_decimal(raw, field.precision, field.scale)
                .map_err(|_| "validation.decimal")?;
            if field
                .min
                .is_some_and(|min| value < ferrioxid_core::Decimal::from(min))
            {
                return Err("validation.min_value");
            }
            if field
                .max
                .is_some_and(|max| value > ferrioxid_core::Decimal::from(max))
            {
                return Err("validation.max_value");
            }
            format_decimal(value, field.scale)
                .map(BoundValue::Text)
                .map_err(|_| "validation.decimal")
        }
        ColumnKind::Choice => {
            if field.choices.iter().any(|choice| choice.value == raw) {
                Ok(BoundValue::Text(raw.to_owned()))
            } else {
                Err("validation.choice")
            }
        }
    }
}

/// Hard cap for one formset. A larger `max_rows` is rejected.
pub const FORMSET_MAX_ROWS: usize = 100;

/// Binds `prefix_TOTAL_FORMS` rows. Field names are `prefix_0_name`.
///
/// `max_rows` is the application limit and must be between 1 and [`FORMSET_MAX_ROWS`].
/// Extra indexes and an oversized total are errors. Each row is parsed with `parse`,
/// and its errors keep the prefixed field name.
pub fn bind_formset<T>(
    values: &FormValues,
    prefix: &str,
    max_rows: usize,
    mode: SubmitMode,
    mut parse: impl FnMut(SubmitMode, &FormValues) -> Result<T, FormErrors>,
) -> Result<Vec<T>, FormErrors> {
    let mut errors = FormErrors::default();
    if !valid_field_name(prefix) || max_rows == 0 || max_rows > FORMSET_MAX_ROWS {
        errors.push(prefix, "form.formset");
        return Err(errors);
    }
    let total_name = format!("{prefix}_TOTAL_FORMS");
    let initial_name = format!("{prefix}_INITIAL_FORMS");
    let total = match values.get(&total_name).map(str::trim) {
        Some(raw) => match raw.parse::<usize>() {
            Ok(total) if total <= max_rows => total,
            Ok(_) => {
                errors.push(&total_name, "validation.formset_size");
                return Err(errors);
            }
            Err(_) => {
                errors.push(&total_name, "validation.invalid_type");
                return Err(errors);
            }
        },
        None => {
            errors.push(&total_name, "validation.required");
            return Err(errors);
        }
    };
    if let Some(raw) = values.get(&initial_name).map(str::trim) {
        match raw.parse::<usize>() {
            Ok(initial) if initial <= total => {}
            Ok(_) => errors.push(&initial_name, "validation.formset_size"),
            Err(_) => errors.push(&initial_name, "validation.invalid_type"),
        }
    }
    let marker = format!("{prefix}_");
    for name in values.names() {
        if name == total_name || name == initial_name || !name.starts_with(&marker) {
            continue;
        }
        let rest = &name[marker.len()..];
        let Some((index, _)) = rest.split_once('_') else {
            errors.push(name, "validation.unknown_field");
            continue;
        };
        match index.parse::<usize>() {
            Ok(index) if index < total => {}
            _ => errors.push(name, "validation.formset_size"),
        }
    }
    let mut rows = Vec::with_capacity(total);
    for index in 0..total {
        let row_prefix = format!("{prefix}_{index}_");
        let mut row = FormValues::default();
        for name in values.names() {
            if let Some(field) = name.strip_prefix(&row_prefix) {
                if let Some(value) = values.get(name) {
                    row.insert(field, value);
                }
            }
        }
        match parse(mode, &row) {
            Ok(item) => rows.push(item),
            Err(row_errors) => {
                for failure in row_errors.fields {
                    let field = if failure.field.is_empty() {
                        row_prefix.trim_end_matches('_').to_owned()
                    } else {
                        format!("{row_prefix}{}", failure.field)
                    };
                    errors.push(field, failure.code);
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(rows)
    } else {
        Err(errors)
    }
}

/// Stores an upload through the storage API.
///
/// The client name is never used as a path. Names containing a separator or
/// `..` are rejected. Size and content type are enforced by the storage root.
pub async fn store_upload(
    storage: &ferrioxid_storage::LocalStorage,
    owner: uuid::Uuid,
    original_name: &str,
    bytes: &[u8],
) -> Result<ferrioxid_storage::StoredFile, FormErrors> {
    let mut errors = FormErrors::default();
    if original_name.trim().is_empty()
        || original_name.contains("..")
        || original_name.contains('/')
        || original_name.contains('\\')
        || original_name.contains('\0')
    {
        errors.push("file", "validation.upload_name");
        return Err(errors);
    }
    match storage.save(owner, original_name, bytes).await {
        Ok(stored) => Ok(stored),
        Err(ferrioxid_storage::StorageError::TooLarge) => {
            errors.push("file", "validation.upload_size");
            Err(errors)
        }
        Err(ferrioxid_storage::StorageError::TypeRejected) => {
            errors.push("file", "validation.upload_type");
            Err(errors)
        }
        Err(_) => {
            errors.push("file", "validation.upload");
            Err(errors)
        }
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
    use super::{
        apply_bounds, bind_formset, bind_model, escape, model_fields, render_fields,
        require_confirmation, required_choice, required_i64, required_postal, required_uuid,
        store_upload, BoundValue, ContactForm, FerrioxidForm, Field, FormErrors, FormValues,
        ModelField, SubmitMode, Widget,
    };
    use ferrioxid_core::{AdminFieldMeta, ColumnKind, ColumnSpec, NumericBound};
    use ferrioxid_storage::Storage;

    #[test]
    fn confirmation_accepts_only_the_checkbox_value() {
        let mut missing = FormValues::default();
        missing.insert("email", "ada@example.com");
        assert_eq!(
            require_confirmation(&missing)
                .expect_err("fehlt")
                .for_field("confirm"),
            Some("validation.required")
        );
        let mut forged = FormValues::default();
        forged.insert("confirm", "true");
        assert_eq!(
            require_confirmation(&forged)
                .expect_err("falsch")
                .for_field("confirm"),
            Some("validation.invalid")
        );
        let mut accepted = FormValues::default();
        accepted.insert("confirm", "1");
        assert!(require_confirmation(&accepted).is_ok());
    }

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

    #[test]
    fn widgets_escape_values_labels_and_reject_unknown_options() {
        let mut values = FormValues::default();
        values.insert("note", "<script>");
        values.insert("topic", "\"><img>");
        values.insert("secret", "hunter2");
        let mut errors = FormErrors::default();
        errors.push("note", "validation.required");
        let fields = [
            Field::new("note", "<b>Notiz</b>", Widget::Textarea, true)
                .help("Hilfe <i>")
                .placeholder("\"hier\""),
            Field::new("topic", "Thema", Widget::Select, true).options(&[("order", "<ok>")]),
            Field::new("secret", "Geheim", Widget::Password, true),
        ];
        let html = render_fields("/save", None, &values, &errors, &fields, "de");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;b&gt;Notiz&lt;/b&gt;"));
        assert!(html.contains("Hilfe &lt;i&gt;"));
        assert!(html.contains("placeholder=\"&quot;hier&quot;\""));
        assert!(html.contains("aria-describedby=\"note-error note-help\""));
        assert!(html.contains("<option value=\"order\">&lt;ok&gt;</option>"));
        assert!(!html.contains("\"><img>"));
        assert!(html.contains("type=\"password\" value=\"\""));
        assert!(!html.contains("hunter2"));
        assert_eq!(
            required_choice(&values, "topic", &["order"])
                .unwrap_err()
                .code,
            "validation.choice"
        );
    }

    #[test]
    fn prefilled_values_remain_after_a_failed_submit() {
        let mut values = FormValues::default();
        values.insert("title", "Entwurf");
        values.insert("topic", "order");
        let mut errors = FormErrors::default();
        errors.push("title", "validation.min_length");
        let fields = [
            Field::new("title", "Titel", Widget::Text, true),
            Field::new("topic", "Thema", Widget::Select, true).options(&[("order", "Bestellung")]),
        ];
        let html = render_fields("/save", None, &values, &errors, &fields, "en");
        assert!(html.contains("value=\"Entwurf\""));
        assert!(html.contains("value=\"order\" selected"));
        assert!(html.contains("The text is too short."));
    }

    #[test]
    fn model_binding_rejects_unlisted_and_protected_fields() {
        let columns = [
            column("id", ColumnKind::Uuid, false, true),
            column("title", ColumnKind::Text, false, false),
            column("stock", ColumnKind::BigInt, true, false),
            column("secret", ColumnKind::Text, true, false),
        ];
        let admin = [admin_field("secret", true, false, false)];
        assert_eq!(
            model_fields(&columns, &admin, &["id"]).unwrap_err().code,
            "form.protected_column"
        );
        assert_eq!(
            model_fields(&columns, &admin, &["secret"])
                .unwrap_err()
                .code,
            "form.protected_column"
        );
        assert_eq!(
            model_fields(&columns, &admin, &["role"]).unwrap_err().code,
            "form.unknown_column"
        );
        let fields = model_fields(&columns, &admin, &["title", "stock"]).unwrap();
        let mut posted = FormValues::default();
        posted.insert("title", "Stift");
        posted.insert("role", "admin");
        let errors = bind_model(SubmitMode::Create, &fields, &posted, &[]).unwrap_err();
        assert_eq!(errors.for_field("role"), Some("validation.unknown_field"));
        let mut clean = FormValues::default();
        clean.insert("title", "Stift");
        let bound = bind_model(SubmitMode::Create, &fields, &clean, &[]).unwrap();
        assert_eq!(bound[0].value, BoundValue::Text("Stift".to_owned()));
        assert_eq!(bound[1].value, BoundValue::Null);
        let patched = bind_model(SubmitMode::Patch, &fields, &FormValues::default(), &[]).unwrap();
        assert_eq!(patched[0].value, BoundValue::Unchanged);
        let mut missing = FormValues::default();
        missing.insert("stock", "2");
        assert_eq!(
            bind_model(SubmitMode::Replace, &fields, &missing, &[])
                .unwrap_err()
                .for_field("title"),
            Some("validation.required")
        );
        let mut invalid = FormValues::default();
        invalid.insert("title", "Stift");
        invalid.insert("stock", "nope");
        let mut json_values = FormValues::default();
        json_values.insert("body", " { \"b\": 1 } ");
        let json_fields = [ModelField {
            name: "body",
            kind: ColumnKind::Json,
            required: true,
            precision: 0,
            scale: 0,
            choices: &[],
            min: None,
            max: None,
        }];
        let json_bound = bind_model(SubmitMode::Create, &json_fields, &json_values, &[]).unwrap();
        assert_eq!(
            json_bound[0].value,
            BoundValue::Text("{\"b\":1}".to_owned())
        );
        let mut broken_json = FormValues::default();
        broken_json.insert("body", "{");
        assert_eq!(
            bind_model(SubmitMode::Create, &json_fields, &broken_json, &[])
                .unwrap_err()
                .for_field("body"),
            Some("validation.json")
        );
        let mut with_csrf = FormValues::default();
        with_csrf.insert("csrf_token", "session-bound-token");
        with_csrf.insert("body", " { \"b\": 1, \"a\": null } ");
        let csrf_bound = bind_model(SubmitMode::Create, &json_fields, &with_csrf, &[]).unwrap();
        assert_eq!(
            csrf_bound[0].value,
            BoundValue::Text("{\"a\":null,\"b\":1}".to_owned())
        );
        with_csrf.insert("extra", "nope");
        assert_eq!(
            bind_model(SubmitMode::Create, &json_fields, &with_csrf, &[])
                .unwrap_err()
                .for_field("extra"),
            Some("validation.unknown_field")
        );
        assert_eq!(
            bind_model(SubmitMode::Replace, &fields, &invalid, &[])
                .unwrap_err()
                .for_field("stock"),
            Some("validation.invalid_type")
        );
        let mut choice = FormValues::default();
        choice.insert("title", "nope");
        assert_eq!(
            bind_model(
                SubmitMode::Replace,
                &fields,
                &choice,
                &[("title", &["Stift"])]
            )
            .unwrap_err()
            .for_field("title"),
            Some("validation.choice")
        );
    }

    #[test]
    fn model_binding_checks_decimal_scale_and_choice_tokens() {
        let mut price = column("price", ColumnKind::Decimal, false, false);
        price.precision = 12;
        price.scale = 2;
        let mut status = column("status", ColumnKind::Choice, false, false);
        status.choices = &[ferrioxid_core::ChoiceValue {
            value: "active",
            label_de: "Aktiv",
            label_en: "Active",
        }];
        let fields = model_fields(&[price, status], &[], &["price", "status"]).unwrap();
        let mut values = FormValues::default();
        values.insert("price", "0.10");
        values.insert("status", "active");
        let bound = bind_model(SubmitMode::Create, &fields, &values, &[]).unwrap();
        assert_eq!(bound[0].value, BoundValue::Text("0.10".to_owned()));
        assert_eq!(bound[1].value, BoundValue::Text("active".to_owned()));
        values.insert("price", "0.001");
        values.insert("status", "nope");
        let errors = bind_model(SubmitMode::Create, &fields, &values, &[]).unwrap_err();
        assert_eq!(errors.for_field("price"), Some("validation.decimal"));
        assert_eq!(errors.for_field("status"), Some("validation.choice"));
    }

    #[test]
    fn postal_code_uses_its_own_code() {
        let mut values = FormValues::default();
        values.insert("postal_code", "ABCDE");
        assert_eq!(
            required_postal(&values, "postal_code").unwrap_err().code,
            "validation.postal"
        );
        values.insert("postal_code", "10115");
        assert_eq!(required_postal(&values, "postal_code").unwrap(), "10115");
    }

    #[test]
    fn numeric_bounds_reject_browser_values_outside_the_model() {
        let stock = column("stock", ColumnKind::BigInt, false, false);
        let price = ColumnSpec {
            precision: 8,
            scale: 2,
            ..column("price", ColumnKind::Decimal, false, false)
        };
        let mut fields = model_fields(&[stock, price], &[], &["stock", "price"]).unwrap();
        apply_bounds(
            &mut fields,
            &[
                NumericBound {
                    column: "stock",
                    min: Some(0),
                    max: Some(10),
                },
                NumericBound {
                    column: "price",
                    min: Some(1),
                    max: None,
                },
            ],
        );
        let mut values = FormValues::default();
        values.insert("stock", "11");
        values.insert("price", "0.50");
        let errors = bind_model(SubmitMode::Create, &fields, &values, &[]).unwrap_err();
        assert_eq!(errors.for_field("stock"), Some("validation.max_value"));
        assert_eq!(errors.for_field("price"), Some("validation.min_value"));
        values.insert("stock", "4");
        values.insert("price", "1.25");
        let bound = bind_model(SubmitMode::Replace, &fields, &values, &[]).unwrap();
        assert_eq!(bound[0].value, BoundValue::Integer(4));
        assert_eq!(bound[1].value, BoundValue::Text("1.25".to_owned()));
        let patched = bind_model(SubmitMode::Patch, &fields, &FormValues::default(), &[]).unwrap();
        assert_eq!(patched[0].value, BoundValue::Unchanged);
    }

    #[test]
    fn formset_bounds_rows_and_prefixes_errors() {
        let mut values = FormValues::default();
        values.insert("lines_TOTAL_FORMS", "2");
        values.insert("lines_0_name", "one");
        values.insert("lines_1_name", "");
        values.insert("lines_2_name", "extra");
        let errors = bind_formset(&values, "lines", 5, SubmitMode::Create, |_mode, row| {
            let mut errors = FormErrors::default();
            match row.get("name").map(str::trim) {
                Some(name) if !name.is_empty() => Ok(name.to_owned()),
                _ => {
                    errors.push("name", "validation.required");
                    Err(errors)
                }
            }
        })
        .unwrap_err();
        assert_eq!(
            errors.for_field("lines_1_name"),
            Some("validation.required")
        );
        assert_eq!(
            errors.for_field("lines_2_name"),
            Some("validation.formset_size")
        );
        let mut values = FormValues::default();
        values.insert("lines_TOTAL_FORMS", "1");
        values.insert("lines_0_name", "one");
        let rows = bind_formset(&values, "lines", 5, SubmitMode::Create, |_mode, row| {
            Ok(row.get("name").unwrap_or("").to_owned())
        })
        .unwrap();
        assert_eq!(rows, vec!["one".to_owned()]);
        values.insert("lines_TOTAL_FORMS", "9");
        let oversized = bind_formset(&values, "lines", 2, SubmitMode::Create, |_mode, row| {
            Ok(row.get("name").unwrap_or("").to_owned())
        })
        .unwrap_err();
        assert_eq!(
            oversized.for_field("lines_TOTAL_FORMS"),
            Some("validation.formset_size")
        );
    }

    #[tokio::test]
    async fn upload_rejects_traversal_and_stores_a_png() {
        let root = std::env::temp_dir().join(format!("ferrioxid-upload-{}", uuid::Uuid::new_v4()));
        let storage = ferrioxid_storage::LocalStorage::open(&root, 32, &["image/png"]).unwrap();
        let owner = uuid::Uuid::from_u128(3);
        let traversed = store_upload(&storage, owner, "../secret.png", b"not-a-png")
            .await
            .unwrap_err();
        assert_eq!(traversed.for_field("file"), Some("validation.upload_name"));
        let png: &[u8] = b"\x89PNG\r\n\x1a\nsafe";
        let stored = store_upload(&storage, owner, "photo.png", png)
            .await
            .unwrap();
        assert_eq!(stored.original_name, "photo.png");
        assert!(!stored.id.to_string().contains(".."));
        let (_meta, bytes) = storage.read(owner, stored.id).await.unwrap();
        assert_eq!(bytes, png);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn column(
        name: &'static str,
        kind: ColumnKind,
        nullable: bool,
        primary_key: bool,
    ) -> ColumnSpec {
        ColumnSpec {
            name,
            kind,
            nullable,
            primary_key,
            default_bool: None,
            unique: false,
            indexed: false,
            foreign_table: None,
            foreign_column: None,
            on_delete: None,
            precision: 0,
            scale: 0,
            choices: &[],
        }
    }

    fn admin_field(
        name: &'static str,
        sensitive: bool,
        readonly: bool,
        hidden: bool,
    ) -> AdminFieldMeta {
        AdminFieldMeta {
            name,
            label: name,
            list_display: false,
            list_link: false,
            searchable: false,
            sortable: false,
            filterable: false,
            readonly,
            hidden,
            sensitive,
            fieldset: "",
        }
    }
}
