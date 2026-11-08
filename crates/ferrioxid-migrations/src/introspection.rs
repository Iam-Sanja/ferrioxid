//! Reads an existing SQLite schema. Ferrioxid logical types are used for comparisons.

use crate::error::MigrationError;
use crate::snapshot::{LogicalType, OnDelete, SchemaSnapshot};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveColumn {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_sql: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveIndex {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveForeignKey {
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub on_delete: String,
    pub on_update: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveTable {
    pub name: String,
    pub columns: Vec<LiveColumn>,
    pub indexes: Vec<LiveIndex>,
    pub foreign_keys: Vec<LiveForeignKey>,
}

pub async fn introspect_sqlite(
    db: &impl ConnectionTrait,
) -> Result<Vec<LiveTable>, MigrationError> {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        return Err(MigrationError::new(
            "Schema introspection for baselining is implemented only for SQLite.".to_owned(),
        ));
    }
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        ))
        .await
        .map_err(|error| MigrationError::new(format!("Tabellenliste fehlgeschlagen: {error}")))?;
    let mut tables = Vec::new();
    for row in rows {
        let name: String = row.try_get("", "name").map_err(|error| {
            MigrationError::new(format!("Could not read table name: {error}"))
        })?;
        if name.starts_with("_ferrum_") {
            continue;
        }
        tables.push(LiveTable {
            columns: table_info(db, &name).await?,
            indexes: index_info(db, &name).await?,
            foreign_keys: foreign_key_info(db, &name).await?,
            name,
        });
    }
    Ok(tables)
}

async fn table_info(
    db: &impl ConnectionTrait,
    table: &str,
) -> Result<Vec<LiveColumn>, MigrationError> {
    if !crate::snapshot::is_identifier(table) {
        return Err(MigrationError::new(format!(
            "Invalid table name `{table}`."
        )));
    }
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("PRAGMA table_info(\"{table}\")"),
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!("PRAGMA table_info fehlgeschlagen: {error}"))
        })?;
    let mut columns = Vec::new();
    for row in rows {
        let notnull = read_i64(&row, "notnull")?;
        let primary = read_i64(&row, "pk")?;
        columns.push(LiveColumn {
            name: row.try_get("", "name").map_err(|error| {
                MigrationError::new(format!("Could not read column name: {error}"))
            })?,
            type_name: row.try_get("", "type").map_err(|error| {
                MigrationError::new(format!("Could not read column type: {error}"))
            })?,
            nullable: notnull == 0,
            primary_key: primary > 0,
            default_sql: row.try_get("", "dflt_value").unwrap_or(None),
        });
    }
    Ok(columns)
}

async fn index_info(
    db: &impl ConnectionTrait,
    table: &str,
) -> Result<Vec<LiveIndex>, MigrationError> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("PRAGMA index_list(\"{table}\")"),
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!("PRAGMA index_list fehlgeschlagen: {error}"))
        })?;
    let mut indexes = Vec::new();
    for row in rows {
        let name: String = row
            .try_get("", "name")
            .map_err(|error| MigrationError::new(format!("Could not read index name: {error}")))?;
        let origin: String = row.try_get("", "origin").unwrap_or_default();
        if origin == "pk" || name.starts_with("sqlite_autoindex_") {
            continue;
        }
        if !crate::snapshot::is_identifier(&name) {
            return Err(MigrationError::new(format!(
                "Invalid index name `{name}`."
            )));
        }
        let columns = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("PRAGMA index_info(\"{name}\")"),
            ))
            .await
            .map_err(|error| {
                MigrationError::new(format!("PRAGMA index_info fehlgeschlagen: {error}"))
            })?;
        let mut named = Vec::new();
        for column in columns {
            let column_name: Option<String> = column.try_get("", "name").unwrap_or(None);
            let Some(column_name) = column_name else {
                return Err(MigrationError::new(format!(
                    "Index `{name}` contains an expression and cannot be compared with the snapshot."
                )));
            };
            named.push(column_name);
        }
        indexes.push(LiveIndex {
            name,
            columns: named,
            unique: read_i64(&row, "unique")? != 0,
        });
    }
    Ok(indexes)
}

async fn foreign_key_info(
    db: &impl ConnectionTrait,
    table: &str,
) -> Result<Vec<LiveForeignKey>, MigrationError> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("PRAGMA foreign_key_list(\"{table}\")"),
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!("PRAGMA foreign_key_list fehlgeschlagen: {error}"))
        })?;
    let mut grouped: Vec<(i64, i64, LiveForeignKey)> = Vec::new();
    for row in rows {
        let id = read_i64(&row, "id")?;
        let seq = read_i64(&row, "seq")?;
        let referenced_table: String = row.try_get("", "table").map_err(|error| {
            MigrationError::new(format!("Could not read foreign-key table: {error}"))
        })?;
        let from: String = row.try_get("", "from").map_err(|error| {
            MigrationError::new(format!("Could not read foreign-key column: {error}"))
        })?;
        let to: String = row.try_get("", "to").map_err(|error| {
            MigrationError::new(format!("Could not read referenced column: {error}"))
        })?;
        let on_delete: String = row.try_get("", "on_delete").unwrap_or_default();
        let on_update: String = row.try_get("", "on_update").unwrap_or_default();
        if let Some(existing) = grouped.iter_mut().find(|item| item.0 == id) {
            existing.2.columns.push(from);
            existing.2.referenced_columns.push(to);
        } else {
            grouped.push((
                id,
                seq,
                LiveForeignKey {
                    columns: vec![from],
                    referenced_table,
                    referenced_columns: vec![to],
                    on_delete: on_delete.to_ascii_uppercase(),
                    on_update: on_update.to_ascii_uppercase(),
                },
            ));
        }
    }
    grouped.sort_by_key(|item| (item.0, item.1));
    Ok(grouped.into_iter().map(|item| item.2).collect())
}

fn read_i64(row: &sea_orm::QueryResult, column: &str) -> Result<i64, MigrationError> {
    row.try_get::<i64>("", column)
        .or_else(|_| row.try_get::<i32>("", column).map(i64::from))
        .map_err(|error| {
            MigrationError::new(format!("Could not read PRAGMA value `{column}`: {error}"))
        })
}

pub fn compare_snapshot(expected: &SchemaSnapshot, live: &[LiveTable]) -> Vec<String> {
    let mut differences = Vec::new();
    for table in &expected.tables {
        let Some(found) = live.iter().find(|candidate| candidate.name == table.name) else {
            differences.push(format!("Table `{}` is missing from the database.", table.name));
            continue;
        };
        for column in &table.columns {
            let Some(live_column) = found
                .columns
                .iter()
                .find(|candidate| candidate.name == column.name)
            else {
                differences.push(format!(
                    "Column `{}.{}` is missing from the database.",
                    table.name, column.name
                ));
                continue;
            };
            if !type_compatible(column.logical_type, &live_column.type_name) {
                differences.push(format!(
                    "Column `{}.{}` has SQL type `{}`; expected {:?}.",
                    table.name, column.name, live_column.type_name, column.logical_type
                ));
            }
            if live_column.nullable != column.nullable {
                differences.push(format!(
                    "Column `{}.{}` is nullable={}; expected {}.",
                    table.name, column.name, live_column.nullable, column.nullable
                ));
            }
            if live_column.primary_key != column.primary_key {
                differences.push(format!(
                    "Primary-key status of `{}.{}` is {}; expected {}.",
                    table.name, column.name, live_column.primary_key, column.primary_key
                ));
            }
            if !default_compatible(column.default, live_column.default_sql.as_deref()) {
                differences.push(format!(
                    "Default of `{}.{}` is {:?}; expected {:?}.",
                    table.name, column.name, live_column.default_sql, column.default
                ));
            }
        }
        for live_column in &found.columns {
            if !table
                .columns
                .iter()
                .any(|column| column.name == live_column.name)
            {
                differences.push(format!(
                    "Additional column `{}.{}` prevents baselining.",
                    table.name, live_column.name
                ));
            }
        }
        for unique in &table.unique_constraints {
            let matched = found.indexes.iter().any(|index| {
                index.unique && index.name == unique.name && index.columns == unique.columns
            });
            if !matched {
                differences.push(format!(
                    "Unique constraint `{}` on `{}` is missing from the database.",
                    unique.name, table.name
                ));
            }
        }
        for index in &table.indexes {
            let matched = found.indexes.iter().any(|live| {
                !live.unique && live.name == index.name && live.columns == index.columns
            });
            if !matched {
                differences.push(format!(
                    "Index `{}` on `{}` is missing from the database.",
                    index.name, table.name
                ));
            }
        }
        for live in &found.indexes {
            let expected_unique = table.unique_constraints.iter().any(|unique| {
                live.unique && unique.name == live.name && unique.columns == live.columns
            });
            let expected_index = table.indexes.iter().any(|index| {
                !live.unique && index.name == live.name && index.columns == live.columns
            });
            if !expected_unique && !expected_index {
                differences.push(format!(
                    "Additional index `{}` on `{}` prevents baselining.",
                    live.name, table.name
                ));
            }
        }
        for foreign_key in &table.foreign_keys {
            let expected_delete = on_delete_sql(foreign_key.on_delete);
            let matched = found.foreign_keys.iter().any(|live| {
                live.columns == foreign_key.columns
                    && live.referenced_table == foreign_key.referenced_table
                    && live.referenced_columns == foreign_key.referenced_columns
                    && live.on_delete == expected_delete
                    && live.on_update == "NO ACTION"
            });
            if !matched {
                differences.push(format!(
                    "Foreign key `{}` on `{}` is missing or differs.",
                    foreign_key.name, table.name
                ));
            }
        }
        if found.foreign_keys.len() != table.foreign_keys.len() {
            differences.push(format!(
                "Table `{}` has {} foreign keys; expected {}.",
                table.name,
                found.foreign_keys.len(),
                table.foreign_keys.len()
            ));
        }
    }
    for table in live {
        if !expected
            .tables
            .iter()
            .any(|candidate| candidate.name == table.name)
        {
            differences.push(format!(
                "Unbekannte Tabelle `{}` blockiert die Baseline.",
                table.name
            ));
        }
    }
    differences
}

fn on_delete_sql(action: OnDelete) -> &'static str {
    match action {
        OnDelete::Restrict => "RESTRICT",
        OnDelete::Cascade => "CASCADE",
        OnDelete::SetNull => "SET NULL",
    }
}

fn type_compatible(logical: LogicalType, sql_type: &str) -> bool {
    let normalized = sql_type.trim().to_ascii_lowercase();
    let integer = matches!(normalized.as_str(), "integer" | "int" | "bigint" | "int8");
    let text = matches!(normalized.as_str(), "text" | "clob" | "varchar" | "char");
    match logical {
        LogicalType::Uuid | LogicalType::String => text,
        LogicalType::I64 => integer,
        LogicalType::Bool => integer || normalized == "boolean" || normalized == "bool",
        LogicalType::Date => text || normalized == "date",
        LogicalType::Time => text || normalized == "time" || normalized.starts_with("time "),
        LogicalType::DateTime => {
            text || normalized.contains("timestamp") || normalized == "datetime"
        }
    }
}

fn default_compatible(expected: Option<bool>, actual: Option<&str>) -> bool {
    let actual = actual.map(normalize_default);
    match expected {
        None => actual.is_none(),
        Some(true) => matches!(actual.as_deref(), Some("1") | Some("true")),
        Some(false) => matches!(actual.as_deref(), Some("0") | Some("false")),
    }
}

fn normalize_default(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| {
            character == '(' || character == ')' || character == '\'' || character == '"'
        })
        .to_ascii_lowercase()
}
