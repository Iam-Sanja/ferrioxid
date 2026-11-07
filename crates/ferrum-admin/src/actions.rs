//! Registrierte Sammelaktionen. Der HTTP-Name wählt nur einen bekannten Eintrag.

use ferrum_core::{AdminModelMeta, ColumnKind, ColumnSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionOperation {
    Delete,
    SetBool { column: &'static str, value: bool },
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
    match action.operation {
        ActionOperation::Delete => action.permission == "delete",
        ActionOperation::SetBool { column, .. } => {
            action.permission == "update"
                && columns.iter().any(|spec| {
                    spec.name == column && spec.kind == ColumnKind::Boolean && !spec.primary_key
                })
                && meta.fields.iter().any(|field| {
                    field.name == column && !field.hidden && !field.sensitive && !field.readonly
                })
        }
    }
}
