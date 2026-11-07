//! Versionierte, deterministische Schema-Snapshots.

use crate::error::MigrationError;
use ferrum_core::{
    foreign_key_name, secondary_index_name, unique_index_name, ColumnKind, ColumnSpec,
    ReferentialAction, TableConstraints,
};
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalType {
    Uuid,
    String,
    I64,
    Bool,
    Date,
    Time,
    DateTime,
}

impl From<ColumnKind> for LogicalType {
    fn from(kind: ColumnKind) -> Self {
        match kind {
            ColumnKind::Uuid => Self::Uuid,
            ColumnKind::Text => Self::String,
            ColumnKind::BigInt => Self::I64,
            ColumnKind::Boolean => Self::Bool,
            ColumnKind::Date => Self::Date,
            ColumnKind::Time => Self::Time,
            ColumnKind::DateTime => Self::DateTime,
        }
    }
}

impl From<LogicalType> for ColumnKind {
    fn from(value: LogicalType) -> Self {
        match value {
            LogicalType::Uuid => Self::Uuid,
            LogicalType::String => Self::Text,
            LogicalType::I64 => Self::BigInt,
            LogicalType::Bool => Self::Boolean,
            LogicalType::Date => Self::Date,
            LogicalType::Time => Self::Time,
            LogicalType::DateTime => Self::DateTime,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSnapshot {
    pub name: String,
    #[serde(rename = "type")]
    pub logical_type: LogicalType,
    pub primary_key: bool,
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnDelete {
    Restrict,
    Cascade,
    SetNull,
}

impl From<ReferentialAction> for OnDelete {
    fn from(action: ReferentialAction) -> Self {
        match action {
            ReferentialAction::Restrict => Self::Restrict,
            ReferentialAction::Cascade => Self::Cascade,
            ReferentialAction::SetNull => Self::SetNull,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UniqueConstraintSnapshot {
    pub name: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexSnapshot {
    pub name: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKeySnapshot {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub on_delete: OnDelete,
    /// Ferrum setzt derzeit immer `no_action`.
    pub on_update: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSnapshot {
    pub name: String,
    pub columns: Vec<ColumnSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unique_constraints: Vec<UniqueConstraintSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indexes: Vec<IndexSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub foreign_keys: Vec<ForeignKeySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaSnapshot {
    pub format_version: u32,
    pub tables: Vec<TableSnapshot>,
}

/// Ein von der Anwendung explizit registriertes Modell.
pub struct RegisteredModel {
    pub table: &'static str,
    pub columns: &'static [ColumnSpec],
    pub constraints: &'static TableConstraints,
}

impl SchemaSnapshot {
    pub fn empty() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            tables: Vec::new(),
        }
    }

    pub fn from_registered(models: &[RegisteredModel]) -> Result<Self, MigrationError> {
        let mut tables = Vec::with_capacity(models.len());
        for model in models {
            tables.push(table_from_registered(model)?);
        }
        tables.sort_by(|left, right| left.name.cmp(&right.name));
        let snapshot = Self {
            format_version: FORMAT_VERSION,
            tables,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn from_json(text: &str) -> Result<Self, MigrationError> {
        let snapshot: Self = serde_json::from_str(text).map_err(|error| {
            MigrationError::new(format!("Snapshot ist kein gültiges JSON: {error}"))
        })?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn to_canonical_json(&self) -> Result<String, MigrationError> {
        serde_json::to_string(self).map_err(|error| {
            MigrationError::new(format!(
                "Snapshot konnte nicht serialisiert werden: {error}"
            ))
        })
    }

    pub fn validate(&self) -> Result<(), MigrationError> {
        if self.format_version != FORMAT_VERSION {
            return Err(MigrationError::new(format!(
                "Unbekannte Snapshot-Version {}. Unterstützt wird nur {FORMAT_VERSION}.",
                self.format_version
            )));
        }
        let mut table_names = Vec::new();
        for table in &self.tables {
            if !is_identifier(&table.name) {
                return Err(MigrationError::new(format!(
                    "Ungültiger Tabellenname `{}`. Erlaubt sind Buchstaben, Ziffern und Unterstriche.",
                    table.name
                )));
            }
            if table_names.contains(&table.name.as_str()) {
                return Err(MigrationError::new(format!(
                    "Doppelter Tabellenname `{}`.",
                    table.name
                )));
            }
            table_names.push(table.name.as_str());
            if table.columns.is_empty() {
                return Err(MigrationError::new(format!(
                    "Tabelle `{}` hat keine Spalten.",
                    table.name
                )));
            }
            let mut column_names = Vec::new();
            let mut primary_keys = 0;
            for column in &table.columns {
                if !is_identifier(&column.name) {
                    return Err(MigrationError::new(format!(
                        "Ungültiger Spaltenname `{}.{}`.",
                        table.name, column.name
                    )));
                }
                if column_names.contains(&column.name.as_str()) {
                    return Err(MigrationError::new(format!(
                        "Doppelter Spaltenname `{}.{}`.",
                        table.name, column.name
                    )));
                }
                column_names.push(column.name.as_str());
                if column.primary_key {
                    primary_keys += 1;
                    if column.nullable {
                        return Err(MigrationError::new(format!(
                            "Primärschlüssel `{}.{}` darf nicht nullable sein.",
                            table.name, column.name
                        )));
                    }
                }
                if column.default.is_some() && column.logical_type != LogicalType::Bool {
                    return Err(MigrationError::new(format!(
                        "Ein Default ist nur für boolesche Spalten vorgesehen (`{}.{}`).",
                        table.name, column.name
                    )));
                }
            }
            if primary_keys != 1 {
                return Err(MigrationError::new(format!(
                    "Tabelle `{}` braucht genau einen Primärschlüssel, gefunden wurden {primary_keys}.",
                    table.name
                )));
            }
            validate_table_objects(table, &column_names)?;
        }
        validate_foreign_keys(self)?;
        Ok(())
    }

    /// Gleicher fachlicher Inhalt. Die Reihenfolge der Tabellen und Spalten spielt keine Rolle.
    pub fn semantically_eq(&self, other: &Self) -> bool {
        if self.format_version != other.format_version || self.tables.len() != other.tables.len() {
            return false;
        }
        self.tables.iter().all(|table| {
            other
                .tables
                .iter()
                .find(|candidate| candidate.name == table.name)
                .is_some_and(|candidate| {
                    columns_equivalent(&table.columns, &candidate.columns)
                        && same_objects(&table.unique_constraints, &candidate.unique_constraints)
                        && same_objects(&table.indexes, &candidate.indexes)
                        && same_objects(&table.foreign_keys, &candidate.foreign_keys)
                })
        })
    }
}

fn columns_equivalent(left: &[ColumnSnapshot], right: &[ColumnSnapshot]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter().all(|column| {
        right
            .iter()
            .find(|candidate| candidate.name == column.name)
            .is_some_and(|candidate| {
                candidate.logical_type == column.logical_type
                    && candidate.primary_key == column.primary_key
                    && candidate.nullable == column.nullable
                    && candidate.default == column.default
            })
    })
}

fn table_from_registered(model: &RegisteredModel) -> Result<TableSnapshot, MigrationError> {
    let columns = model
        .columns
        .iter()
        .map(|column| ColumnSnapshot {
            name: column.name.to_owned(),
            logical_type: column.kind.into(),
            primary_key: column.primary_key,
            nullable: column.nullable,
            default: column.default_bool,
        })
        .collect::<Vec<_>>();
    let mut unique_constraints = Vec::new();
    let mut indexes = Vec::new();
    let mut foreign_keys = Vec::new();
    for column in model.columns {
        if column.unique {
            unique_constraints.push(unique_snapshot(model.table, &[column.name]));
        }
        if column.indexed {
            indexes.push(index_snapshot(model.table, &[column.name]));
        }
        if let (Some(referenced_table), Some(referenced_column), Some(on_delete)) = (
            column.foreign_table,
            column.foreign_column,
            column.on_delete,
        ) {
            foreign_keys.push(ForeignKeySnapshot {
                name: foreign_key_name(
                    model.table,
                    column.name,
                    referenced_table,
                    referenced_column,
                ),
                columns: vec![column.name.to_owned()],
                referenced_table: referenced_table.to_owned(),
                referenced_columns: vec![referenced_column.to_owned()],
                on_delete: on_delete.into(),
                on_update: "no_action".to_owned(),
            });
        }
    }
    for group in model.constraints.unique_groups {
        unique_constraints.push(unique_snapshot(model.table, group));
    }
    for group in model.constraints.index_groups {
        indexes.push(index_snapshot(model.table, group));
    }
    unique_constraints.sort_by(|left, right| left.name.cmp(&right.name));
    indexes.sort_by(|left, right| left.name.cmp(&right.name));
    foreign_keys.sort_by(|left, right| left.name.cmp(&right.name));
    indexes.retain(|index| {
        !unique_constraints
            .iter()
            .any(|unique| unique.columns == index.columns)
    });
    Ok(TableSnapshot {
        name: model.table.to_owned(),
        columns,
        unique_constraints,
        indexes,
        foreign_keys,
    })
}

fn unique_snapshot(table: &str, columns: &[&str]) -> UniqueConstraintSnapshot {
    UniqueConstraintSnapshot {
        name: unique_index_name(table, columns),
        columns: columns.iter().map(|column| (*column).to_owned()).collect(),
    }
}

fn index_snapshot(table: &str, columns: &[&str]) -> IndexSnapshot {
    IndexSnapshot {
        name: secondary_index_name(table, columns),
        columns: columns.iter().map(|column| (*column).to_owned()).collect(),
    }
}

fn validate_table_objects(table: &TableSnapshot, columns: &[&str]) -> Result<(), MigrationError> {
    let mut names = Vec::new();
    for unique in &table.unique_constraints {
        remember_name(table, &mut names, &unique.name)?;
        require_columns(table, columns, &unique.columns)?;
    }
    for index in &table.indexes {
        remember_name(table, &mut names, &index.name)?;
        require_columns(table, columns, &index.columns)?;
        if table
            .unique_constraints
            .iter()
            .any(|unique| unique.columns == index.columns)
        {
            return Err(MigrationError::new(format!(
                "Index `{}` wiederholt den Unique-Constraint und wird nicht doppelt angelegt.",
                index.name
            )));
        }
        if index.columns.len() == 1
            && table
                .columns
                .iter()
                .any(|column| column.name == index.columns[0] && column.primary_key)
        {
            return Err(MigrationError::new(format!(
                "Index `{}` wiederholt den Primärschlüssel.",
                index.name
            )));
        }
    }
    for foreign_key in &table.foreign_keys {
        remember_name(table, &mut names, &foreign_key.name)?;
        require_columns(table, columns, &foreign_key.columns)?;
        if foreign_key.on_update != "no_action" {
            return Err(MigrationError::new(format!(
                "Fremdschlüssel `{}` darf für ON UPDATE nur no_action verwenden.",
                foreign_key.name
            )));
        }
        if foreign_key.on_delete == OnDelete::SetNull {
            for column_name in &foreign_key.columns {
                let nullable = table
                    .columns
                    .iter()
                    .find(|column| column.name == *column_name)
                    .is_some_and(|column| column.nullable);
                if !nullable {
                    return Err(MigrationError::new(format!(
                        "SET NULL ist für `{}.{}` unzulässig, weil die Spalte NOT NULL ist.",
                        table.name, column_name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_foreign_keys(schema: &SchemaSnapshot) -> Result<(), MigrationError> {
    for table in &schema.tables {
        for foreign_key in &table.foreign_keys {
            let referenced = schema
                .tables
                .iter()
                .find(|candidate| candidate.name == foreign_key.referenced_table)
                .ok_or_else(|| {
                    MigrationError::new(format!(
                        "Fremdschlüssel `{}` verweist auf die unbekannte Tabelle `{}`.",
                        foreign_key.name, foreign_key.referenced_table
                    ))
                })?;
            if foreign_key.columns.len() != foreign_key.referenced_columns.len()
                || foreign_key.columns.len() != 1
            {
                return Err(MigrationError::new(format!(
                    "Fremdschlüssel `{}` muss genau eine Spalte referenzieren.",
                    foreign_key.name
                )));
            }
            let referenced_column = referenced
                .columns
                .iter()
                .find(|column| column.name == foreign_key.referenced_columns[0])
                .ok_or_else(|| {
                    MigrationError::new(format!(
                        "Fremdschlüssel `{}` verweist auf die unbekannte Spalte `{}.{}`.",
                        foreign_key.name, referenced.name, foreign_key.referenced_columns[0]
                    ))
                })?;
            let Some(local) = table
                .columns
                .iter()
                .find(|column| column.name == foreign_key.columns[0])
            else {
                return Err(MigrationError::new(format!(
                    "Fremdschlüssel `{}` nennt die fehlende Spalte `{}.{}`.",
                    foreign_key.name, table.name, foreign_key.columns[0]
                )));
            };
            if local.logical_type != referenced_column.logical_type {
                return Err(MigrationError::new(format!(
                    "Fremdschlüssel `{}` verbindet {:?} mit {:?}.",
                    foreign_key.name, local.logical_type, referenced_column.logical_type
                )));
            }
            let unique_target = referenced_column.primary_key
                || referenced.unique_constraints.iter().any(|unique| {
                    unique.columns.len() == 1 && unique.columns[0] == referenced_column.name
                });
            if !unique_target {
                return Err(MigrationError::new(format!(
                    "Fremdschlüssel `{}` verlangt einen Primärschlüssel oder Unique-Constraint auf `{}.{}`.",
                    foreign_key.name, referenced.name, referenced_column.name
                )));
            }
        }
    }
    Ok(())
}

fn remember_name<'a>(
    table: &TableSnapshot,
    names: &mut Vec<&'a str>,
    name: &'a str,
) -> Result<(), MigrationError> {
    if !is_identifier(name) {
        return Err(MigrationError::new(format!(
            "Ungültiger Constraint-Name `{name}`."
        )));
    }
    if names.contains(&name) {
        return Err(MigrationError::new(format!(
            "Doppelter Constraint-Name `{name}` in `{}`.",
            table.name
        )));
    }
    names.push(name);
    Ok(())
}

fn require_columns(
    table: &TableSnapshot,
    columns: &[&str],
    used: &[String],
) -> Result<(), MigrationError> {
    if used.is_empty() {
        return Err(MigrationError::new(format!(
            "Constraint in `{}` hat keine Spalten.",
            table.name
        )));
    }
    let mut seen = Vec::new();
    for column in used {
        if !columns.contains(&column.as_str()) {
            return Err(MigrationError::new(format!(
                "Constraint in `{}` nennt die unbekannte Spalte `{column}`.",
                table.name
            )));
        }
        if seen.contains(&column.as_str()) {
            return Err(MigrationError::new(format!(
                "Spalte `{column}` ist in einem Constraint von `{}` doppelt.",
                table.name
            )));
        }
        seen.push(column.as_str());
    }
    Ok(())
}

fn same_objects<T: PartialEq>(left: &[T], right: &[T]) -> bool {
    left.len() == right.len() && left.iter().all(|item| right.contains(item))
}

pub fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(character) if character.is_ascii_alphabetic() || character == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(
        name: &str,
        logical_type: LogicalType,
        primary_key: bool,
        nullable: bool,
    ) -> ColumnSnapshot {
        ColumnSnapshot {
            name: name.to_owned(),
            logical_type,
            primary_key,
            nullable,
            default: None,
        }
    }

    fn sample() -> SchemaSnapshot {
        SchemaSnapshot {
            format_version: FORMAT_VERSION,
            tables: vec![TableSnapshot {
                name: "products".to_owned(),
                columns: vec![
                    column("id", LogicalType::Uuid, true, false),
                    column("name", LogicalType::String, false, false),
                    ColumnSnapshot {
                        name: "active".to_owned(),
                        logical_type: LogicalType::Bool,
                        primary_key: false,
                        nullable: false,
                        default: Some(true),
                    },
                ],
                unique_constraints: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
            }],
        }
    }

    #[test]
    fn canonical_json_is_stable_and_keeps_column_order() {
        let snapshot = sample();
        let first = snapshot.to_canonical_json().unwrap();
        let second = snapshot.to_canonical_json().unwrap();
        assert_eq!(first, second);
        let id = first.find("\"name\":\"id\"").unwrap();
        let name = first.find("\"name\":\"name\"").unwrap();
        let active = first.find("\"name\":\"active\"").unwrap();
        assert!(id < name && name < active);
        assert!(!first.contains("unique_constraints"));
        assert!(!first.contains("foreign_keys"));
        let legacy = r#"{"format_version":1,"tables":[{"name":"products","columns":[{"name":"id","type":"uuid","primary_key":true,"nullable":false},{"name":"name","type":"string","primary_key":false,"nullable":false}]}]}"#;
        let parsed = SchemaSnapshot::from_json(legacy).unwrap();
        assert!(parsed.tables[0].unique_constraints.is_empty());
        assert!(parsed.tables[0].indexes.is_empty());
        assert!(parsed.tables[0].foreign_keys.is_empty());
    }

    #[test]
    fn registered_models_sort_tables_but_keep_columns() {
        const ID: ColumnSpec = ColumnSpec {
            name: "id",
            kind: ColumnKind::Uuid,
            nullable: false,
            primary_key: true,
            default_bool: None,
            unique: false,
            indexed: false,
            foreign_table: None,
            foreign_column: None,
            on_delete: None,
        };
        const LABEL: ColumnSpec = ColumnSpec {
            name: "label",
            kind: ColumnKind::Text,
            nullable: false,
            primary_key: false,
            default_bool: None,
            unique: false,
            indexed: false,
            foreign_table: None,
            foreign_column: None,
            on_delete: None,
        };
        let left = SchemaSnapshot::from_registered(&[
            RegisteredModel {
                table: "zeta",
                columns: &[ID, LABEL],
                constraints: &ferrum_core::TableConstraints::EMPTY,
            },
            RegisteredModel {
                table: "alpha",
                columns: &[ID, LABEL],
                constraints: &ferrum_core::TableConstraints::EMPTY,
            },
        ])
        .unwrap();
        let right = SchemaSnapshot::from_registered(&[
            RegisteredModel {
                table: "alpha",
                columns: &[ID, LABEL],
                constraints: &ferrum_core::TableConstraints::EMPTY,
            },
            RegisteredModel {
                table: "zeta",
                columns: &[ID, LABEL],
                constraints: &ferrum_core::TableConstraints::EMPTY,
            },
        ])
        .unwrap();
        assert_eq!(
            left.to_canonical_json().unwrap(),
            right.to_canonical_json().unwrap()
        );
        assert_eq!(left.tables[0].name, "alpha");
        assert_eq!(left.tables[0].columns[0].name, "id");
        assert_eq!(left.tables[0].columns[1].name, "label");
    }

    #[test]
    fn duplicate_names_and_unknown_versions_are_rejected() {
        let mut snapshot = sample();
        snapshot.tables.push(snapshot.tables[0].clone());
        assert!(snapshot
            .validate()
            .unwrap_err()
            .0
            .contains("Doppelter Tabellenname"));

        let mut columns = sample();
        let duplicate = columns.tables[0].columns[1].clone();
        columns.tables[0].columns.push(duplicate);
        assert!(columns
            .validate()
            .unwrap_err()
            .0
            .contains("Doppelter Spaltenname"));

        let mut version = sample();
        version.format_version = 99;
        assert!(version
            .validate()
            .unwrap_err()
            .0
            .contains("Unbekannte Snapshot-Version"));

        let mut ident = sample();
        ident.tables[0].name = "bad-name".to_owned();
        assert!(ident
            .validate()
            .unwrap_err()
            .0
            .contains("Ungültiger Tabellenname"));
    }
}
