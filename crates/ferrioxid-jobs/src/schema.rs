//! Job-Tabelle. Die Migration hängt an der eigenen Historie `_ferrioxid_job_migrations`.

use crate::error::JobError;
#[cfg(test)]
use ferrioxid_migrations::{
    plan_change, seal, ColumnSnapshot, IndexSnapshot, LogicalType, MigrationFile, SchemaSnapshot,
    TableSnapshot, UniqueConstraintSnapshot,
};
use sea_orm::DatabaseConnection;
use std::path::PathBuf;

pub const JOBS: &str = "_ferrioxid_jobs";
pub const JOB_HISTORY: &str = "_ferrioxid_job_migrations";

pub fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

#[cfg(test)]
pub fn migration_file() -> Result<MigrationFile, JobError> {
    let snapshot = schema()?;
    let plan = plan_change(&SchemaSnapshot::empty(), &snapshot)
        .map_err(|error| JobError::message("job.migration", error.to_string()))?;
    if let Some(reason) = &plan.refusal {
        return Err(JobError::message("job.migration", reason.clone()));
    }
    seal(
        "0001_jobs".to_owned(),
        "jobs".to_owned(),
        SchemaSnapshot::empty(),
        snapshot,
        &plan,
    )
    .map_err(|error| JobError::message("job.migration", error.to_string()))
}

pub async fn apply_migrations(db: &DatabaseConnection) -> Result<Vec<String>, JobError> {
    #[cfg(test)]
    ensure_test_migration();
    let report = ferrioxid_migrations::apply_directory_with_history(
        db,
        &migrations_dir(),
        false,
        JOB_HISTORY,
    )
    .await
    .map_err(|error| JobError::message("job.migration", error.to_string()))?;
    Ok(report.applied)
}

pub async fn render_migration_status(db: &DatabaseConnection) -> Result<(String, bool), JobError> {
    let report =
        ferrioxid_migrations::migration_status_in(db, &migrations_dir(), JOB_HISTORY, None)
            .await
            .map_err(|error| JobError::message("job.migration", error.to_string()))?;
    let current =
        report.conflicts.is_empty() && report.lines.iter().all(|line| line.state == "angewendet");
    Ok((format!("Job-Migrationen:\n{}", report.render()), current))
}

pub async fn table_ready(db: &DatabaseConnection) -> Result<bool, JobError> {
    ferrioxid_migrations::table_exists(db, JOBS)
        .await
        .map_err(|error| JobError::message("job.database", error.to_string()))
}

#[cfg(test)]
fn schema() -> Result<SchemaSnapshot, JobError> {
    let snapshot = SchemaSnapshot::from_tables(vec![jobs_table()]);
    snapshot
        .validate()
        .map_err(|error| JobError::message("job.migration", error.to_string()))?;
    Ok(snapshot)
}

#[cfg(test)]
fn jobs_table() -> TableSnapshot {
    TableSnapshot {
        name: JOBS.to_owned(),
        columns: vec![
            column("id", LogicalType::Uuid, true, false),
            column("handler", LogicalType::String, false, false),
            column("payload_version", LogicalType::I64, false, false),
            column("payload", LogicalType::String, false, false),
            column("status", LogicalType::String, false, false),
            column("created_at", LogicalType::I64, false, false),
            column("available_at", LogicalType::I64, false, false),
            column("attempts", LogicalType::I64, false, false),
            column("max_attempts", LogicalType::I64, false, false),
            column("next_attempt_at", LogicalType::I64, false, true),
            column("lease_owner", LogicalType::String, false, true),
            column("lease_token", LogicalType::String, false, true),
            column("lease_expires_at", LogicalType::I64, false, true),
            column("error_code", LogicalType::String, false, true),
            column("finished_at", LogicalType::I64, false, true),
            column("idempotency_key", LogicalType::String, false, true),
            column("interval_secs", LogicalType::I64, false, true),
            column("schedule_key", LogicalType::String, false, true),
        ],
        unique_constraints: vec![unique(&["idempotency_key"]), unique(&["schedule_key"])],
        indexes: vec![IndexSnapshot {
            name: format!("ix_{JOBS}_status_available_at"),
            columns: vec!["status".to_owned(), "available_at".to_owned()],
        }],
        foreign_keys: Vec::new(),
    }
}

#[cfg(test)]
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

#[cfg(test)]
fn unique(columns: &[&str]) -> UniqueConstraintSnapshot {
    let mut name = format!("uq_{JOBS}");
    let mut owned = Vec::new();
    for column in columns {
        name.push('_');
        name.push_str(column);
        owned.push((*column).to_owned());
    }
    UniqueConstraintSnapshot {
        name,
        columns: owned,
    }
}

#[cfg(test)]
fn ensure_test_migration() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let path = migrations_dir().join("0001_jobs.json");
        if path.exists() {
            return;
        }
        let file = migration_file().expect("jobs schema can be sealed");
        ferrioxid_migrations::save_migration(&migrations_dir(), &file).expect("write migration");
    });
}

#[cfg(test)]
mod tests {
    use super::{ensure_test_migration, migration_file, migrations_dir};

    #[test]
    fn jobs_migration_is_sealed() {
        ensure_test_migration();
        let file = migration_file().expect("jobs schema can be sealed");
        let path = migrations_dir().join("0001_jobs.json");
        if !path.exists() {
            ferrioxid_migrations::save_migration(&migrations_dir(), &file)
                .expect("write migration");
        }
        let loaded = ferrioxid_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0001_jobs");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(format!("{loaded:?}").contains("_ferrioxid_jobs"));
    }
}
