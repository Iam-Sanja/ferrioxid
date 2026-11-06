//! Übernimmt eine vorhandene SQLite-Datenbank in die Migrationshistorie, ohne Tabellen oder Zeilen zu ändern.

use crate::diff::MigrationOperation;
use crate::error::MigrationError;
use crate::history::{self, insert_history};
use crate::introspection::{compare_snapshot, introspect_sqlite};
use crate::store::{inspect_directory, MigrationFile};
use ferrum_core::DatabaseKind;
use ferrum_db::kind_of;
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
            "Baseline ist für PostgreSQL nicht implementiert. Es wurde keine Historie geschrieben. Die Übernahme muss manuell erfolgen.".to_owned(),
        ));
    }
    let state = inspect_directory(directory);
    if !state.conflicts.is_empty() {
        return Err(MigrationError::new(state.conflicts.join("\n")));
    }
    if state.files.is_empty() {
        return Err(MigrationError::new(
            "Es gibt keine Migrationsdateien für eine Baseline.".to_owned(),
        ));
    }
    refuse_unverifiable(&state.files)?;
    let history = history::read_history(db).await?;
    if !history.is_empty() {
        return Err(MigrationError::new(
            "Die Migrationshistorie enthält bereits Einträge. Baseline überschreibt sie nicht und verändert keine Daten.".to_owned(),
        ));
    }
    let expected = &state
        .files
        .last()
        .ok_or_else(|| {
            MigrationError::new("Es gibt keine Migrationsdateien für eine Baseline.".to_owned())
        })?
        .to_snapshot;
    let live = introspect_sqlite(db).await?;
    let differences = compare_snapshot(expected, &live);
    if !differences.is_empty() {
        return Err(MigrationError::new(format!(
            "Baseline abgebrochen. Schema und Migrationsziel stimmen nicht überein. Es wurde keine Historie geschrieben.\n{}",
            differences.join("\n")
        )));
    }
    if !confirm {
        return Err(MigrationError::new(
            "Prüfung erfolgreich: das vorhandene Schema entspricht dem Migrationsziel. Es wurden keine Tabellen und keine Daten verändert. Wiederhole den Befehl mit --confirm, um ausschließlich die Historie zu schreiben.".to_owned(),
        ));
    }
    let transaction = db.begin().await.map_err(|error| {
        MigrationError::new(format!(
            "Transaktion für die Baseline konnte nicht gestartet werden: {error}"
        ))
    })?;
    history::ensure_history(&transaction).await?;
    let applied_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| MigrationError::new(format!("Systemzeit ist ungültig: {error}")))?
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
    transaction.commit().await.map_err(|error| {
        MigrationError::new(format!(
            "Baseline konnte nicht festgeschrieben werden: {error}"
        ))
    })?;
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
                    "Migration {} enthält `{}`. Das lässt sich am Endschema nicht belegen. Baseline markiert diese Kette nicht als ausgeführt. Die Übernahme muss manuell erfolgen.",
                    file.id,
                    operation.summary()
                )));
            }
        }
    }
    Ok(())
}
