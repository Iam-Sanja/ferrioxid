//! Versionierte, deterministische Schema-Snapshots.

use crate::error::MigrationError;
use ferrum_core::{ColumnKind, ColumnSpec};
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalType {
    Uuid,
    String,
    I64,
    Bool,
}

impl From<ColumnKind> for LogicalType {
    fn from(kind: ColumnKind) -> Self {
        match kind {
            ColumnKind::Uuid => Self::Uuid,
            ColumnKind::Text => Self::String,
            ColumnKind::BigInt => Self::I64,
            ColumnKind::Boolean => Self::Bool,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSnapshot {
    pub name: String,
    pub columns: Vec<ColumnSnapshot>,
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
                .collect();
            tables.push(TableSnapshot {
                name: model.table.to_owned(),
                columns,
            });
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
        }
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
                .is_some_and(|candidate| columns_equivalent(&table.columns, &candidate.columns))
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
    }

    #[test]
    fn registered_models_sort_tables_but_keep_columns() {
        const ID: ColumnSpec = ColumnSpec {
            name: "id",
            kind: ColumnKind::Uuid,
            nullable: false,
            primary_key: true,
            default_bool: None,
        };
        const LABEL: ColumnSpec = ColumnSpec {
            name: "label",
            kind: ColumnKind::Text,
            nullable: false,
            primary_key: false,
            default_bool: None,
        };
        let left = SchemaSnapshot::from_registered(&[
            RegisteredModel {
                table: "zeta",
                columns: &[ID, LABEL],
            },
            RegisteredModel {
                table: "alpha",
                columns: &[ID, LABEL],
            },
        ])
        .unwrap();
        let right = SchemaSnapshot::from_registered(&[
            RegisteredModel {
                table: "alpha",
                columns: &[ID, LABEL],
            },
            RegisteredModel {
                table: "zeta",
                columns: &[ID, LABEL],
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
