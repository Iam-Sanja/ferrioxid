//! Versionierte Migrationen aus Ferrioxid-Modellmetadaten.
//!
//! Die Engine liest keine Rust-Quellen. Anwendungen registrieren ihre Modelle
//! explizit und stellen das Schema der CLI über `ferrioxid.toml` oder eine Schema-Datei bereit.

mod baseline;
mod diff;
mod down;
mod error;
mod fixtures;
mod history;
mod inspect;
mod introspection;
mod management;
mod manifest;
mod packages;
mod plan;
mod project;
mod rebuild;
mod runner;
mod simulate;
mod snapshot;
mod store;
mod upgrade;

pub use baseline::baseline_database;
pub use diff::{
    classify, plan_change, plan_change_with_backfills, ApplyClass, BackfillLiteral, ColumnBackfill,
    MigrationOperation, MigrationPlan, MigrationRisk,
};
pub use error::MigrationError;
pub use fixtures::{
    dump_fixture, fixture_to_json, load_fixture, parse_fixture, prepare_records, FixtureFile,
    FixtureRecord, LoadReport, LoadRequest, FIXTURE_VERSION,
};
pub use history::{column_exists, history_exists, table_exists, AUTH_HISTORY_TABLE};
pub use inspect::{
    dev_notice, inspect_framework, inspect_migrations, inspect_migrations_with_packages,
    readonly_database_url, render_diagnosis, Diagnostic, DualReport, Level, MigrationScope,
    PackageStatus,
};
pub use management::{
    is_command_name, ArgSpec, CommandError, CommandSpec, Invocation, ManagementCommand, Registry,
};
pub use manifest::{
    load_manifest, manifest_json, ProjectManifest, TrackedFile, MANIFEST_RELATIVE, TEMPLATE_VERSION,
};
pub use packages::{
    apply_packages, package_order, package_reports, resolve_installed_schemas,
    runtime_contract_note, same_migration_contract, validate_package_graph, InstalledApp,
    PackageMigration,
};
pub use plan::{plan_database, plan_database_in, PlanFinding, PlanLevel, PlanReport};
pub use project::{
    discover_project, load_schema_from_command, redact_database_url, DeclaredApp, ProjectConfig,
    Settings,
};
pub use runner::{
    apply_directory, apply_directory_with_history, apply_files_with_history,
    apply_namespaced_directory, migration_status, migration_status_in, namespaced_status,
    reseal_unapplied, revert_latest, revert_namespaced, status_of_files, ApplyReport, RevertPlan,
    StatusReport,
};
pub use snapshot::{
    is_identifier, ColumnSnapshot, ExternalTableSnapshot, ForeignKeySnapshot, IndexSnapshot,
    LogicalType, OnDelete, RegisteredModel, SchemaSnapshot, TableSnapshot,
    UniqueConstraintSnapshot, FORMAT_VERSION,
};
pub use store::sha256_hex;
pub use store::{
    inspect_directory, load_migration, load_migration_text, make_migration,
    make_migration_with_backfills, require_embedded_migrations, reseal_migration, save_migration,
    seal, write_explicit_migration, MakeOutcome, MigrationFile,
};
pub use upgrade::{
    apply_upgrade, contained_target, ferrioxid_dependency, plan_upgrade, render_plan,
    DependencyKind, PlanAction, UpgradePlan,
};
