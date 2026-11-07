//! Registrierte Formular-Widgets. Ein Widget ändert die Darstellung, nicht das Recht.

use ferrum_core::{is_email, is_http_url, AdminModelMeta, ColumnKind, ColumnSpec};
use serde_json::{Map, Value};

use crate::views::{valid_key, ExtensionError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetKind {
    Text,
    Textarea { max_length: usize },
    Email,
    Url,
    Number { min: Option<i64>, max: Option<i64> },
    Readonly,
}

#[derive(Clone, Debug)]
pub struct AdminWidget {
    pub model_key: &'static str,
    pub field: &'static str,
    pub kind: WidgetKind,
}

impl AdminWidget {
    pub fn text(model_key: &'static str, field: &'static str) -> Self {
        Self {
            model_key,
            field,
            kind: WidgetKind::Text,
        }
    }

    pub fn textarea(model_key: &'static str, field: &'static str, max_length: usize) -> Self {
        Self {
            model_key,
            field,
            kind: WidgetKind::Textarea { max_length },
        }
    }

    pub fn email(model_key: &'static str, field: &'static str) -> Self {
        Self {
            model_key,
            field,
            kind: WidgetKind::Email,
        }
    }

    pub fn url(model_key: &'static str, field: &'static str) -> Self {
        Self {
            model_key,
            field,
            kind: WidgetKind::Url,
        }
    }

    pub fn number(
        model_key: &'static str,
        field: &'static str,
        min: Option<i64>,
        max: Option<i64>,
    ) -> Self {
        Self {
            model_key,
            field,
            kind: WidgetKind::Number { min, max },
        }
    }

    pub fn readonly(model_key: &'static str, field: &'static str) -> Self {
        Self {
            model_key,
            field,
            kind: WidgetKind::Readonly,
        }
    }
}

pub(crate) fn check_widget(
    widget: &AdminWidget,
    meta: Option<&AdminModelMeta>,
    columns: Option<&[ColumnSpec]>,
) -> Result<(), ExtensionError> {
    if !valid_key(widget.model_key) || !valid_key(widget.field) {
        return Err(ExtensionError::new(
            "extension.widget",
            "Widget oder Feldname ist ungültig.",
        ));
    }
    if let WidgetKind::Textarea { max_length } = widget.kind {
        if max_length == 0 || max_length > 4000 {
            return Err(ExtensionError::new(
                "extension.widget",
                "Ein Textbereich erlaubt 1 bis 4000 Zeichen.",
            ));
        }
    }
    if let WidgetKind::Number { min, max } = widget.kind {
        if min.is_some_and(|min| max.is_some_and(|max| min > max)) {
            return Err(ExtensionError::new(
                "extension.widget",
                "Die Zahlgrenze ist umgekehrt.",
            ));
        }
    }
    if let (Some(meta), Some(columns)) = (meta, columns) {
        let Some(field) = meta.fields.iter().find(|field| field.name == widget.field) else {
            return Err(ExtensionError::new(
                "extension.widget",
                "Das Widget-Feld gehört nicht zum Modell.",
            ));
        };
        if field.hidden || field.sensitive {
            return Err(ExtensionError::new(
                "extension.widget",
                "Ein Widget darf ein verborgenes Feld nicht öffnen.",
            ));
        }
        let Some(column) = columns.iter().find(|column| column.name == widget.field) else {
            return Err(ExtensionError::new(
                "extension.widget",
                "Das Widget-Feld hat keine Spalte.",
            ));
        };
        if column.foreign_table.is_some() || column.primary_key {
            return Err(ExtensionError::new(
                "extension.widget",
                "Fremdschlüssel und Primärschlüssel behalten das eingebaute Widget.",
            ));
        }
        let text = matches!(
            widget.kind,
            WidgetKind::Text | WidgetKind::Textarea { .. } | WidgetKind::Email | WidgetKind::Url
        );
        if text && column.kind != ColumnKind::Text {
            return Err(ExtensionError::new(
                "extension.widget",
                "Text-Widgets passen nur auf Textspalten.",
            ));
        }
        if matches!(widget.kind, WidgetKind::Number { .. }) && column.kind != ColumnKind::BigInt {
            return Err(ExtensionError::new(
                "extension.widget",
                "Ein Zahlen-Widget passt nur auf eine Ganzzahlspalte.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn presentation(kind: WidgetKind) -> (&'static str, &'static str) {
    match kind {
        WidgetKind::Text => ("text", "text"),
        WidgetKind::Textarea { .. } => ("textarea", "text"),
        WidgetKind::Email => ("text", "email"),
        WidgetKind::Url => ("text", "url"),
        WidgetKind::Number { .. } => ("text", "number"),
        WidgetKind::Readonly => ("readonly", "text"),
    }
}

pub(crate) fn enforce(
    widgets: &[AdminWidget],
    model_key: &str,
    object: &Map<String, Value>,
) -> Result<(), String> {
    for widget in widgets
        .iter()
        .filter(|widget| widget.model_key == model_key)
    {
        let Some(value) = object.get(widget.field) else {
            continue;
        };
        if matches!(widget.kind, WidgetKind::Readonly) {
            return Err(format!("field:{}: field is not writable", widget.field));
        }
        let Some(text) = value.as_str() else {
            if let WidgetKind::Number { min, max } = widget.kind {
                let Some(number) = value.as_i64() else {
                    return Err(format!("field:{}: value is not a number", widget.field));
                };
                bound(widget.field, number, min, max)?;
            }
            continue;
        };
        if text.is_empty() {
            continue;
        }
        match widget.kind {
            WidgetKind::Email if !is_email(text) => {
                return Err(format!("field:{}: value must be an email", widget.field));
            }
            WidgetKind::Url if !is_http_url(text) => {
                return Err(format!("field:{}: value must be an http url", widget.field));
            }
            WidgetKind::Textarea { max_length } if text.chars().count() > max_length => {
                return Err(format!(
                    "field:{}: value may have at most {max_length} characters",
                    widget.field
                ));
            }
            WidgetKind::Number { min, max } => {
                let number = text
                    .parse::<i64>()
                    .map_err(|_| format!("field:{}: value is not a number", widget.field))?;
                bound(widget.field, number, min, max)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn bound(field: &str, number: i64, min: Option<i64>, max: Option<i64>) -> Result<(), String> {
    if min.is_some_and(|min| number < min) || max.is_some_and(|max| number > max) {
        return Err(format!("field:{field}: value is outside the widget bounds"));
    }
    Ok(())
}
