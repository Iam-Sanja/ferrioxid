//! Registrierte Sammelaktionen. Der HTTP-Name wählt nur einen bekannten Eintrag.

use fexu_core::{AdminModelMeta, ColumnKind, ColumnSpec};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::views::{valid_key, valid_label, ExtensionError};

#[derive(Clone)]
pub enum ActionOperation {
    Delete,
    SetBool { column: &'static str, value: bool },
    Custom(CustomAction),
}

impl std::fmt::Debug for ActionOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Delete => write!(formatter, "Delete"),
            Self::SetBool { column, value } => formatter
                .debug_struct("SetBool")
                .field("column", column)
                .field("value", value)
                .finish(),
            Self::Custom(_) => write!(formatter, "Custom"),
        }
    }
}

impl PartialEq for ActionOperation {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Delete, Self::Delete) => true,
            (
                Self::SetBool {
                    column: left,
                    value: left_value,
                },
                Self::SetBool {
                    column: right,
                    value: right_value,
                },
            ) => left == right && left_value == right_value,
            _ => false,
        }
    }
}

impl Eq for ActionOperation {}

#[derive(Clone)]
pub struct CustomAction {
    pub(crate) columns: &'static [&'static str],
    pub(crate) handler: Arc<dyn ActionHandler>,
}

#[derive(Clone, Debug)]
pub enum ActionValue {
    Text(String),
    Integer(i64),
    Boolean(bool),
}

#[derive(Clone, Debug)]
pub(crate) struct PlannedWrite {
    pub id: Uuid,
    pub column: &'static str,
    pub value: ActionValue,
}

/// Geprüfte Auswahl einer atomaren Aktion. Der Handler erhält keine Datenbankverbindung.
pub struct ActionContext<'a> {
    allowed: &'static [&'static str],
    specs: &'a [ColumnSpec],
    meta: &'a AdminModelMeta,
    records: &'a [Value],
    planned: Vec<PlannedWrite>,
}

impl<'a> ActionContext<'a> {
    pub(crate) fn prepare(
        allowed: &'static [&'static str],
        specs: &'a [ColumnSpec],
        meta: &'a AdminModelMeta,
        records: &'a [Value],
    ) -> Self {
        Self {
            allowed,
            specs,
            meta,
            records,
            planned: Vec::new(),
        }
    }

    pub fn records(&self) -> &'a [Value] {
        self.records
    }

    pub fn ids(&self) -> Vec<Uuid> {
        self.records
            .iter()
            .filter_map(|record| {
                record
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(|text| Uuid::parse_str(text).ok())
            })
            .collect()
    }

    pub fn set_text(
        &mut self,
        id: Uuid,
        column: &'static str,
        value: impl Into<String>,
    ) -> Result<(), ActionError> {
        self.push(
            id,
            column,
            ActionValue::Text(value.into()),
            ColumnKind::Text,
        )
    }

    pub fn set_integer(
        &mut self,
        id: Uuid,
        column: &'static str,
        value: i64,
    ) -> Result<(), ActionError> {
        self.push(id, column, ActionValue::Integer(value), ColumnKind::BigInt)
    }

    pub fn set_bool(
        &mut self,
        id: Uuid,
        column: &'static str,
        value: bool,
    ) -> Result<(), ActionError> {
        self.push(id, column, ActionValue::Boolean(value), ColumnKind::Boolean)
    }

    pub(crate) fn planned(&self) -> &[PlannedWrite] {
        &self.planned
    }

    fn push(
        &mut self,
        id: Uuid,
        column: &'static str,
        value: ActionValue,
        kind: ColumnKind,
    ) -> Result<(), ActionError> {
        if !self.ids().contains(&id) {
            return Err(ActionError::new(
                "extension.action",
                "record is outside the selection",
            ));
        }
        if !self.allowed.contains(&column) {
            return Err(ActionError::new(
                "extension.action",
                "column is not declared",
            ));
        }
        let allowed = self.specs.iter().any(|spec| {
            spec.name == column
                && (spec.kind == kind
                    || (kind == ColumnKind::Text && spec.kind == ColumnKind::Choice))
        }) && self.meta.fields.iter().any(|field| {
            field.name == column && !field.hidden && !field.sensitive && !field.readonly
        });
        if !allowed {
            return Err(ActionError::new(
                "extension.action",
                "column is not writable",
            ));
        }
        if let ActionValue::Text(text) = &value {
            if text.chars().count() > 4000 {
                return Err(ActionError::new("extension.action", "value is too long"));
            }
        }
        self.planned.push(PlannedWrite { id, column, value });
        Ok(())
    }
}

#[derive(Debug)]
pub struct ActionError {
    pub code: &'static str,
    pub detail: &'static str,
}

impl ActionError {
    pub const fn new(code: &'static str, detail: &'static str) -> Self {
        Self { code, detail }
    }
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ActionError {}

#[async_trait::async_trait]
pub trait ActionHandler: Send + Sync {
    async fn apply(&self, context: &mut ActionContext) -> Result<(), ActionError>;
}

#[derive(Debug, Clone)]
pub struct AdminAction {
    pub model_key: &'static str,
    pub name: &'static str,
    pub label_de: &'static str,
    pub label_en: &'static str,
    pub permission: &'static str,
    pub confirm: bool,
    pub max_ids: usize,
    pub operation: ActionOperation,
}

pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && name.len() <= 40
}

pub fn for_model(
    meta: &AdminModelMeta,
    columns: &[ColumnSpec],
    custom: &[AdminAction],
) -> Vec<AdminAction> {
    let mut actions = vec![AdminAction {
        model_key: meta.key,
        name: "delete",
        label_de: "Ausgewählte löschen",
        label_en: "Delete selected",
        permission: "delete",
        confirm: true,
        max_ids: 100,
        operation: ActionOperation::Delete,
    }];
    let active = columns.iter().any(|column| {
        column.name == "active"
            && column.kind == ColumnKind::Boolean
            && meta.fields.iter().any(|field| {
                field.name == "active" && !field.hidden && !field.sensitive && !field.readonly
            })
    });
    if active {
        actions.push(flag(
            meta.key,
            "activate",
            "Ausgewählte aktivieren",
            "Activate selected",
            true,
        ));
        actions.push(flag(
            meta.key,
            "deactivate",
            "Ausgewählte deaktivieren",
            "Deactivate selected",
            false,
        ));
    }
    for action in custom {
        if action.model_key == meta.key && accepts(action, meta, columns) {
            actions.push(action.clone());
        }
    }
    actions
}

fn flag(
    model_key: &'static str,
    name: &'static str,
    label_de: &'static str,
    label_en: &'static str,
    value: bool,
) -> AdminAction {
    AdminAction {
        model_key,
        name,
        label_de,
        label_en,
        permission: "update",
        confirm: true,
        max_ids: 100,
        operation: ActionOperation::SetBool {
            column: "active",
            value,
        },
    }
}

fn accepts(action: &AdminAction, meta: &AdminModelMeta, columns: &[ColumnSpec]) -> bool {
    if !valid_name(action.name) || matches!(action.name, "delete" | "activate" | "deactivate") {
        return false;
    }
    if action.max_ids == 0 || action.max_ids > 100 || !action.confirm {
        return false;
    }
    match &action.operation {
        ActionOperation::Delete => action.permission == "delete",
        ActionOperation::SetBool { column, .. } => {
            action.permission == "update"
                && columns.iter().any(|spec| {
                    spec.name == *column && spec.kind == ColumnKind::Boolean && !spec.primary_key
                })
                && meta.fields.iter().any(|field| {
                    field.name == *column && !field.hidden && !field.sensitive && !field.readonly
                })
        }
        ActionOperation::Custom(custom) => {
            action.permission == "update"
                && !custom.columns.is_empty()
                && custom
                    .columns
                    .iter()
                    .all(|column| writable(meta, columns, column))
        }
    }
}

pub(crate) fn writable(meta: &AdminModelMeta, columns: &[ColumnSpec], column: &str) -> bool {
    columns
        .iter()
        .any(|spec| spec.name == column && !spec.primary_key)
        && meta.fields.iter().any(|field| {
            field.name == column && !field.hidden && !field.sensitive && !field.readonly
        })
}

impl AdminAction {
    pub fn custom(
        model_key: &'static str,
        name: &'static str,
        handler: Arc<dyn ActionHandler>,
    ) -> Self {
        Self {
            model_key,
            name,
            label_de: name,
            label_en: name,
            permission: "update",
            confirm: true,
            max_ids: 100,
            operation: ActionOperation::Custom(CustomAction {
                columns: &[],
                handler,
            }),
        }
    }

    pub fn labels(mut self, german: &'static str, english: &'static str) -> Self {
        self.label_de = german;
        self.label_en = english;
        self
    }

    /// Spalten, die der Handler über den Aktionskontext setzen darf.
    pub fn writes(mut self, columns: &'static [&'static str]) -> Self {
        if let ActionOperation::Custom(custom) = &mut self.operation {
            custom.columns = columns;
        }
        self
    }
}

pub(crate) fn check_custom(action: &AdminAction) -> Result<(), ExtensionError> {
    if !valid_key(action.model_key) || !valid_name(action.name) {
        return Err(ExtensionError::new(
            "extension.action",
            "Aktionsname oder Modellname ist ungültig.",
        ));
    }
    if matches!(action.name, "delete" | "activate" | "deactivate") {
        return Err(ExtensionError::new(
            "extension.action",
            "Der Aktionsname ist reserviert.",
        ));
    }
    if !action.confirm || action.max_ids == 0 || action.max_ids > 100 {
        return Err(ExtensionError::new(
            "extension.action",
            "Eine eigene Aktion bestätigt höchstens 100 Datensätze.",
        ));
    }
    if !valid_label(action.label_de) || !valid_label(action.label_en) {
        return Err(ExtensionError::new(
            "extension.action",
            "Die Aktionsbeschriftung ist leer oder enthält Steuerzeichen.",
        ));
    }
    let ActionOperation::Custom(custom) = &action.operation else {
        return Err(ExtensionError::new(
            "extension.action",
            "try_action erwartet eine eigene Aktion.",
        ));
    };
    if action.permission != "update" || custom.columns.is_empty() {
        return Err(ExtensionError::new(
            "extension.action",
            "Eine eigene Aktion schreibt mit dem Recht update in deklarierte Spalten.",
        ));
    }
    if custom.columns.iter().any(|column| !valid_key(column)) {
        return Err(ExtensionError::new(
            "extension.action",
            "Eine Schreibspalte ist kein Feldname.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ActionContext, ActionHandler, ActionOperation, AdminAction};
    use fexu_core::{AdminFieldMeta, AdminModelMeta, ColumnKind, ColumnSpec};
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;

    struct MarkShipped;

    #[async_trait::async_trait]
    impl ActionHandler for MarkShipped {
        async fn apply(&self, context: &mut ActionContext) -> Result<(), super::ActionError> {
            for id in context.ids() {
                context.set_text(id, "status", "shipped")?;
            }
            Ok(())
        }
    }

    #[test]
    fn mark_shipped_is_an_action_and_only_writes_declared_columns() {
        let action = AdminAction::custom("orders", "mark_shipped", Arc::new(MarkShipped))
            .labels("Als versendet markieren", "Mark shipped")
            .writes(&["status"]);
        assert!(matches!(action.operation, ActionOperation::Custom(_)));
        assert_eq!(action.permission, "update");
        let id = Uuid::nil();
        let outside = Uuid::from_u128(7);
        let records = vec![json!({"id": id.to_string(), "status": "open"})];
        let columns = [ColumnSpec {
            name: "status",
            kind: ColumnKind::Text,
            nullable: false,
            primary_key: false,
            default_bool: None,
            unique: false,
            indexed: false,
            foreign_table: None,
            foreign_column: None,
            on_delete: None,
            precision: 0,
            scale: 0,
            choices: &[],
        }];
        let fields: &'static [AdminFieldMeta] = Box::leak(Box::new([AdminFieldMeta {
            name: "status",
            label: "Status",
            list_display: true,
            list_link: false,
            searchable: false,
            sortable: false,
            filterable: false,
            readonly: false,
            hidden: false,
            sensitive: false,
            fieldset: "",
        }]));
        let meta = AdminModelMeta {
            key: "orders",
            label: "Bestellungen",
            permission: Some("orders"),
            fields,
            ordering: None,
            per_page: 25,
            fieldsets: &[],
            inlines: &[],
            date_hierarchy: None,
        };
        let mut context = ActionContext::prepare(&["status"], &columns, &meta, &records);
        assert!(context.set_text(id, "status", "shipped").is_ok());
        assert!(context.set_text(id, "note", "secret").is_err());
        assert!(context.set_text(outside, "status", "shipped").is_err());
    }
}
