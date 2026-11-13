//! Compares two verified schema snapshots.

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
        /// Wert nur für bestehende Zeilen. Kein dauerhafter Default und keine erfundenen Werte.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        backfill: Option<BackfillLiteral>,
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
        /// Vorhandener Schlüssel, damit `migrate down` ihn wieder anlegen kann.
        /// Alte Dateien ohne dieses Feld bleiben lesbar.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous: Option<ForeignKeySnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum BackfillLiteral {
    Bool(bool),
    I64(i64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnBackfill {
    pub table: String,
    pub column: String,
    pub value: BackfillLiteral,
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
    /// Safety reason: this change must not be written or executed as a migration.
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
            Self::DropForeignKey { table, name, .. } => format!("drop_foreign_key {table}.{name}"),
        }
    }
}

pub fn alter_is_supported(from: &ColumnSnapshot, to: &ColumnSnapshot) -> bool {
    if from.name != to.name || from.primary_key != to.primary_key {
        return false;
    }
    if from.logical_type == to.logical_type {
        if from.precision != to.precision
            || from.scale != to.scale
            || choice_values_changed(from, to)
        {
            return false;
        }
        return from.nullable != to.nullable || from.default != to.default;
    }
    matches!(
        (from.logical_type, to.logical_type),
        (LogicalType::String, LogicalType::I64)
            | (LogicalType::I64, LogicalType::String)
            | (LogicalType::String, LogicalType::Choice)
    )
}

fn choice_values_changed(from: &ColumnSnapshot, to: &ColumnSnapshot) -> bool {
    from.choices
        .iter()
        .map(|choice| choice.value.as_str())
        .ne(to.choices.iter().map(|choice| choice.value.as_str()))
}

pub fn plan_change(
    from: &SchemaSnapshot,
    to: &SchemaSnapshot,
) -> Result<MigrationPlan, MigrationError> {
    plan_change_with_backfills(from, to, &[])
}

pub fn plan_change_with_backfills(
    from: &SchemaSnapshot,
    to: &SchemaSnapshot,
    backfills: &[ColumnBackfill],
) -> Result<MigrationPlan, MigrationError> {
    from.validate()?;
    to.validate()?;
    if from.semantically_eq(to) {
        if !backfills.is_empty() {
            return Err(MigrationError::new(
                "Backfill was supplied, but the schema has no new required column. No migration was written and no values were invented.".to_owned(),
            ));
        }
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
    attach_backfills(&mut ordered, backfills)?;
    let risks = risks_for(&ordered);
    let refusal = refusal_for(&ordered);
    Ok(MigrationPlan {
        operations: ordered,
        risks,
        refusal,
    })
}

fn attach_backfills(
    operations: &mut [MigrationOperation],
    backfills: &[ColumnBackfill],
) -> Result<(), MigrationError> {
    let mut used = vec![false; backfills.len()];
    for operation in operations.iter_mut() {
        let MigrationOperation::AddColumn {
            table,
            column,
            backfill,
            ..
        } = operation
        else {
            continue;
        };
        let Some(index) = backfills
            .iter()
            .position(|item| item.table == *table && item.column == column.name)
        else {
            continue;
        };
        if used[index] {
            return Err(MigrationError::new(format!(
                "Backfill for `{}.{}` was supplied twice.",
                table, column.name
            )));
        }
        if column.nullable || column.default.is_some() {
            return Err(MigrationError::new(format!(
                "Backfill for `{}.{}` is unnecessary because the column is nullable or already has a default. No migration was written.",
                table, column.name
            )));
        }
        if !backfill_matches(column.logical_type, &backfills[index].value) {
            return Err(MigrationError::new(format!(
                "Backfill for `{}.{}` does not match {:?}. No value was invented.",
                table, column.name, column.logical_type
            )));
        }
        *backfill = Some(backfills[index].value.clone());
        used[index] = true;
    }
    for (item, was_used) in backfills.iter().zip(used) {
        if !was_used {
            return Err(MigrationError::new(format!(
                "Backfill for `{}.{}` does not match a new NOT NULL column without a default. No migration was written and no values were invented.",
                item.table, item.column
            )));
        }
    }
    Ok(())
}

pub fn backfill_matches(logical_type: LogicalType, value: &BackfillLiteral) -> bool {
    matches!(
        (logical_type, value),
        (LogicalType::Bool, BackfillLiteral::Bool(_))
            | (LogicalType::I64, BackfillLiteral::I64(_))
            | (
                LogicalType::String
                    | LogicalType::Uuid
                    | LogicalType::Date
                    | LogicalType::Time
                    | LogicalType::DateTime
                    | LogicalType::Decimal
                    | LogicalType::Choice
                    | LogicalType::Json,
                BackfillLiteral::Text(_),
            )
    )
}

pub fn backfill_literal(
    kind: ferrioxid_core::DatabaseKind,
    value: &BackfillLiteral,
) -> Result<String, MigrationError> {
    match value {
        BackfillLiteral::Bool(boolean) => Ok(match kind {
            ferrioxid_core::DatabaseKind::Postgres => {
                if *boolean { "TRUE" } else { "FALSE" }.to_owned()
            }
            ferrioxid_core::DatabaseKind::Sqlite => if *boolean { "1" } else { "0" }.to_owned(),
        }),
        BackfillLiteral::I64(number) => Ok(number.to_string()),
        BackfillLiteral::Text(text) => {
            if text.contains('\0') {
                return Err(MigrationError::new(
                    "A text backfill cannot contain a NUL byte.".to_owned(),
                ));
            }
            Ok(format!("'{}'", text.replace('\'', "''")))
        }
    }
}

pub fn backfill_literal_for(
    kind: ferrioxid_core::DatabaseKind,
    column: &crate::snapshot::ColumnSnapshot,
    value: &BackfillLiteral,
) -> Result<String, MigrationError> {
    match (column.logical_type, value) {
        (LogicalType::Decimal, BackfillLiteral::Text(text)) => {
            let precision = column.precision.unwrap_or(0);
            let scale = column.scale.unwrap_or(0);
            ferrioxid_core::decimal_sql_literal(
                matches!(kind, ferrioxid_core::DatabaseKind::Postgres),
                text,
                precision,
                scale,
            )
            .map_err(|error| MigrationError::new(error.message.to_owned()))
        }
        (LogicalType::Json, BackfillLiteral::Text(text)) => {
            let canonical = ferrioxid_core::canonical_json_text(text).map_err(|code| {
                MigrationError::new(format!(
                    "Backfill for `{}` is not JSON: {code}",
                    column.name
                ))
            })?;
            let literal = backfill_literal(kind, &BackfillLiteral::Text(canonical))?;
            if matches!(kind, ferrioxid_core::DatabaseKind::Postgres) {
                Ok(format!("{literal}::jsonb"))
            } else {
                Ok(literal)
            }
        }
        (LogicalType::Choice, BackfillLiteral::Text(text)) => {
            if !column
                .choices
                .iter()
                .any(|choice| choice.value == text.as_str())
            {
                return Err(MigrationError::new(format!(
                    "Backfill `{text}` is not a stored value of choice column `{}`.",
                    column.name
                )));
            }
            backfill_literal(kind, value)
        }
        _ => backfill_literal(kind, value),
    }
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
                    previous: Some(existing.clone()),
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
                    ..
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
                previous: Some(foreign_key.clone()),
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
                "Foreign keys form a cycle. No migration is written.".to_owned(),
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
        MigrationOperation::DropForeignKey { .. } => 2,
        MigrationOperation::AddForeignKey { .. } => 3,
        MigrationOperation::AlterColumn { .. }
        | MigrationOperation::RenameColumn { .. }
        | MigrationOperation::RenameTable { .. } => 4,
        MigrationOperation::DropIndex { .. } | MigrationOperation::DropUniqueConstraint { .. } => 5,
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
                backfill: None,
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
        && left.precision == right.precision
        && left.scale == right.scale
        && left
            .choices
            .iter()
            .map(|choice| choice.value.as_str())
            .eq(right.choices.iter().map(|choice| choice.value.as_str()))
}

fn risks_for(operations: &[MigrationOperation]) -> Vec<MigrationRisk> {
    operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| match operation {
            MigrationOperation::DropTable { name } => Some(MigrationRisk {
                operation_index: index,
                code: "drop_table".to_owned(),
                message: format!("Table `{name}` and its data would be deleted."),
            }),
            MigrationOperation::DropColumn { table, name } => Some(MigrationRisk {
                operation_index: index,
                code: "drop_column".to_owned(),
                message: format!("Column `{table}.{name}` and its values would be deleted."),
            }),
            MigrationOperation::AlterColumn { table, from, to } if alter_is_supported(from, to) => {
                Some(MigrationRisk {
                    operation_index: index,
                    code: "rebuild_column".to_owned(),
                    message: format!(
                        "Column `{table}.{}` is changed only if existing values can be converted without loss. The transaction is rolled back on failure.",
                        from.name
                    ),
                })
            }
            MigrationOperation::AlterColumn { table, from, .. } => Some(MigrationRisk {
                operation_index: index,
                code: "alter_column".to_owned(),
                message: format!(
                    "Column `{table}.{}` has no verified lossless conversion.",
                    from.name
                ),
            }),
            MigrationOperation::AddColumn {
                table,
                column,
                backfill,
                ..
            } if column.primary_key
                || (!column.nullable && column.default.is_none() && backfill.is_none()) =>
            {
                Some(MigrationRisk {
                    operation_index: index,
                    code: "unsafe_add_column".to_owned(),
                    message: format!(
                        "Column `{table}.{}` cannot safely populate existing rows. Supply an explicit backfill, or add it as nullable, fill the rows, and then set NOT NULL.",
                        column.name
                    ),
                })
            }
            MigrationOperation::AddColumn {
                table,
                column,
                backfill: Some(_),
                ..
            } => Some(MigrationRisk {
                operation_index: index,
                code: "backfill_column".to_owned(),
                message: format!(
                    "Existing rows in `{table}.{}` receive the explicit backfill. No other value is invented.",
                    column.name
                ),
            }),
            MigrationOperation::DropUniqueConstraint { table, name } => Some(MigrationRisk {
                operation_index: index,
                code: "drop_unique".to_owned(),
                message: format!(
                    "Unique constraint `{name}` on `{table}` will be removed. Existing rows remain, and future duplicates become possible."
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
                let detail = if from.logical_type == to.logical_type
                    && (from.precision != to.precision
                        || from.scale != to.scale
                        || choice_values_changed(from, to))
                {
                    "precision, scale, or stored choice values are not rewritten automatically. Add a new column and backfill it."
                } else {
                    "no verified lossless conversion exists."
                };
                reasons.push(format!(
                    "Change to `{table}.{}` ({:?} -> {:?}, nullable {} -> {}, default {:?} -> {:?}) is blocked: {detail}",
                    from.name, from.logical_type, to.logical_type, from.nullable, to.nullable, from.default, to.default
                ))
            }
            MigrationOperation::AddColumn { table, column, .. } if column.primary_key => reasons.push(format!(
                "Adding primary-key column `{table}.{}` is blocked.",
                column.name
            )),
            MigrationOperation::AddColumn {
                table,
                column,
                backfill,
                ..
            } if !column.nullable && column.default.is_none() && backfill.is_none() => {
                reasons.push(format!(
                    "NOT NULL without a default for `{table}.{}` is blocked because existing rows have no safe value. Supply an explicit backfill, or add the column as nullable, backfill existing rows, and then set NOT NULL. No value is invented.",
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
            MigrationOperation::AddForeignKey { .. } => {}
            MigrationOperation::DropForeignKey { .. } => {}
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
                "Table `{table}` loses {} and gains {}. This is not interpreted as a rename. Use `ferrioxid migrate rename-column` for an intentional rename.",
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
            "The plan deletes {} and creates {}. This is not interpreted as a rename. Use `ferrioxid migrate rename-table` or split the changes.",
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
            Some("A primary-key column cannot safely be added to an existing table.".into()),
        ),
        MigrationOperation::AddColumn {
            column, backfill, ..
        } if !column.nullable && column.default.is_none() && backfill.is_none() => (
            ApplyClass::Unsupported,
            Some(
                "NOT NULL without a default cannot safely populate existing rows. Supply an explicit backfill or use a nullable column first. No value is invented."
                    .into(),
            ),
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
                    Some(format!("Primary key `{table}.{name}` cannot be dropped.")),
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
                "Primary keys cannot be rebuilt.".to_owned()
            } else {
                format!(
                    "Conversion from {:?} to {:?} has no implemented lossless migration.",
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
        MigrationOperation::AddForeignKey { .. } => (ApplyClass::Safe, None),
        MigrationOperation::DropForeignKey { .. } => (ApplyClass::Safe, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{ColumnSnapshot, LogicalType, TableSnapshot};

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
            precision: None,
            scale: None,
            choices: Vec::new(),
        }
    }

    fn id_column() -> ColumnSnapshot {
        ColumnSnapshot {
            name: "id".to_owned(),
            logical_type: LogicalType::Uuid,
            primary_key: true,
            nullable: false,
            default: None,
            precision: None,
            scale: None,
            choices: Vec::new(),
        }
    }

    fn table(name: &str, columns: Vec<ColumnSnapshot>) -> SchemaSnapshot {
        SchemaSnapshot::from_tables(vec![TableSnapshot {
            name: name.to_owned(),
            columns,
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
        }])
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
        assert!(unsupported.refusal.as_ref().unwrap().contains("blocked"));

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
        assert!(refusal.contains("not interpreted as a rename"));
        assert!(refusal.contains("note"));
        assert!(refusal.contains("description"));
    }
}
