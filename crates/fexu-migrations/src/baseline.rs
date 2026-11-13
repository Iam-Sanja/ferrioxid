//! Baselines an existing SQLite database into migration history without changing tables or rows.

use crate::diff::MigrationOperation;
use crate::error::MigrationError;
use crate::history::{self, insert_history};
use crate::introspection::{compare_snapshot, introspect_sqlite};
use crate::store::{inspect_directory, MigrationFile};
use fexu_core::DatabaseKind;
use fexu_db::kind_of;
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn baseline_database(
    db: &DatabaseConnection,
    directory: &Path,
    confirm: bool,
) -> Result<Vec<String>, MigrationError> {
    let kind = kind_of(db.get_database_backend())
        .map_err(|error| MigrationError::new(error.to_string()))?;
    if kind != DatabaseKind::Sqlite {
        return Err(MigrationError::new(
            "Baseline is not implemented for PostgreSQL. No history was written; migrate manually."
                .to_owned(),
        ));
    }
    let state = inspect_directory(directory);
    if !state.conflicts.is_empty() {
        return Err(MigrationError::new(state.conflicts.join("\n")));
    }
    if state.files.is_empty() {
        return Err(MigrationError::new(
            "There are no migration files to baseline.".to_owned(),
        ));
    }
    refuse_unverifiable(&state.files)?;
    let history = history::read_history(db).await?;
    if !history.is_empty() {
        return Err(MigrationError::new(
            "Migration history already contains entries. Baseline does not overwrite history or change data.".to_owned(),
        ));
    }
    let expected = &state
        .files
        .last()
        .ok_or_else(|| MigrationError::new("There are no migration files to baseline.".to_owned()))?
        .to_snapshot;
    let live = introspect_sqlite(db).await?;
    let differences = compare_snapshot(expected, &live);
    if !differences.is_empty() {
        return Err(MigrationError::new(format!(
            "Baseline aborted. Database schema and migration target do not match. No history was written.\n{}",
            differences.join("\n")
        )));
    }
    if !confirm {
        return Err(MigrationError::new(
            "Validation successful: the database schema matches the migration target. No tables or data were changed. Re-run with --confirm to write history only.".to_owned(),
        ));
    }
    let transaction = db.begin().await.map_err(|error| {
        MigrationError::new(format!("Could not begin baseline transaction: {error}"))
    })?;
    history::ensure_history(&transaction).await?;
    let applied_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| MigrationError::new(format!("Invalid system time: {error}")))?
        .as_secs() as i64;
    let mut ids = Vec::new();
    for file in &state.files {
        insert_history(
            &transaction,
            &file.id,
            &file.name,
            &file.checksum,
            applied_at_unix,
        )
        .await?;
        ids.push(file.id.clone());
    }
    transaction
        .commit()
        .await
        .map_err(|error| MigrationError::new(format!("Could not commit baseline: {error}")))?;
    Ok(ids)
}

fn refuse_unverifiable(files: &[MigrationFile]) -> Result<(), MigrationError> {
    for file in files {
        for operation in &file.operations {
            let blocked = match operation {
                MigrationOperation::DropTable { .. }
                | MigrationOperation::DropColumn { .. }
                | MigrationOperation::DropUniqueConstraint { .. }
                | MigrationOperation::DropIndex { .. }
                | MigrationOperation::DropForeignKey { .. }
                | MigrationOperation::AddForeignKey { .. } => true,
                MigrationOperation::AlterColumn { from, to, .. }
                    if from.logical_type != to.logical_type =>
                {
                    true
                }
                MigrationOperation::CreateTable { .. }
                | MigrationOperation::AddColumn { .. }
                | MigrationOperation::AlterColumn { .. }
                | MigrationOperation::RenameColumn { .. }
                | MigrationOperation::RenameTable { .. }
                | MigrationOperation::AddUniqueConstraint { .. }
                | MigrationOperation::AddIndex { .. } => false,
            };
            if blocked {
                return Err(MigrationError::new(format!(
                    "Migration {} contains `{}`. This cannot be verified from the final schema. Baseline will not mark this chain as executed; migrate manually.",
                    file.id,
                    operation.summary()
                )));
            }
        }
    }
    Ok(())
}
