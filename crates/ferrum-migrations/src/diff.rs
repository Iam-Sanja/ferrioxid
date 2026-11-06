//! Vergleich zweier bestätigter Schema-Snapshots.

use crate::error::MigrationError;
use crate::snapshot::{ColumnSnapshot, SchemaSnapshot, TableSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MigrationOperation {
    CreateTable {
        table: TableSnapshot,
    },
    DropTable {
        name: String,
    },
    AddColumn {
        table: String,
        column: ColumnSnapshot,
    },
    DropColumn {
        table: String,
        name: String,
    },
    AlterColumn {
        table: String,
        from: ColumnSnapshot,
        to: ColumnSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationRisk {
    pub operation_index: usize,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlan {
    pub operations: Vec<MigrationOperation>,
    pub risks: Vec<MigrationRisk>,
    /// Gesetzte Begründung: die Änderung darf nicht als Migration geschrieben oder ausgeführt werden.
    pub refusal: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyClass {
    Safe,
    Destructive,
    Unsupported,
}

impl MigrationOperation {
    pub fn summary(&self) -> String {
        match self {
            Self::CreateTable { table } => format!("create_table {}", table.name),
            Self::DropTable { name } => format!("drop_table {name}"),
            Self::AddColumn { table, column } => format!("add_column {table}.{}", column.name),
            Self::DropColumn { table, name } => format!("drop_column {table}.{name}"),
            Self::AlterColumn { table, from, to } => {
                format!("alter_column {table}.{} -> {}", from.name, to.name)
            }
        }
    }
}

pub fn plan_change(
    from: &SchemaSnapshot,
    to: &SchemaSnapshot,
) -> Result<MigrationPlan, MigrationError> {
    from.validate()?;
    to.validate()?;
    if from.semantically_eq(to) {
        return Ok(MigrationPlan {
            operations: Vec::new(),
            risks: Vec::new(),
            refusal: None,
        });
    }

    let mut operations = Vec::new();
    let mut names: Vec<&str> = from
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .chain(to.tables.iter().map(|table| table.name.as_str()))
        .collect();
    names.sort_unstable();
    names.dedup();

    for name in names {
        let previous = from.tables.iter().find(|table| table.name == name);
        let next = to.tables.iter().find(|table| table.name == name);
        match (previous, next) {
            (None, Some(table)) => operations.push(MigrationOperation::CreateTable {
                table: table.clone(),
            }),
            (Some(table), None) => operations.push(MigrationOperation::DropTable {
                name: table.name.clone(),
            }),
            (Some(previous), Some(next)) => diff_columns(&mut operations, previous, next),
            (None, None) => {}
        }
    }

    let risks = risks_for(&operations);
    let refusal = refusal_for(&operations);
    Ok(MigrationPlan {
        operations,
        risks,
        refusal,
    })
}

fn diff_columns(
    operations: &mut Vec<MigrationOperation>,
    previous: &TableSnapshot,
    next: &TableSnapshot,
) {
    let mut names: Vec<&str> = previous
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .chain(next.columns.iter().map(|column| column.name.as_str()))
        .collect();
    names.sort_unstable();
    names.dedup();
    for name in names {
        let before = previous.columns.iter().find(|column| column.name == name);
        let after = next.columns.iter().find(|column| column.name == name);
        match (before, after) {
            (None, Some(column)) => operations.push(MigrationOperation::AddColumn {
                table: next.name.clone(),
                column: column.clone(),
            }),
            (Some(column), None) => operations.push(MigrationOperation::DropColumn {
                table: previous.name.clone(),
                name: column.name.clone(),
            }),
            (Some(before), Some(after)) if !same_column(before, after) => {
                operations.push(MigrationOperation::AlterColumn {
                    table: next.name.clone(),
                    from: before.clone(),
                    to: after.clone(),
                })
            }
            _ => {}
        }
    }
}

fn same_column(left: &ColumnSnapshot, right: &ColumnSnapshot) -> bool {
    left.logical_type == right.logical_type
        && left.primary_key == right.primary_key
        && left.nullable == right.nullable
        && left.default == right.default
}

fn risks_for(operations: &[MigrationOperation]) -> Vec<MigrationRisk> {
    operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| match operation {
            MigrationOperation::DropTable { name } => Some(MigrationRisk {
                operation_index: index,
                code: "drop_table".to_owned(),
                message: format!("Tabelle `{name}` würde samt Inhalt gelöscht."),
            }),
            MigrationOperation::DropColumn { table, name } => Some(MigrationRisk {
                operation_index: index,
                code: "drop_column".to_owned(),
                message: format!("Spalte `{table}.{name}` würde samt Werten gelöscht."),
            }),
            MigrationOperation::AlterColumn { table, from, .. } => Some(MigrationRisk {
                operation_index: index,
                code: "alter_column".to_owned(),
                message: format!(
                    "Spalte `{table}.{}` ändert Typ, Nullbarkeit, Default oder Primärschlüssel. Ein sicherer Umbau ist nicht implementiert.",
                    from.name
                ),
            }),
            MigrationOperation::AddColumn { table, column }
                if column.primary_key || (!column.nullable && column.default.is_none()) =>
            {
                Some(MigrationRisk {
                    operation_index: index,
                    code: "unsafe_add_column".to_owned(),
                    message: format!(
                        "Spalte `{table}.{}` kann bestehende Zeilen nicht sicher füllen.",
                        column.name
                    ),
                })
            }
            _ => None,
        })
        .collect()
}

fn refusal_for(operations: &[MigrationOperation]) -> Option<String> {
    let mut reasons = Vec::new();
    for operation in operations {
        match operation {
            MigrationOperation::AlterColumn { table, from, to } => reasons.push(format!(
                "Änderung an `{table}.{}` ({:?} -> {:?}, nullable {} -> {}, default {:?} -> {:?}) wird blockiert. SQLite kann diesen Umbau nicht sicher per ALTER ausführen, und ein Tabellen-Neubau ist nicht implementiert. PostgreSQL nutzt denselben logischen Plan und führt diese Änderung ebenfalls nicht aus.",
                from.name, from.logical_type, to.logical_type, from.nullable, to.nullable, from.default, to.default
            )),
            MigrationOperation::AddColumn { table, column } if column.primary_key => reasons.push(format!(
                "Neue Primärschlüsselspalte `{table}.{}` wird blockiert.",
                column.name
            )),
            MigrationOperation::AddColumn { table, column }
                if !column.nullable && column.default.is_none() =>
            {
                reasons.push(format!(
                    "NOT NULL ohne Default für `{table}.{}` wird blockiert, weil bestehende Zeilen keinen sicheren Wert bekommen.",
                    column.name
                ))
            }
            MigrationOperation::DropColumn { .. } | MigrationOperation::CreateTable { .. } | MigrationOperation::DropTable { .. } | MigrationOperation::AddColumn { .. } => {}
        }
    }

    let mut tables = Vec::new();
    for operation in operations {
        let table = match operation {
            MigrationOperation::AddColumn { table, .. }
            | MigrationOperation::DropColumn { table, .. } => table.as_str(),
            _ => continue,
        };
        if !tables.contains(&table) {
            tables.push(table);
        }
    }
    for table in tables {
        let added: Vec<&str> = operations
            .iter()
            .filter_map(|operation| match operation {
                MigrationOperation::AddColumn {
                    table: name,
                    column,
                } if name == table => Some(column.name.as_str()),
                _ => None,
            })
            .collect();
        let removed: Vec<&str> = operations
            .iter()
            .filter_map(|operation| match operation {
                MigrationOperation::DropColumn {
                    table: name,
                    name: column,
                } if name == table => Some(column.as_str()),
                _ => None,
            })
            .collect();
        if !added.is_empty() && !removed.is_empty() {
            reasons.push(format!(
                "Tabelle `{table}` verliert {} und erhält {}. Das wird nicht als Umbenennung gedeutet und nicht als Löschen plus Anlegen ausgeführt. Eine Rename-Operation ist nicht implementiert. Setze die Modelländerung so um, dass nur eine eindeutige, sichere Operation übrig bleibt.",
                removed.join(", "),
                added.join(", ")
            ));
        }
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons.join("\n"))
    }
}

pub fn classify(
    operation: &MigrationOperation,
    from: &SchemaSnapshot,
) -> (ApplyClass, Option<String>) {
    match operation {
        MigrationOperation::CreateTable { .. } => (ApplyClass::Safe, None),
        MigrationOperation::AddColumn { column, .. } if column.primary_key => (
            ApplyClass::Unsupported,
            Some("Eine Primärschlüsselspalte kann nicht sicher nachträglich angelegt werden.".into()),
        ),
        MigrationOperation::AddColumn { column, .. } if !column.nullable && column.default.is_none() => (
            ApplyClass::Unsupported,
            Some("NOT NULL ohne Default kann bestehende Zeilen nicht füllen.".into()),
        ),
        MigrationOperation::AddColumn { .. } => (ApplyClass::Safe, None),
        MigrationOperation::DropTable { .. } => (ApplyClass::Destructive, None),
        MigrationOperation::DropColumn { table, name } => {
            let primary = from
                .tables
                .iter()
                .find(|candidate| candidate.name == *table)
                .and_then(|candidate| candidate.columns.iter().find(|column| column.name == *name))
                .is_some_and(|column| column.primary_key);
            if primary {
                (
                    ApplyClass::Unsupported,
                    Some(format!(
                        "Primärschlüssel `{table}.{name}` kann nicht gelöscht werden. Ein Tabellen-Neubau ist nicht implementiert."
                    )),
                )
            } else {
                (ApplyClass::Destructive, None)
            }
        }
        MigrationOperation::AlterColumn { .. } => (
            ApplyClass::Unsupported,
            Some(
                "Typ-, Nullbarkeits- und Default-Änderungen sind nicht als sichere ALTER-Operation implementiert."
                    .into(),
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{ColumnSnapshot, LogicalType, TableSnapshot, FORMAT_VERSION};

    fn column(
        name: &str,
        logical_type: LogicalType,
        nullable: bool,
        default: Option<bool>,
    ) -> ColumnSnapshot {
        ColumnSnapshot {
            name: name.to_owned(),
            logical_type,
            primary_key: false,
            nullable,
            default,
        }
    }

    fn id_column() -> ColumnSnapshot {
        ColumnSnapshot {
            name: "id".to_owned(),
            logical_type: LogicalType::Uuid,
            primary_key: true,
            nullable: false,
            default: None,
        }
    }

    fn table(name: &str, columns: Vec<ColumnSnapshot>) -> SchemaSnapshot {
        SchemaSnapshot {
            format_version: FORMAT_VERSION,
            tables: vec![TableSnapshot {
                name: name.to_owned(),
                columns,
            }],
        }
    }

    fn products(extra: Vec<ColumnSnapshot>) -> SchemaSnapshot {
        let mut columns = vec![
            id_column(),
            column("name", LogicalType::String, false, None),
            column("active", LogicalType::Bool, false, Some(true)),
        ];
        columns.extend(extra);
        table("products", columns)
    }

    #[test]
    fn detects_create_add_drop_type_default_and_empty_plan() {
        let empty = SchemaSnapshot::empty();
        let initial = products(vec![]);
        let created = plan_change(&empty, &initial).unwrap();
        assert!(matches!(
            created.operations.as_slice(),
            [MigrationOperation::CreateTable { .. }]
        ));
        assert!(created.refusal.is_none());
        assert!(plan_change(&initial, &initial)
            .unwrap()
            .operations
            .is_empty());

        let mut added = products(vec![column("note", LogicalType::String, true, None)]);
        let add = plan_change(&initial, &added).unwrap();
        assert!(matches!(
            add.operations.as_slice(),
            [MigrationOperation::AddColumn { .. }]
        ));
        assert!(add.refusal.is_none());
        assert!(add.risks.is_empty());

        added.tables[0].columns.pop();
        let removed = plan_change(
            &products(vec![column("note", LogicalType::String, true, None)]),
            &initial,
        )
        .unwrap();
        assert!(matches!(
            removed.operations.as_slice(),
            [MigrationOperation::DropColumn { .. }]
        ));
        assert!(removed.refusal.is_none());
        assert_eq!(removed.risks[0].code, "drop_column");

        let mut changed = initial.clone();
        changed.tables[0].columns[1].logical_type = LogicalType::I64;
        let typed = plan_change(&initial, &changed).unwrap();
        assert!(typed.refusal.as_ref().unwrap().contains("blockiert"));

        let mut default_changed = initial.clone();
        default_changed.tables[0].columns[2].default = Some(false);
        let defaults = plan_change(&initial, &default_changed).unwrap();
        assert!(defaults.refusal.as_ref().unwrap().contains("blockiert"));

        let dropped = plan_change(&initial, &empty).unwrap();
        assert!(matches!(
            dropped.operations.as_slice(),
            [MigrationOperation::DropTable { .. }]
        ));
        assert_eq!(dropped.risks[0].code, "drop_table");
        assert!(dropped.refusal.is_none());
    }

    #[test]
    fn rename_like_changes_are_refused() {
        let before = products(vec![column("note", LogicalType::String, true, None)]);
        let after = products(vec![column("description", LogicalType::String, true, None)]);
        let plan = plan_change(&before, &after).unwrap();
        let refusal = plan.refusal.unwrap();
        assert!(refusal.contains("nicht als Umbenennung"));
        assert!(refusal.contains("note"));
        assert!(refusal.contains("description"));
    }
}
