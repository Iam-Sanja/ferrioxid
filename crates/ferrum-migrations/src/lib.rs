//! Versionierte Migrationen aus Ferrum-Modellmetadaten.
//!
//! Die Engine liest keine Rust-Quellen. Anwendungen registrieren ihre Modelle
//! explizit und stellen das Schema der CLI über `ferrum.toml` oder eine Schema-Datei bereit.

mod baseline;
mod diff;
mod error;
mod history;
mod inspect;
mod introspection;
mod manifest;
mod project;
mod rebuild;
mod runner;
mod simulate;
mod snapshot;
mod store;
mod upgrade;

pub use baseline::baseline_database;
pub use diff::{plan_change, ApplyClass, MigrationOperation, MigrationPlan, MigrationRisk};
pub use error::MigrationError;
pub use history::{history_exists, table_exists, AUTH_HISTORY_TABLE};
pub use inspect::{
    dev_notice, inspect_migrations, readonly_database_url, render_diagnosis, Diagnostic,
    DualReport, Level, MigrationScope,
};
pub use manifest::{
    load_manifest, manifest_json, ProjectManifest, TrackedFile, MANIFEST_RELATIVE, TEMPLATE_VERSION,
};
pub use project::{
    discover_project, load_schema_from_command, redact_database_url, ProjectConfig, Settings,
};
pub use runner::{
    apply_directory, apply_directory_with_history, migration_status, reseal_unapplied, ApplyReport,
    StatusReport,
};
pub use snapshot::{
    ColumnSnapshot, ForeignKeySnapshot, IndexSnapshot, LogicalType, OnDelete, RegisteredModel,
    SchemaSnapshot, TableSnapshot, UniqueConstraintSnapshot, FORMAT_VERSION,
};
pub use store::sha256_hex;
pub use store::{
    inspect_directory, load_migration, make_migration, reseal_migration, save_migration, seal,
    write_explicit_migration, MakeOutcome, MigrationFile,
};
pub use upgrade::{
    apply_upgrade, contained_target, ferrum_dependency, plan_upgrade, render_plan, DependencyKind,
    PlanAction, UpgradePlan,
};
