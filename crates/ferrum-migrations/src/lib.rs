//! Versionierte Migrationen aus Ferrum-Modellmetadaten.
//!
//! Die Engine liest keine Rust-Quellen. Anwendungen registrieren ihre Modelle
//! explizit und stellen das Schema der CLI über `ferrum.toml` oder eine Schema-Datei bereit.

mod baseline;
mod diff;
mod error;
mod history;
mod introspection;
mod project;
mod rebuild;
mod runner;
mod simulate;
mod snapshot;
mod store;

pub use baseline::baseline_database;
pub use diff::{plan_change, ApplyClass, MigrationOperation, MigrationPlan, MigrationRisk};
pub use error::MigrationError;
pub use history::{history_exists, table_exists, AUTH_HISTORY_TABLE};
pub use project::{discover_project, load_schema_from_command, ProjectConfig};
pub use runner::{
    apply_directory, apply_directory_with_history, migration_status, reseal_unapplied, ApplyReport,
    StatusReport,
};
pub use snapshot::{
    ColumnSnapshot, ForeignKeySnapshot, IndexSnapshot, LogicalType, OnDelete, RegisteredModel,
    SchemaSnapshot, TableSnapshot, UniqueConstraintSnapshot, FORMAT_VERSION,
};
pub use store::{
    inspect_directory, load_migration, make_migration, reseal_migration, save_migration, seal,
    write_explicit_migration, MakeOutcome, MigrationFile,
};
