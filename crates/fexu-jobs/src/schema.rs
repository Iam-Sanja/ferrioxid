//! Job-Tabelle. Die Migration hängt an der eigenen Historie `_fexu_job_migrations`.

use crate::error::JobError;
#[cfg(test)]
use fexu_migrations::{
    plan_change, seal, ColumnSnapshot, IndexSnapshot, LogicalType, MigrationFile, SchemaSnapshot,
    TableSnapshot, UniqueConstraintSnapshot,
};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use std::path::PathBuf;

pub const JOBS: &str = "_fexu_jobs";
pub const JOB_HISTORY: &str = "_fexu_job_migrations";

pub fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

const EMBEDDED_MIGRATIONS: &[(&str, &str)] = &[
    ("0001_jobs", include_str!("../migrations/0001_jobs.json")),
    (
        "0002_job_schedule",
        include_str!("../migrations/0002_job_schedule.json"),
    ),
];

/// Job migrations compiled into the binary. Runtime does not open `CARGO_MANIFEST_DIR`.
pub fn embedded_migrations() -> Result<Vec<fexu_migrations::MigrationFile>, JobError> {
    fexu_migrations::require_embedded_migrations(
        "fexu-jobs",
        &["0001_jobs", "0002_job_schedule"],
        EMBEDDED_MIGRATIONS,
    )
    .map_err(|error| JobError::message("job.migration", error.to_string()))
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
    let files = embedded_migrations()?;
    let report = fexu_migrations::apply_files_with_history(db, &files, false, JOB_HISTORY)
        .await
        .map_err(|error| JobError::message("job.migration", error.to_string()))?;
    Ok(report.applied)
}

pub async fn migration_status(
    db: &DatabaseConnection,
) -> Result<fexu_migrations::StatusReport, JobError> {
    let files = embedded_migrations()?;
    fexu_migrations::status_of_files(db, &files, JOB_HISTORY, None)
        .await
        .map_err(|error| JobError::message("job.migration", error.to_string()))
}

pub async fn render_migration_status(db: &DatabaseConnection) -> Result<(String, bool), JobError> {
    let report = migration_status(db).await?;
    let current =
        report.conflicts.is_empty() && report.lines.iter().all(|line| line.state == "angewendet");
    Ok((format!("Job-Migrationen:\n{}", report.render()), current))
}

pub async fn table_ready(db: &impl ConnectionTrait) -> Result<bool, JobError> {
    let ready = fexu_migrations::table_exists(db, JOBS)
        .await
        .map_err(|error| JobError::message("job.database", error.to_string()))?;
    if !ready {
        return Ok(false);
    }
    fexu_migrations::column_exists(db, JOBS, "cron_expression")
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
        fexu_migrations::save_migration(&migrations_dir(), &file).expect("write migration");
    });
}

#[cfg(test)]
fn ensure_schedule_migration() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let path = migrations_dir().join("0002_job_schedule.json");
        if path.exists() {
            return;
        }
        let file = schedule_migration_file().expect("schedule schema can be sealed");
        fexu_migrations::save_migration(&migrations_dir(), &file).expect("write migration");
    });
}

#[cfg(test)]
pub fn schedule_migration_file() -> Result<MigrationFile, JobError> {
    let previous = fexu_migrations::load_migration(&migrations_dir().join("0001_jobs.json"))
        .map_err(|error| JobError::message("job.migration", error.to_string()))?;
    let mut to = previous.to_snapshot.clone();
    let table = to
        .tables
        .iter_mut()
        .find(|table| table.name == JOBS)
        .ok_or_else(|| JobError::message("job.migration", "Die Job-Tabelle fehlt."))?;
    table
        .columns
        .push(column("cron_expression", LogicalType::String, false, true));
    table
        .columns
        .push(column("cron_zone", LogicalType::String, false, true));
    to.validate()
        .map_err(|error| JobError::message("job.migration", error.to_string()))?;
    let plan = plan_change(&previous.to_snapshot, &to)
        .map_err(|error| JobError::message("job.migration", error.to_string()))?;
    if let Some(reason) = &plan.refusal {
        return Err(JobError::message("job.migration", reason.clone()));
    }
    seal(
        "0002_job_schedule".to_owned(),
        "job_schedule".to_owned(),
        previous.to_snapshot,
        to,
        &plan,
    )
    .map_err(|error| JobError::message("job.migration", error.to_string()))
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
            fexu_migrations::save_migration(&migrations_dir(), &file).expect("write migration");
        }
        let loaded = fexu_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0001_jobs");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(format!("{loaded:?}").contains("_fexu_jobs"));
    }

    #[test]
    fn schedule_migration_is_sealed() {
        super::ensure_schedule_migration();
        let file = super::schedule_migration_file().expect("schedule schema can be sealed");
        let path = migrations_dir().join("0002_job_schedule.json");
        let loaded = fexu_migrations::load_migration(&path).expect("load migration");
        assert_eq!(loaded.id, "0002_job_schedule");
        assert_eq!(loaded.checksum, file.checksum);
        assert!(format!("{loaded:?}").contains("cron_expression"));
    }
}
