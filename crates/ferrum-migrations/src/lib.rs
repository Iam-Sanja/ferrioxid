//! Versionierte Migrationen aus Ferrum-Modellmetadaten.
//!
//! Die Engine liest keine Rust-Quellen. Anwendungen registrieren ihre Modelle
//! explizit und stellen das Schema der CLI über `ferrum.toml` oder eine Schema-Datei bereit.

mod diff;
mod error;
mod history;
mod project;
mod runner;
mod snapshot;
mod store;

pub use diff::{plan_change, ApplyClass, MigrationOperation, MigrationPlan, MigrationRisk};
pub use error::MigrationError;
pub use history::{history_exists, table_exists};
pub use project::{discover_project, load_schema_from_command, ProjectConfig};
pub use runner::{apply_directory, migration_status, ApplyReport, StatusReport};
pub use snapshot::{
    ColumnSnapshot, LogicalType, RegisteredModel, SchemaSnapshot, TableSnapshot, FORMAT_VERSION,
};
pub use store::{
    inspect_directory, load_migration, make_migration, save_migration, seal, MakeOutcome,
    MigrationFile,
};
