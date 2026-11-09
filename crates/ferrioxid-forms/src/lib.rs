//! Typisierte Formulare. Sie sind nicht an eine Datenbanktabelle gebunden.
//!
//! Feldregeln kommen aus `ferrioxid_core`. HTML wird escaped. Unbekannte Felder werden abgelehnt.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use ferrioxid_core::{
    parse_bool, parse_date, parse_datetime_local, parse_datetime_utc, parse_i64, parse_time,
    parse_uuid, text_code, AdminFieldMeta, ColumnKind, ColumnSpec,
};
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
        });
    }
    Ok(fields)
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
        ColumnKind::BigInt => parse_i64(raw, None, None).map(BoundValue::Integer),
        ColumnKind::Boolean => parse_bool(raw).map(BoundValue::Boolean),
        ColumnKind::Uuid => parse_uuid(raw).map(|id| BoundValue::Text(id.to_string())),
        ColumnKind::Date => parse_date(raw).map(|_| BoundValue::Text(raw.to_owned())),
        ColumnKind::Time => parse_time(raw).map(|_| BoundValue::Text(raw.to_owned())),
        ColumnKind::DateTime => parse_datetime_utc(raw).map(|_| BoundValue::Text(raw.to_owned())),
        ColumnKind::Text => Ok(BoundValue::Text(raw.to_owned())),
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
        bind_model, escape, model_fields, render_fields, require_confirmation, required_choice,
        required_i64, required_postal, required_uuid, BoundValue, ContactForm, FerrioxidForm,
        Field, FormErrors, FormValues, SubmitMode, Widget,
    };
    use ferrioxid_core::{AdminFieldMeta, ColumnKind, ColumnSpec};

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
