//! Registrierte Listenfilter. Die Spalte steht in der Registry, der Wert wird gebunden.

use ferrioxid_core::{AdminModelMeta, ColumnKind, ColumnSpec};
use sea_orm::Value as DbValue;

use crate::views::{valid_key, valid_label, ExtensionError};

#[derive(Clone, Debug)]
pub struct FilterOption {
    pub value: &'static str,
    pub label_de: &'static str,
    pub label_en: &'static str,
}

impl FilterOption {
    pub const fn new(value: &'static str, german: &'static str, english: &'static str) -> Self {
        Self {
            value,
            label_de: german,
            label_en: english,
        }
    }
}

#[derive(Clone, Debug)]
pub enum FilterKind {
    Equals {
        column: &'static str,
    },
    Choice {
        column: &'static str,
        options: &'static [FilterOption],
    },
}

#[derive(Clone, Debug)]
pub struct AdminFilter {
    pub model_key: &'static str,
    pub name: &'static str,
    pub label_de: &'static str,
    pub label_en: &'static str,
    pub kind: FilterKind,
}

impl AdminFilter {
    pub fn equals(model_key: &'static str, name: &'static str, column: &'static str) -> Self {
        Self {
            model_key,
            name,
            label_de: name,
            label_en: name,
            kind: FilterKind::Equals { column },
        }
    }

    pub fn choice(
        model_key: &'static str,
        name: &'static str,
        column: &'static str,
        options: &'static [FilterOption],
    ) -> Self {
        Self {
            model_key,
            name,
            label_de: name,
            label_en: name,
            kind: FilterKind::Choice { column, options },
        }
    }

    pub fn labels(mut self, german: &'static str, english: &'static str) -> Self {
        self.label_de = german;
        self.label_en = english;
        self
    }

    pub(crate) fn label(&self, locale: &str) -> &'static str {
        if locale == "de" {
            self.label_de
        } else {
            self.label_en
        }
    }

    pub(crate) fn column(&self) -> &'static str {
        match self.kind {
            FilterKind::Equals { column } | FilterKind::Choice { column, .. } => column,
        }
    }

    pub(crate) fn bind(
        &self,
        columns: &[ColumnSpec],
        value: &str,
    ) -> Result<(&'static str, DbValue), String> {
        let column = columns
            .iter()
            .find(|column| column.name == self.column())
            .ok_or_else(|| "filter field is not allowed".to_owned())?;
        if value.is_empty() {
            return Err("filter field is not allowed".into());
        }
        match &self.kind {
            FilterKind::Choice { options, .. } => {
                if !options.iter().any(|option| option.value == value) {
                    return Err("filter value is not allowed".into());
                }
            }
            FilterKind::Equals { .. } => {}
        }
        typed(column, value).map(|bound| (column.name, bound))
    }
}

pub(crate) fn check_filter(
    filter: &AdminFilter,
    meta: Option<&AdminModelMeta>,
    columns: Option<&[ColumnSpec]>,
) -> Result<(), ExtensionError> {
    if !valid_key(filter.model_key) || !valid_key(filter.name) {
        return Err(ExtensionError::new(
            "extension.filter",
            "Filtername oder Modellname ist ungültig.",
        ));
    }
    if !valid_label(filter.label_de) || !valid_label(filter.label_en) {
        return Err(ExtensionError::new(
            "extension.filter",
            "Die Filterbeschriftung ist leer oder enthält Steuerzeichen.",
        ));
    }
    if !valid_key(filter.column()) {
        return Err(ExtensionError::new(
            "extension.filter",
            "Die Filterspalte ist kein Modellfeld.",
        ));
    }
    match &filter.kind {
        FilterKind::Choice { options, .. } => {
            if options.is_empty() || options.len() > 20 {
                return Err(ExtensionError::new(
                    "extension.filter",
                    "Eine Auswahl braucht zwischen 1 und 20 feste Werte.",
                ));
            }
            if options.iter().any(|option| {
                option.value.is_empty()
                    || option.value.len() > 80
                    || !valid_label(option.label_de)
                    || !valid_label(option.label_en)
                    || option.value.contains('&')
                    || option.value.contains('=')
            }) {
                return Err(ExtensionError::new(
                    "extension.filter",
                    "Ein Auswahlwert ist leer oder enthält Steuerzeichen.",
                ));
            }
        }
        FilterKind::Equals { .. } => {}
    }
    if let (Some(meta), Some(columns)) = (meta, columns) {
        if meta.fields.iter().any(|field| field.name == filter.name) {
            return Err(ExtensionError::new(
                "extension.filter",
                "Der Filtername kollidiert mit einem Modellfeld.",
            ));
        }
        let Some(column) = columns.iter().find(|column| column.name == filter.column()) else {
            return Err(ExtensionError::new(
                "extension.filter",
                "Die Filterspalte gehört nicht zum Modell.",
            ));
        };
        if column.primary_key {
            return Err(ExtensionError::new(
                "extension.filter",
                "Ein Filter darf nicht auf dem Primärschlüssel liegen.",
            ));
        }
        let hidden = meta
            .fields
            .iter()
            .any(|field| field.name == column.name && (field.hidden || field.sensitive));
        if hidden {
            return Err(ExtensionError::new(
                "extension.filter",
                "Ein Filter darf kein verborgenes Feld öffnen.",
            ));
        }
    }
    Ok(())
}

fn typed(column: &ColumnSpec, value: &str) -> Result<DbValue, String> {
    match column.kind {
        ColumnKind::Text => Ok(DbValue::from(value.to_owned())),
        ColumnKind::BigInt => value
            .parse::<i64>()
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be an integer", column.name)),
        ColumnKind::Boolean => match value {
            "true" | "1" => Ok(DbValue::from(true)),
            "false" | "0" => Ok(DbValue::from(false)),
            _ => Err(format!("filter {} must be true or false", column.name)),
        },
        ColumnKind::Uuid => uuid::Uuid::parse_str(value)
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be a uuid", column.name)),
        ColumnKind::Date => ferrioxid_core::parse_date(value)
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be a date", column.name)),
        ColumnKind::Time => ferrioxid_core::parse_time(value)
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be a time", column.name)),
        ColumnKind::DateTime => ferrioxid_core::parse_datetime_utc(value)
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be a utc datetime", column.name)),
        ColumnKind::Decimal => decimal_filter(column, value),
        ColumnKind::Choice => choice_filter(column, value),
        ColumnKind::Json => Err("json columns have no portable filter".into()),
    }
}

fn decimal_filter(column: &ColumnSpec, value: &str) -> Result<DbValue, String> {
    let parsed = ferrioxid_core::parse_decimal(value, column.precision, column.scale)
        .map_err(|_| format!("filter {} must be a decimal", column.name))?;
    let bytes = ferrioxid_core::encode_decimal(parsed, column.precision, column.scale)
        .map_err(|_| format!("filter {} must be a decimal", column.name))?;
    Ok(DbValue::Bytes(Some(Box::new(bytes.to_vec()))))
}

fn choice_filter(column: &ColumnSpec, value: &str) -> Result<DbValue, String> {
    if !ferrioxid_core::choice_is_known(column.choices, value) {
        return Err(format!("filter {} is not a known choice", column.name));
    }
    Ok(DbValue::from(value.to_owned()))
}
