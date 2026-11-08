//! Applies operations to a snapshot instead of assuming the automatic diff is the only valid operation list.

use crate::diff::MigrationOperation;
use crate::error::MigrationError;
use crate::snapshot::{is_identifier, SchemaSnapshot};
use ferrioxid_core::{foreign_key_name, secondary_index_name, unique_index_name};

pub fn apply_operations(
    from: &SchemaSnapshot,
    operations: &[MigrationOperation],
) -> Result<SchemaSnapshot, MigrationError> {
    let mut schema = from.clone();
    for operation in operations {
        apply_one(&mut schema, operation)?;
    }
    schema.validate()?;
    Ok(schema)
}

fn apply_one(
    schema: &mut SchemaSnapshot,
    operation: &MigrationOperation,
) -> Result<(), MigrationError> {
    match operation {
        MigrationOperation::CreateTable { table } => {
            if schema
                .tables
                .iter()
                .any(|candidate| candidate.name == table.name)
            {
                return Err(MigrationError::new(format!(
                    "Table `{}` already exists in the original schema.",
                    table.name
                )));
            }
            schema.tables.push(table.clone());
        }
        MigrationOperation::DropTable { name } => {
            let length = schema.tables.len();
            schema.tables.retain(|table| table.name != *name);
            if schema.tables.len() == length {
                return Err(MigrationError::new(format!(
                    "Table `{name}` is missing from the original schema."
                )));
            }
        }
        MigrationOperation::AddColumn {
            table,
            column,
            foreign_key,
        } => {
            let target = table_mut(schema, table)?;
            if target
                .columns
                .iter()
                .any(|candidate| candidate.name == column.name)
            {
                return Err(MigrationError::new(format!(
                    "Column `{table}.{}` already exists.",
                    column.name
                )));
            }
            target.columns.push(column.clone());
            if let Some(foreign_key) = foreign_key {
                target.foreign_keys.push(foreign_key.clone());
            }
        }
        MigrationOperation::DropColumn { table, name } => {
            let target = table_mut(schema, table)?;
            let length = target.columns.len();
            target.columns.retain(|column| column.name != *name);
            if target.columns.len() == length {
                return Err(MigrationError::new(format!(
                    "Column `{table}.{name}` is missing from the original schema."
                )));
            }
        }
        MigrationOperation::AlterColumn { table, from, to } => {
            if from.name != to.name {
                return Err(MigrationError::new(
                    "AlterColumn cannot change column names. Use RenameColumn.",
                ));
            }
            let target = table_mut(schema, table)?;
            let column = target
                .columns
                .iter_mut()
                .find(|candidate| candidate.name == from.name)
                .ok_or_else(|| {
                    MigrationError::new(format!("Column `{table}.{}` is missing.", from.name))
                })?;
            if column != from {
                return Err(MigrationError::new(format!(
                    "AlterColumn expects the exact original stored column for `{table}.{}`.",
                    from.name
                )));
            }
            *column = to.clone();
        }
        MigrationOperation::RenameColumn { table, from, to } => {
            require_ident(table)?;
            require_ident(from)?;
            require_ident(to)?;
            {
                let target = table_mut(schema, table)?;
                if target.columns.iter().any(|column| column.name == *to) {
                    return Err(MigrationError::new(format!(
                        "Target column `{table}.{to}` already exists."
                    )));
                }
                let column = target
                    .columns
                    .iter_mut()
                    .find(|column| column.name == *from)
                    .ok_or_else(|| {
                        MigrationError::new(format!("Source column `{table}.{from}` is missing."))
                    })?;
                column.name.clone_from(to);
                for unique in &mut target.unique_constraints {
                    rename_in_list(&mut unique.columns, from, to);
                }
                for index in &mut target.indexes {
                    rename_in_list(&mut index.columns, from, to);
                }
                for foreign_key in &mut target.foreign_keys {
                    rename_in_list(&mut foreign_key.columns, from, to);
                    if foreign_key.referenced_table == *table {
                        rename_in_list(&mut foreign_key.referenced_columns, from, to);
                    }
                }
            }
            for other in &mut schema.tables {
                if other.name == *table {
                    continue;
                }
                for foreign_key in &mut other.foreign_keys {
                    if foreign_key.referenced_table == *table {
                        rename_in_list(&mut foreign_key.referenced_columns, from, to);
                    }
                }
            }
            refresh_constraint_names(schema);
        }
        MigrationOperation::RenameTable { from, to } => {
            require_ident(from)?;
            require_ident(to)?;
            if schema.tables.iter().any(|table| table.name == *to) {
                return Err(MigrationError::new(format!(
                    "Target table `{to}` already exists."
                )));
            }
            let Some(position) = schema.tables.iter().position(|table| table.name == *from) else {
                return Err(MigrationError::new(format!(
                    "Source table `{from}` is missing."
                )));
            };
            schema.tables[position].name.clone_from(to);
            for table in &mut schema.tables {
                for foreign_key in &mut table.foreign_keys {
                    if foreign_key.referenced_table == *from {
                        foreign_key.referenced_table.clone_from(to);
                    }
                }
            }
            refresh_constraint_names(schema);
        }
        MigrationOperation::AddUniqueConstraint { table, constraint } => {
            table_mut(schema, table)?
                .unique_constraints
                .push(constraint.clone());
        }
        MigrationOperation::DropUniqueConstraint { table, name } => {
            let target = table_mut(schema, table)?;
            let length = target.unique_constraints.len();
            target.unique_constraints.retain(|item| item.name != *name);
            if target.unique_constraints.len() == length {
                return Err(MigrationError::new(format!(
                    "Unique constraint `{name}` is missing."
                )));
            }
        }
        MigrationOperation::AddIndex { table, index } => {
            table_mut(schema, table)?.indexes.push(index.clone());
        }
        MigrationOperation::DropIndex { table, name } => {
            let target = table_mut(schema, table)?;
            let length = target.indexes.len();
            target.indexes.retain(|item| item.name != *name);
            if target.indexes.len() == length {
                return Err(MigrationError::new(format!("Index `{name}` is missing.")));
            }
        }
        MigrationOperation::AddForeignKey { table, foreign_key } => {
            table_mut(schema, table)?
                .foreign_keys
                .push(foreign_key.clone());
        }
        MigrationOperation::DropForeignKey { table, name } => {
            let target = table_mut(schema, table)?;
            let length = target.foreign_keys.len();
            target.foreign_keys.retain(|item| item.name != *name);
            if target.foreign_keys.len() == length {
                return Err(MigrationError::new(format!(
                    "Foreign key `{name}` is missing."
                )));
            }
        }
    }
    Ok(())
}

fn refresh_constraint_names(schema: &mut SchemaSnapshot) {
    for table in &mut schema.tables {
        for unique in &mut table.unique_constraints {
            let columns = unique
                .columns
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            unique.name = unique_index_name(&table.name, &columns);
        }
        for index in &mut table.indexes {
            let columns = index.columns.iter().map(String::as_str).collect::<Vec<_>>();
            index.name = secondary_index_name(&table.name, &columns);
        }
        for foreign_key in &mut table.foreign_keys {
            if let (Some(column), Some(referenced)) = (
                foreign_key.columns.first(),
                foreign_key.referenced_columns.first(),
            ) {
                foreign_key.name = foreign_key_name(
                    &table.name,
                    column,
                    &foreign_key.referenced_table,
                    referenced,
                );
            }
        }
    }
}

fn rename_in_list(columns: &mut [String], from: &str, to: &str) {
    for column in columns {
        if column.as_str() == from {
            *column = to.to_owned();
        }
    }
}

fn table_mut<'a>(
    schema: &'a mut SchemaSnapshot,
    name: &str,
) -> Result<&'a mut crate::snapshot::TableSnapshot, MigrationError> {
    schema
        .tables
        .iter_mut()
        .find(|table| table.name == name)
        .ok_or_else(|| MigrationError::new(format!("Table `{name}` is missing from the schema.")))
}

fn require_ident(name: &str) -> Result<(), MigrationError> {
    if is_identifier(name) {
        Ok(())
    } else {
        Err(MigrationError::new(format!(
            "Invalid SQL identifier `{name}`."
        )))
    }
}
