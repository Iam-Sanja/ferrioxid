//! Versionierte Migrationen aus Ferrioxid-Modellmetadaten.
//!
//! Die Engine liest keine Rust-Quellen. Anwendungen registrieren ihre Modelle
//! explizit und stellen das Schema der CLI über `ferrioxid.toml` oder eine Schema-Datei bereit.

mod baseline;
mod diff;
mod error;
mod history;
mod inspect;
mod introspection;
mod manifest;
mod packages;
mod project;
mod rebuild;
mod runner;
mod simulate;
mod snapshot;
mod store;
mod upgrade;

pub use baseline::baseline_database;
pub use diff::{
    plan_change, plan_change_with_backfills, ApplyClass, BackfillLiteral, ColumnBackfill,
    MigrationOperation, MigrationPlan, MigrationRisk,
};
pub use error::MigrationError;
pub use history::{history_exists, table_exists, AUTH_HISTORY_TABLE};
pub use inspect::{
    dev_notice, inspect_migrations, inspect_migrations_with_packages, readonly_database_url,
    render_diagnosis, Diagnostic, DualReport, Level, MigrationScope, PackageStatus,
};
pub use manifest::{
    load_manifest, manifest_json, ProjectManifest, TrackedFile, MANIFEST_RELATIVE, TEMPLATE_VERSION,
};
pub use packages::{
    apply_packages, package_order, package_reports, resolve_installed_schemas,
    runtime_contract_note, same_migration_contract, validate_package_graph, InstalledApp,
    PackageMigration,
};
pub use project::{
    discover_project, load_schema_from_command, redact_database_url, DeclaredApp, ProjectConfig,
    Settings,
};
pub use runner::{
    apply_directory, apply_directory_with_history, apply_namespaced_directory, migration_status,
    migration_status_in, namespaced_status, reseal_unapplied, ApplyReport, StatusReport,
};
pub use snapshot::{
    ColumnSnapshot, ExternalTableSnapshot, ForeignKeySnapshot, IndexSnapshot, LogicalType,
    OnDelete, RegisteredModel, SchemaSnapshot, TableSnapshot, UniqueConstraintSnapshot,
    FORMAT_VERSION,
};
pub use store::sha256_hex;
pub use store::{
    inspect_directory, load_migration, make_migration, make_migration_with_backfills,
    reseal_migration, save_migration, seal, write_explicit_migration, MakeOutcome, MigrationFile,
};
pub use upgrade::{
    apply_upgrade, contained_target, ferrioxid_dependency, plan_upgrade, render_plan,
    DependencyKind, PlanAction, UpgradePlan,
};
