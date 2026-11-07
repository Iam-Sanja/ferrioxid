//! Shared contracts, independent of HTTP and database implementations.

mod problem;
mod validation;

pub use problem::problem_json;
pub use validation::{is_email, is_http_url, parse_bool, parse_i64, parse_uuid, text_code};

#[derive(Debug, Clone, Copy)]
pub struct ModelDescriptor {
    pub name: &'static str,
    pub table: &'static str,
    pub api_path: &'static str,
    /// Explicit opt-in for unauthenticated access (demo only).
    pub allow_anonymous: bool,
    /// `None` verweigert den Zugriff, sofern `allow_anonymous` nicht gesetzt ist.
    /// `Some("authenticated")` verlangt eine aktive Sitzung.
    /// Jeder andere Wert ist der Ressourcenname für `{name}.read|create|update|delete`.
    pub permission: Option<&'static str>,
    /// Objektregeln liefern einen `QueryScope`. Ohne gebundene Policy bleiben die Routen gesperrt.
    pub requires_object_policy: bool,
}

/// Vergleichswert eines Query-Scopes. Er stammt aus der Policy, nicht aus der HTTP-Anfrage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeValue {
    Text(String),
    Uuid(uuid::Uuid),
    BigInt(i64),
    Bool(bool),
}

/// Datenbankfilter einer Objektregel. `None` liefert keine Zeile. `All` nur, wenn die Policy das ausdrücklich sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryScope {
    All,
    None,
    Equals {
        column: &'static str,
        value: ScopeValue,
    },
}

/// Admin-Feld. Die Werte stammen aus demselben Modell wie das Schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminFieldMeta {
    pub name: &'static str,
    pub label: &'static str,
    pub list_display: bool,
    pub searchable: bool,
    pub sortable: bool,
    pub filterable: bool,
    pub readonly: bool,
    pub hidden: bool,
}

/// Admin-Freigabe eines Modells. `None` bei `admin_meta` bedeutet: nicht im Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminModelMeta {
    pub key: &'static str,
    pub label: &'static str,
    /// `None`: nur ein aktiver Superuser. Sonst `{permission}.read|create|update|delete`.
    pub permission: Option<&'static str>,
    pub fields: &'static [AdminFieldMeta],
}

/// Backend, für den Ferrum DDL und Abfragen erzeugt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
}

/// Spaltentyp, unabhängig vom SQL-Dialekt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    Uuid,
    Text,
    BigInt,
    Boolean,
}

/// Aktion für `ON DELETE`. `ON UPDATE` bleibt `NO ACTION`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferentialAction {
    Restrict,
    Cascade,
    SetNull,
}

/// Metadaten einer Modellspalte. Das Makro erzeugt sie, die Datenbankschicht rendert sie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnSpec {
    pub name: &'static str,
    pub kind: ColumnKind,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_bool: Option<bool>,
    pub unique: bool,
    pub indexed: bool,
    pub foreign_table: Option<&'static str>,
    pub foreign_column: Option<&'static str>,
    pub on_delete: Option<ReferentialAction>,
}

/// Zusammengesetzte Unique-Constraints und Indizes. Leere Listen sind der Normalfall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableConstraints {
    pub unique_groups: &'static [&'static [&'static str]],
    pub index_groups: &'static [&'static [&'static str]],
}

impl TableConstraints {
    pub const EMPTY: Self = Self {
        unique_groups: &[],
        index_groups: &[],
    };
}

impl Default for TableConstraints {
    fn default() -> Self {
        Self::EMPTY
    }
}

pub trait FerrumModel: Send + Sync + 'static {
    fn descriptor() -> &'static ModelDescriptor;
    fn columns() -> &'static [ColumnSpec];
    fn constraints() -> &'static TableConstraints {
        &TableConstraints::EMPTY
    }
    fn schema_sql(kind: DatabaseKind) -> String;
    fn admin_meta() -> Option<&'static AdminModelMeta> {
        None
    }
}

pub fn unique_index_name(table: &str, columns: &[&str]) -> String {
    object_name("uq", table, columns)
}

pub fn secondary_index_name(table: &str, columns: &[&str]) -> String {
    object_name("ix", table, columns)
}

pub fn foreign_key_name(
    table: &str,
    column: &str,
    referenced_table: &str,
    referenced_column: &str,
) -> String {
    format!("fk_{table}_{column}_{referenced_table}_{referenced_column}")
}

fn object_name(prefix: &str, table: &str, columns: &[&str]) -> String {
    let mut name = format!("{prefix}_{table}");
    for column in columns {
        name.push('_');
        name.push_str(column);
    }
    name
}

#[derive(Debug, thiserror::Error)]
pub enum FerrumError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("permission denied")]
    Forbidden,
    #[error("{0}")]
    Denied(String),
    #[error("authentication required")]
    Unauthorized(String),
    #[error("resource not found")]
    NotFound,
    #[error("database operation failed")]
    Database(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl FerrumError {
    pub fn database<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Database(Box::new(error))
    }
}
