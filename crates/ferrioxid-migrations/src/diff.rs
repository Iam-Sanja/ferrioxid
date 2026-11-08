//! Vergleich zweier bestätigter Schema-Snapshots.

use crate::error::MigrationError;
use crate::snapshot::{
    ColumnSnapshot, ForeignKeySnapshot, IndexSnapshot, LogicalType, SchemaSnapshot, TableSnapshot,
    UniqueConstraintSnapshot,
};
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        foreign_key: Option<ForeignKeySnapshot>,
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
    RenameColumn {
        table: String,
        from: String,
        to: String,
    },
    RenameTable {
        from: String,
        to: String,
    },
    AddUniqueConstraint {
        table: String,
        constraint: UniqueConstraintSnapshot,
    },
    DropUniqueConstraint {
        table: String,
        name: String,
    },
    AddIndex {
        table: String,
        index: IndexSnapshot,
    },
    DropIndex {
        table: String,
        name: String,
    },
    AddForeignKey {
        table: String,
        foreign_key: ForeignKeySnapshot,
    },
    DropForeignKey {
        table: String,
        name: String,
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
            Self::AddColumn { table, column, .. } => format!("add_column {table}.{}", column.name),
            Self::DropColumn { table, name } => format!("drop_column {table}.{name}"),
            Self::AlterColumn { table, from, to } => {
                format!("alter_column {table}.{} -> {}", from.name, to.name)
            }
            Self::RenameColumn { table, from, to } => {
                format!("rename_column {table}.{from} -> {to}")
            }
            Self::RenameTable { from, to } => format!("rename_table {from} -> {to}"),
            Self::AddUniqueConstraint { table, constraint } => {
                format!("add_unique {table}.{}", constraint.name)
            }
            Self::DropUniqueConstraint { table, name } => format!("drop_unique {table}.{name}"),
            Self::AddIndex { table, index } => format!("add_index {table}.{}", index.name),
            Self::DropIndex { table, name } => format!("drop_index {table}.{name}"),
            Self::AddForeignKey { table, foreign_key } => {
                format!("add_foreign_key {table}.{}", foreign_key.name)
            }
            Self::DropForeignKey { table, name } => format!("drop_foreign_key {table}.{name}"),
        }
    }
}

pub fn alter_is_supported(from: &ColumnSnapshot, to: &ColumnSnapshot) -> bool {
    if from.name != to.name || from.primary_key != to.primary_key {
        return false;
    }
    if from.logical_type == to.logical_type {
        return from.nullable != to.nullable || from.default != to.default;
    }
    matches!(
        (from.logical_type, to.logical_type),
        (LogicalType::String, LogicalType::I64) | (LogicalType::I64, LogicalType::String)
    )
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

    let mut creates = Vec::new();
    let mut drops = Vec::new();
    for name in names {
        let previous = from.tables.iter().find(|table| table.name == name);
        let next = to.tables.iter().find(|table| table.name == name);
        match (previous, next) {
            (None, Some(table)) => creates.push(table.clone()),
            (Some(table), None) => drops.push(table.clone()),
            (Some(previous), Some(next)) => diff_table(&mut operations, previous, next),
            (None, None) => {}
        }
    }
    let mut ordered = Vec::new();
    for table in order_creates(creates)? {
        ordered.push(MigrationOperation::CreateTable { table });
    }
    operations.sort_by_key(operation_rank);
    ordered.append(&mut operations);
    for table in order_drops(drops)? {
        ordered.push(MigrationOperation::DropTable { name: table });
    }
    let risks = risks_for(&ordered);
    let refusal = refusal_for(&ordered);
    Ok(MigrationPlan {
        operations: ordered,
        risks,
        refusal,
    })
}

fn diff_table(
    operations: &mut Vec<MigrationOperation>,
    previous: &TableSnapshot,
    next: &TableSnapshot,
) {
    diff_columns(operations, previous, next);
    attach_new_foreign_keys(operations, previous, next);
    diff_named(
        operations,
        &previous.name,
        &previous.unique_constraints,
        &next.unique_constraints,
        |constraint| MigrationOperation::AddUniqueConstraint {
            table: next.name.clone(),
            constraint: constraint.clone(),
        },
        |name| MigrationOperation::DropUniqueConstraint {
            table: previous.name.clone(),
            name,
        },
    );
    diff_named(
        operations,
        &previous.name,
        &previous.indexes,
        &next.indexes,
        |index| MigrationOperation::AddIndex {
            table: next.name.clone(),
            index: index.clone(),
        },
        |name| MigrationOperation::DropIndex {
            table: previous.name.clone(),
            name,
        },
    );
}

fn diff_named<T>(
    operations: &mut Vec<MigrationOperation>,
    _table: &str,
    previous: &[T],
    next: &[T],
    add: impl Fn(&T) -> MigrationOperation,
    drop: impl Fn(String) -> MigrationOperation,
) where
    T: Clone + PartialEq + NamedObject,
{
    for item in next {
        match previous
            .iter()
            .find(|candidate| candidate.object_name() == item.object_name())
        {
            Some(existing) if existing == item => {}
            Some(_) => {
                operations.push(drop(item.object_name()));
                operations.push(add(item));
            }
            None => operations.push(add(item)),
        }
    }
    for item in previous {
        if !next
            .iter()
            .any(|candidate| candidate.object_name() == item.object_name())
        {
            operations.push(drop(item.object_name()));
        }
    }
}

trait NamedObject {
    fn object_name(&self) -> String;
}

impl NamedObject for UniqueConstraintSnapshot {
    fn object_name(&self) -> String {
        self.name.clone()
    }
}

impl NamedObject for IndexSnapshot {
    fn object_name(&self) -> String {
        self.name.clone()
    }
}

fn attach_new_foreign_keys(
    operations: &mut Vec<MigrationOperation>,
    previous: &TableSnapshot,
    next: &TableSnapshot,
) {
    for foreign_key in &next.foreign_keys {
        if let Some(existing) = previous
            .foreign_keys
            .iter()
            .find(|candidate| candidate.name == foreign_key.name)
        {
            if existing != foreign_key {
                operations.push(MigrationOperation::DropForeignKey {
                    table: previous.name.clone(),
                    name: foreign_key.name.clone(),
                });
                operations.push(MigrationOperation::AddForeignKey {
                    table: next.name.clone(),
                    foreign_key: foreign_key.clone(),
                });
            }
            continue;
        }
        let attached = foreign_key.columns.len() == 1
            && !previous
                .columns
                .iter()
                .any(|column| column.name == foreign_key.columns[0])
            && operations.iter_mut().any(|operation| match operation {
                MigrationOperation::AddColumn {
                    table,
                    column,
                    foreign_key: slot,
                } if table == &next.name
                    && column.name == foreign_key.columns[0]
                    && slot.is_none() =>
                {
                    *slot = Some(foreign_key.clone());
                    true
                }
                _ => false,
            });
        if !attached {
            operations.push(MigrationOperation::AddForeignKey {
                table: next.name.clone(),
                foreign_key: foreign_key.clone(),
            });
        }
    }
    for foreign_key in &previous.foreign_keys {
        if !next
            .foreign_keys
            .iter()
            .any(|candidate| candidate.name == foreign_key.name)
        {
            operations.push(MigrationOperation::DropForeignKey {
                table: previous.name.clone(),
                name: foreign_key.name.clone(),
            });
        }
    }
}

fn order_creates(tables: Vec<TableSnapshot>) -> Result<Vec<TableSnapshot>, MigrationError> {
    order_by_dependencies(tables, false)
}

fn order_drops(tables: Vec<TableSnapshot>) -> Result<Vec<String>, MigrationError> {
    Ok(order_by_dependencies(tables, true)?
        .into_iter()
        .map(|table| table.name)
        .collect())
}

fn order_by_dependencies(
    tables: Vec<TableSnapshot>,
    dependents_first: bool,
) -> Result<Vec<TableSnapshot>, MigrationError> {
    let mut pending = tables;
    let mut ordered = Vec::new();
    while !pending.is_empty() {
        let ready = pending.iter().position(|table| {
            table.foreign_keys.iter().all(|foreign_key| {
                let referenced_in_batch = pending
                    .iter()
                    .any(|candidate| candidate.name == foreign_key.referenced_table);
                let self_reference = foreign_key.referenced_table == table.name;
                !referenced_in_batch || self_reference
            })
        });
        let Some(index) = ready else {
            return Err(MigrationError::new(
                "Fremdschlüssel bilden einen Zyklus. Die Migration wird nicht geschrieben."
                    .to_owned(),
            ));
        };
        let table = pending.remove(index);
        if dependents_first {
            ordered.insert(0, table);
        } else {
            ordered.push(table);
        }
    }
    Ok(ordered)
}

fn operation_rank(operation: &MigrationOperation) -> u8 {
    match operation {
        MigrationOperation::AddColumn { .. } => 1,
        MigrationOperation::AddUniqueConstraint { .. } | MigrationOperation::AddIndex { .. } => 2,
        MigrationOperation::AddForeignKey { .. } => 3,
        MigrationOperation::AlterColumn { .. }
        | MigrationOperation::RenameColumn { .. }
        | MigrationOperation::RenameTable { .. } => 4,
        MigrationOperation::DropForeignKey { .. }
        | MigrationOperation::DropIndex { .. }
        | MigrationOperation::DropUniqueConstraint { .. } => 5,
        MigrationOperation::DropColumn { .. } => 6,
        MigrationOperation::CreateTable { .. } | MigrationOperation::DropTable { .. } => 0,
    }
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
                foreign_key: None,
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
            MigrationOperation::AlterColumn { table, from, to } if alter_is_supported(from, to) => {
                Some(MigrationRisk {
                    operation_index: index,
                    code: "rebuild_column".to_owned(),
                    message: format!(
                        "Spalte `{table}.{}` wird nur geändert, wenn vorhandene Werte verlustfrei übernommen werden können. Bei einem Fehler wird die Transaktion zurückgerollt.",
                        from.name
                    ),
                })
            }
            MigrationOperation::AlterColumn { table, from, .. } => Some(MigrationRisk {
                operation_index: index,
                code: "alter_column".to_owned(),
                message: format!(
                    "Spalte `{table}.{}` hat keine geprüfte verlustfreie Änderung.",
                    from.name
                ),
            }),
            MigrationOperation::AddColumn { table, column, .. }
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
            MigrationOperation::DropUniqueConstraint { table, name } => Some(MigrationRisk {
                operation_index: index,
                code: "drop_unique".to_owned(),
                message: format!(
                    "Unique-Constraint `{name}` auf `{table}` wird entfernt. Vorhandene Zeilen bleiben, neue Duplikate werden danach zugelassen."
                ),
            }),
            _ => None,
        })
        .collect()
}

fn refusal_for(operations: &[MigrationOperation]) -> Option<String> {
    let mut reasons = Vec::new();
    for operation in operations {
        match operation {
            MigrationOperation::AlterColumn { table, from, to } if !alter_is_supported(from, to) => {
                reasons.push(format!(
                    "Änderung an `{table}.{}` ({:?} -> {:?}, nullable {} -> {}, default {:?} -> {:?}) wird blockiert. Es gibt keine geprüfte verlustfreie Übernahme.",
                    from.name, from.logical_type, to.logical_type, from.nullable, to.nullable, from.default, to.default
                ))
            }
            MigrationOperation::AddColumn { table, column, .. } if column.primary_key => reasons.push(format!(
                "Neue Primärschlüsselspalte `{table}.{}` wird blockiert.",
                column.name
            )),
            MigrationOperation::AddColumn { table, column, .. }
                if !column.nullable && column.default.is_none() =>
            {
                reasons.push(format!(
                    "NOT NULL ohne Default für `{table}.{}` wird blockiert, weil bestehende Zeilen keinen sicheren Wert bekommen.",
                    column.name
                ))
            }
            MigrationOperation::DropColumn { .. }
            | MigrationOperation::CreateTable { .. }
            | MigrationOperation::DropTable { .. }
            | MigrationOperation::AddColumn { .. }
            | MigrationOperation::AlterColumn { .. }
            | MigrationOperation::RenameColumn { .. }
            | MigrationOperation::RenameTable { .. }
            | MigrationOperation::AddUniqueConstraint { .. }
            | MigrationOperation::DropUniqueConstraint { .. }
            | MigrationOperation::AddIndex { .. }
            | MigrationOperation::DropIndex { .. } => {}
            MigrationOperation::AddForeignKey { table, foreign_key } => reasons.push(format!(
                "Fremdschlüssel `{}` auf `{table}` kann nicht an eine bestehende Spalte angefügt werden. SQLite unterstützt das nicht ohne Tabellenumbau, und dieser Umbau wird für Fremdschlüssel nicht ausgeführt.",
                foreign_key.name
            )),
            MigrationOperation::DropForeignKey { table, name } => reasons.push(format!(
                "Fremdschlüssel `{name}` auf `{table}` kann nicht entfernt oder in seiner Löschregel geändert werden. SQLite bräuchte dafür einen Tabellenumbau, der für Fremdschlüssel blockiert bleibt."
            )),
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
                    ..
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
                "Tabelle `{table}` verliert {} und erhält {}. Das wird nicht als Umbenennung gedeutet. Verwende `ferrioxid migrate rename-column`, wenn genau diese Spalte umbenannt werden soll.",
                removed.join(", "),
                added.join(", ")
            ));
        }
    }

    let dropped_tables: Vec<&str> = operations
        .iter()
        .filter_map(|operation| match operation {
            MigrationOperation::DropTable { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let created_tables: Vec<&str> = operations
        .iter()
        .filter_map(|operation| match operation {
            MigrationOperation::CreateTable { table } => Some(table.name.as_str()),
            _ => None,
        })
        .collect();
    if !dropped_tables.is_empty() && !created_tables.is_empty() {
        reasons.push(format!(
            "Der Plan löscht {} und erzeugt {}. Das wird nicht als Umbenennung gedeutet. Verwende `ferrioxid migrate rename-table` oder trenne die Änderungen.",
            dropped_tables.join(", "),
            created_tables.join(", ")
        ));
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
            Some(
                "Eine Primärschlüsselspalte kann nicht sicher nachträglich angelegt werden.".into(),
            ),
        ),
        MigrationOperation::AddColumn { column, .. }
            if !column.nullable && column.default.is_none() =>
        {
            (
                ApplyClass::Unsupported,
                Some("NOT NULL ohne Default kann bestehende Zeilen nicht füllen.".into()),
            )
        }
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
                        "Primärschlüssel `{table}.{name}` kann nicht gelöscht werden."
                    )),
                )
            } else {
                (ApplyClass::Destructive, None)
            }
        }
        MigrationOperation::AlterColumn { from, to, .. } if alter_is_supported(from, to) => {
            (ApplyClass::Safe, None)
        }
        MigrationOperation::AlterColumn { from, to, .. } => (
            ApplyClass::Unsupported,
            Some(if from.primary_key != to.primary_key {
                "Primärschlüssel können nicht umgebaut werden.".to_owned()
            } else {
                format!(
                    "Änderung von {:?} nach {:?} ist nicht als verlustfreie Übernahme implementiert.",
                    from.logical_type, to.logical_type
                )
            }),
        ),
        MigrationOperation::RenameColumn { .. } | MigrationOperation::RenameTable { .. } => {
            (ApplyClass::Safe, None)
        }
        MigrationOperation::AddUniqueConstraint { .. }
        | MigrationOperation::AddIndex { .. }
        | MigrationOperation::DropIndex { .. } => (ApplyClass::Safe, None),
        MigrationOperation::DropUniqueConstraint { .. } => (ApplyClass::Safe, None),
        MigrationOperation::AddForeignKey { .. } => (
            ApplyClass::Unsupported,
            Some(
                "Ein Fremdschlüssel auf einer bestehenden Spalte wird nicht ausgeführt."
                    .to_owned(),
            ),
        ),
        MigrationOperation::DropForeignKey { .. } => (
            ApplyClass::Unsupported,
            Some(
                "Ein vorhandener Fremdschlüssel wird nicht entfernt und seine Löschregel nicht geändert."
                    .to_owned(),
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
                unique_constraints: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
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
        assert!(typed.refusal.is_none());
        assert_eq!(typed.risks[0].code, "rebuild_column");

        let mut default_changed = initial.clone();
        default_changed.tables[0].columns[2].default = Some(false);
        let defaults = plan_change(&initial, &default_changed).unwrap();
        assert!(defaults.refusal.is_none());
        assert!(matches!(
            defaults.operations.as_slice(),
            [MigrationOperation::AlterColumn { .. }]
        ));

        let mut blocked = initial.clone();
        blocked.tables[0].columns[2].logical_type = LogicalType::String;
        blocked.tables[0].columns[2].default = None;
        let unsupported = plan_change(&initial, &blocked).unwrap();
        assert!(unsupported.refusal.as_ref().unwrap().contains("blockiert"));

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
