//! Versionierte, deterministische Schema-Snapshots.

use crate::error::MigrationError;
use fexu_core::{
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
    Decimal,
    Choice,
    Json,
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
            ColumnKind::Decimal => Self::Decimal,
            ColumnKind::Choice => Self::Choice,
            ColumnKind::Json => Self::Json,
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
            LogicalType::Decimal => Self::Decimal,
            LogicalType::Choice => Self::Choice,
            LogicalType::Json => Self::Json,
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
    /// Nur bei `decimal`. Fehlt in älteren Snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u8>,
    /// Speicherwerte einer Choice-Spalte. Labels sind Anzeigetext und kein Schemaunterschied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<StoredChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredChoice {
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label_de: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label_en: String,
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
    /// Fexu setzt derzeit immer `no_action`.
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

/// Tabelle einer anderen installierten App. Diese App erzeugt sie nicht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalTableSnapshot {
    pub app: String,
    pub table: TableSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaSnapshot {
    pub format_version: u32,
    pub tables: Vec<TableSnapshot>,
    /// Direkte App-Abhängigkeiten. Leer bei bestehenden Dateien und dann nicht serialisiert.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Zieltabellen aus `depends_on`. Sie gehören nicht zu den Operationen dieser App.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_tables: Vec<ExternalTableSnapshot>,
}

/// Ein von der Anwendung explizit registriertes Modell.
pub struct RegisteredModel {
    pub table: &'static str,
    pub columns: &'static [ColumnSpec],
    pub constraints: &'static TableConstraints,
}

impl SchemaSnapshot {
    pub fn empty() -> Self {
        Self::from_tables(Vec::new())
    }

    pub fn from_tables(tables: Vec<TableSnapshot>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            tables,
            depends_on: Vec::new(),
            external_tables: Vec::new(),
        }
    }

    pub fn from_registered(models: &[RegisteredModel]) -> Result<Self, MigrationError> {
        let mut tables = Vec::with_capacity(models.len());
        for model in models {
            tables.push(table_from_registered(model)?);
        }
        tables.sort_by(|left, right| left.name.cmp(&right.name));
        let snapshot = Self::from_tables(tables);
        snapshot.validate_owned()?;
        Ok(snapshot)
    }

    pub fn from_json(text: &str) -> Result<Self, MigrationError> {
        let snapshot = Self::parse_json(text)?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Modellausgabe einer einzelnen App. Ziele aus anderen Apps prüft erst die App-Auflösung.
    pub fn from_model_json(text: &str) -> Result<Self, MigrationError> {
        let snapshot = Self::parse_json(text)?;
        snapshot.validate_owned()?;
        Ok(snapshot)
    }

    fn parse_json(text: &str) -> Result<Self, MigrationError> {
        serde_json::from_str(text)
            .map_err(|error| MigrationError::new(format!("Snapshot is not valid JSON: {error}")))
    }

    pub fn to_canonical_json(&self) -> Result<String, MigrationError> {
        serde_json::to_string(self)
            .map_err(|error| MigrationError::new(format!("Could not serialize snapshot: {error}")))
    }

    pub fn validate(&self) -> Result<(), MigrationError> {
        self.validate_inner(true)
    }

    /// Prüft die eigenen Tabellen. Ein Ziel aus einer anderen App darf hier noch offen sein.
    pub fn validate_owned(&self) -> Result<(), MigrationError> {
        self.validate_inner(false)
    }

    fn validate_inner(&self, require_targets: bool) -> Result<(), MigrationError> {
        if self.format_version != FORMAT_VERSION {
            return Err(MigrationError::new(format!(
                "Unknown snapshot version {}. Only {FORMAT_VERSION} is supported.",
                self.format_version
            )));
        }
        let mut seen_dependencies = Vec::new();
        for dependency in &self.depends_on {
            if !is_identifier(dependency) {
                return Err(MigrationError::new(format!(
                    "Invalid app dependency `{dependency}`."
                )));
            }
            if seen_dependencies.contains(&dependency.as_str()) {
                return Err(MigrationError::new(format!(
                    "App dependency `{dependency}` is declared twice."
                )));
            }
            seen_dependencies.push(dependency.as_str());
        }
        let mut table_names = Vec::new();
        for table in &self.tables {
            validate_table_shape(table, &mut table_names)?;
        }
        let mut external_names = Vec::new();
        for external in &self.external_tables {
            if !is_identifier(&external.app) {
                return Err(MigrationError::new(format!(
                    "Invalid owning app `{}` for external table `{}`.",
                    external.app, external.table.name
                )));
            }
            if !self
                .depends_on
                .iter()
                .any(|dependency| dependency == &external.app)
            {
                return Err(MigrationError::new(format!(
                    "External table `{}.{}` belongs to app `{}`, which is not listed in depends_on. Existing migration files were not changed.",
                    external.app, external.table.name, external.app
                )));
            }
            if external_names.contains(&external.table.name.as_str())
                || table_names.contains(&external.table.name.as_str())
            {
                return Err(MigrationError::new(format!(
                    "Table `{}` is ambiguous: it is declared both locally and by app `{}`.",
                    external.table.name, external.app
                )));
            }
            external_names.push(external.table.name.as_str());
            let mut ignored = Vec::new();
            validate_table_shape(&external.table, &mut ignored)?;
        }
        validate_foreign_keys(self, require_targets)?;
        Ok(())
    }

    /// Gleicher Tabelleninhalt. `depends_on` und externe Tabellen sind Katalogdaten und erzeugen keine Operation.
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
                    && candidate.precision == column.precision
                    && candidate.scale == column.scale
                    && candidate
                        .choices
                        .iter()
                        .map(|choice| choice.value.as_str())
                        .eq(column.choices.iter().map(|choice| choice.value.as_str()))
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
            precision: (column.kind == ColumnKind::Decimal).then_some(column.precision),
            scale: (column.kind == ColumnKind::Decimal).then_some(column.scale),
            choices: column
                .choices
                .iter()
                .map(|choice| StoredChoice {
                    value: choice.value.to_owned(),
                    label_de: choice.label_de.to_owned(),
                    label_en: choice.label_en.to_owned(),
                })
                .collect(),
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

fn validate_table_shape<'a>(
    table: &'a TableSnapshot,
    table_names: &mut Vec<&'a str>,
) -> Result<(), MigrationError> {
    if !is_identifier(&table.name) {
        return Err(MigrationError::new(format!(
            "Invalid table name `{}`. Only letters, digits, and underscores are allowed.",
            table.name
        )));
    }
    if table_names.contains(&table.name.as_str()) {
        return Err(MigrationError::new(format!(
            "Duplicate table name `{}`.",
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
                "Invalid column name `{}.{}`.",
                table.name, column.name
            )));
        }
        if column_names.contains(&column.name.as_str()) {
            return Err(MigrationError::new(format!(
                "Duplicate column name `{}.{}`.",
                table.name, column.name
            )));
        }
        column_names.push(column.name.as_str());
        if column.primary_key {
            primary_keys += 1;
            if column.nullable {
                return Err(MigrationError::new(format!(
                    "Primary key `{}.{}` cannot be nullable.",
                    table.name, column.name
                )));
            }
        }
        if column.default.is_some() && column.logical_type != LogicalType::Bool {
            return Err(MigrationError::new(format!(
                "Defaults are supported only for boolean columns (`{}.{}`).",
                table.name, column.name
            )));
        }
        if column.logical_type == LogicalType::Decimal {
            let precision = column.precision.unwrap_or(0);
            let scale = column.scale.unwrap_or(0);
            if fexu_core::validate_decimal_meta(precision, scale).is_err() {
                return Err(MigrationError::new(format!(
                    "Decimal column `{}.{}` needs precision 1..=28 and scale <= precision.",
                    table.name, column.name
                )));
            }
        } else if column.precision.is_some() || column.scale.is_some() {
            return Err(MigrationError::new(format!(
                "Only decimal columns carry precision and scale (`{}.{}`).",
                table.name, column.name
            )));
        }
        if column.logical_type == LogicalType::Choice {
            if column.choices.is_empty() {
                return Err(MigrationError::new(format!(
                    "Choice column `{}.{}` needs at least one stored value.",
                    table.name, column.name
                )));
            }
            let mut seen = Vec::new();
            for choice in &column.choices {
                if !is_choice_token(&choice.value) || seen.contains(&choice.value.as_str()) {
                    return Err(MigrationError::new(format!(
                        "Choice value `{}` on `{}.{}` must be a unique lowercase identifier.",
                        choice.value, table.name, column.name
                    )));
                }
                seen.push(choice.value.as_str());
            }
        } else if !column.choices.is_empty() {
            return Err(MigrationError::new(format!(
                "Only choice columns carry stored values (`{}.{}`).",
                table.name, column.name
            )));
        }
    }
    if primary_keys != 1 {
        return Err(MigrationError::new(format!(
            "Table `{}` requires exactly one primary key; found {primary_keys}.",
            table.name
        )));
    }
    validate_table_objects(table, &column_names)
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
                "Index `{}` duplicates a unique constraint and is not created twice.",
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
                "Index `{}` duplicates the primary key.",
                index.name
            )));
        }
    }
    for foreign_key in &table.foreign_keys {
        remember_name(table, &mut names, &foreign_key.name)?;
        require_columns(table, columns, &foreign_key.columns)?;
        if foreign_key.on_update != "no_action" {
            return Err(MigrationError::new(format!(
                "Foreign key `{}` supports only no_action for ON UPDATE.",
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
                        "SET NULL is not allowed for `{}.{}` because the column is NOT NULL.",
                        table.name, column_name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn referenced_table<'a>(schema: &'a SchemaSnapshot, name: &str) -> Option<&'a TableSnapshot> {
    schema
        .tables
        .iter()
        .find(|candidate| candidate.name == name)
        .or_else(|| {
            schema
                .external_tables
                .iter()
                .find(|candidate| candidate.table.name == name)
                .map(|candidate| &candidate.table)
        })
}

fn validate_foreign_keys(
    schema: &SchemaSnapshot,
    require_targets: bool,
) -> Result<(), MigrationError> {
    for table in &schema.tables {
        for foreign_key in &table.foreign_keys {
            let Some(referenced) = referenced_table(schema, &foreign_key.referenced_table) else {
                if require_targets {
                    return Err(MigrationError::new(format!(
                        "Foreign key `{}` references unknown table `{}`. Register that table in this app, or declare depends_on for the installed app that owns it. Existing migration files were not changed.",
                        foreign_key.name, foreign_key.referenced_table
                    )));
                }
                continue;
            };
            if let Some(external) = schema
                .external_tables
                .iter()
                .find(|candidate| candidate.table.name == foreign_key.referenced_table)
            {
                if !schema
                    .depends_on
                    .iter()
                    .any(|dependency| dependency == &external.app)
                {
                    return Err(MigrationError::new(format!(
                        "Foreign key `{}` references `{}` in app `{}`, but that app is not declared in depends_on.",
                        foreign_key.name, foreign_key.referenced_table, external.app
                    )));
                }
            }
            if foreign_key.columns.len() != foreign_key.referenced_columns.len()
                || foreign_key.columns.len() != 1
            {
                return Err(MigrationError::new(format!(
                    "Foreign key `{}` must reference exactly one column.",
                    foreign_key.name
                )));
            }
            let referenced_column = referenced
                .columns
                .iter()
                .find(|column| column.name == foreign_key.referenced_columns[0])
                .ok_or_else(|| {
                    MigrationError::new(format!(
                        "Foreign key `{}` references unknown column `{}.{}`.",
                        foreign_key.name, referenced.name, foreign_key.referenced_columns[0]
                    ))
                })?;
            let Some(local) = table
                .columns
                .iter()
                .find(|column| column.name == foreign_key.columns[0])
            else {
                return Err(MigrationError::new(format!(
                    "Foreign key `{}` names missing column `{}.{}`.",
                    foreign_key.name, table.name, foreign_key.columns[0]
                )));
            };
            if local.logical_type != referenced_column.logical_type {
                return Err(MigrationError::new(format!(
                    "Foreign key `{}` joins incompatible types {:?} and {:?}.",
                    foreign_key.name, local.logical_type, referenced_column.logical_type
                )));
            }
            let unique_target = referenced_column.primary_key
                || referenced.unique_constraints.iter().any(|unique| {
                    unique.columns.len() == 1 && unique.columns[0] == referenced_column.name
                });
            if !unique_target {
                return Err(MigrationError::new(format!(
                    "Foreign key `{}` requires a primary key or unique constraint on `{}.{}`.",
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
            "Invalid constraint name `{name}`."
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
                "Column `{column}` appears twice in a constraint on `{}`.",
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

fn is_choice_token(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(character) if character.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && value.len() <= 64
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
            precision: None,
            scale: None,
            choices: Vec::new(),
        }
    }

    fn sample() -> SchemaSnapshot {
        SchemaSnapshot::from_tables(vec![TableSnapshot {
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
                    precision: None,
                    scale: None,
                    choices: Vec::new(),
                },
            ],
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
        }])
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
        assert!(!first.contains("depends_on"));
        assert!(!first.contains("external_tables"));
        let legacy = r#"{"format_version":1,"tables":[{"name":"products","columns":[{"name":"id","type":"uuid","primary_key":true,"nullable":false},{"name":"name","type":"string","primary_key":false,"nullable":false}]}]}"#;
        let parsed = SchemaSnapshot::from_json(legacy).unwrap();
        assert!(parsed.tables[0].unique_constraints.is_empty());
        assert!(parsed.tables[0].indexes.is_empty());
        assert!(parsed.tables[0].foreign_keys.is_empty());
        assert!(parsed.depends_on.is_empty());
        assert_eq!(parsed.to_canonical_json().unwrap(), legacy);
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
            precision: 0,
            scale: 0,
            choices: &[],
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
            precision: 0,
            scale: 0,
            choices: &[],
        };
        let left = SchemaSnapshot::from_registered(&[
            RegisteredModel {
                table: "zeta",
                columns: &[ID, LABEL],
                constraints: &fexu_core::TableConstraints::EMPTY,
            },
            RegisteredModel {
                table: "alpha",
                columns: &[ID, LABEL],
                constraints: &fexu_core::TableConstraints::EMPTY,
            },
        ])
        .unwrap();
        let right = SchemaSnapshot::from_registered(&[
            RegisteredModel {
                table: "alpha",
                columns: &[ID, LABEL],
                constraints: &fexu_core::TableConstraints::EMPTY,
            },
            RegisteredModel {
                table: "zeta",
                columns: &[ID, LABEL],
                constraints: &fexu_core::TableConstraints::EMPTY,
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
            .contains("Duplicate table name"));

        let mut columns = sample();
        let duplicate = columns.tables[0].columns[1].clone();
        columns.tables[0].columns.push(duplicate);
        assert!(columns
            .validate()
            .unwrap_err()
            .0
            .contains("Duplicate column name"));

        let mut version = sample();
        version.format_version = 99;
        assert!(version
            .validate()
            .unwrap_err()
            .0
            .contains("Unknown snapshot version"));

        let mut ident = sample();
        ident.tables[0].name = "bad-name".to_owned();
        assert!(ident
            .validate()
            .unwrap_err()
            .0
            .contains("Invalid table name"));
    }

    #[test]
    fn registered_cross_app_target_stays_unresolved_until_dependencies_are_bound() {
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
            precision: 0,
            scale: 0,
            choices: &[],
        };
        const PRODUCT: ColumnSpec = ColumnSpec {
            name: "product_id",
            kind: ColumnKind::Uuid,
            nullable: false,
            primary_key: false,
            default_bool: None,
            unique: false,
            indexed: false,
            foreign_table: Some("products"),
            foreign_column: Some("id"),
            on_delete: Some(fexu_core::ReferentialAction::Restrict),
            precision: 0,
            scale: 0,
            choices: &[],
        };
        let owned = SchemaSnapshot::from_registered(&[RegisteredModel {
            table: "cart_items",
            columns: &[ID, PRODUCT],
            constraints: &fexu_core::TableConstraints::EMPTY,
        }])
        .unwrap();
        let error = owned.validate().unwrap_err().0;
        assert!(error.contains("depends_on"), "{error}");
        assert!(error.contains("products"), "{error}");
    }
}
