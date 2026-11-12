//! Executes a verified migration plan. Unsafe operations are rejected.

use crate::diff::{classify, ApplyClass, MigrationOperation};
use crate::error::MigrationError;
use crate::history::{self, HistoryEntry};
use crate::snapshot::{ColumnSnapshot, SchemaSnapshot};
use crate::store::{inspect_directory, MigrationFile};
use ferrioxid_core::{secondary_index_name, unique_index_name, DatabaseKind, ReferentialAction};
use ferrioxid_db::{
    kind_of, render_add_column_referencing, render_create_index, render_drop_column,
    render_drop_index, render_drop_table, render_rename_column, render_rename_table,
    render_table_bundle, with_checks, ForeignKeyClause, PhysicalColumn,
};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, ExecResult, QueryResult,
    RuntimeErr, Statement, TransactionTrait,
};
use sea_query_binder::SqlxValues;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct ApplyReport {
    pub applied: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevertPlan {
    pub id: String,
    pub steps: Vec<String>,
    pub destructive: bool,
    pub executed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationLine {
    pub id: String,
    pub state: String,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    pub backend: String,
    pub lines: Vec<MigrationLine>,
    pub conflicts: Vec<String>,
    pub schema_note: String,
}

impl StatusReport {
    pub fn render(&self) -> String {
        let mut text = format!("Backend: {}\n", self.backend);
        if self.lines.is_empty() {
            text.push_str("Migrations: none\n");
        } else {
            text.push_str("Migrations:\n");
            for line in &self.lines {
                text.push_str(&format!(
                    "  {}  {}  checksum {}\n",
                    line.id,
                    match line.state.as_str() {
                        "angewendet" => "applied",
                        "ausstehend" => "pending",
                        other => other,
                    },
                    line.checksum
                ));
            }
        }
        if self.conflicts.is_empty() {
            text.push_str("Conflicts: none\n");
        } else {
            text.push_str("Conflicts:\n");
            for conflict in &self.conflicts {
                text.push_str(&format!("  {conflict}\n"));
            }
        }
        text.push_str(&format!("Schema: {}\n", self.schema_note));
        text
    }
}

/// Reverts the latest applied migration in `directory`.
///
/// A later applied migration id, including one from another app, blocks the
/// revert. Irreversible operations are refused. Dropping a table or column
/// requires `allow_destructive`. `dry_run` returns the inverse plan and writes
/// nothing.
pub async fn revert_latest(
    db: &DatabaseConnection,
    directory: &Path,
    allow_destructive: bool,
    dry_run: bool,
) -> Result<RevertPlan, MigrationError> {
    revert_scoped(db, directory, None, allow_destructive, dry_run).await
}

/// Reverts the latest applied migration of one app.
///
/// History ids are `app:file`. A later applied file id from any app still blocks
/// the revert. Dropping a table or column requires `allow_destructive`.
pub async fn revert_namespaced(
    db: &DatabaseConnection,
    directory: &Path,
    namespace: &str,
    allow_destructive: bool,
    dry_run: bool,
) -> Result<RevertPlan, MigrationError> {
    validate_namespace(namespace)?;
    revert_scoped(db, directory, Some(namespace), allow_destructive, dry_run).await
}

async fn revert_scoped(
    db: &DatabaseConnection,
    directory: &Path,
    namespace: Option<&str>,
    allow_destructive: bool,
    dry_run: bool,
) -> Result<RevertPlan, MigrationError> {
    let state = inspect_directory(directory);
    if !state.conflicts.is_empty() {
        return Err(MigrationError::new(state.conflicts.join("\n")));
    }
    history::ensure_history_in(db, history::HISTORY_TABLE).await?;
    let history = history::read_history_in(db, history::HISTORY_TABLE).await?;
    let applied = scoped_history(&history, namespace)?;
    let pending = pending_files(&state.files, &applied)?;
    let applied_count = state.files.len().saturating_sub(pending.len());
    if applied_count == 0 {
        return Err(MigrationError::new(
            "No applied migration can be reverted.".to_owned(),
        ));
    }
    let target = &state.files[applied_count - 1];
    for entry in &history {
        let other = crate::down::file_id(&entry.id);
        if other > target.id.as_str() {
            return Err(MigrationError::new(format!(
                "Cannot revert {} while {} is still applied.",
                target.id, entry.id
            )));
        }
    }
    let simulated = crate::down::simulated_target(&target.from_snapshot, &target.operations)?;
    if !simulated.semantically_eq(&target.to_snapshot) {
        return Err(MigrationError::new(format!(
            "Migration {} does not match its stored snapshot. Nothing was changed.",
            target.id
        )));
    }
    let inverse = crate::down::invert_operations(&target.operations)?;
    let mut schema = target.to_snapshot.clone();
    let mut destructive = false;
    let mut steps = Vec::new();
    for operation in &inverse {
        let (class, reason) = classify(operation, &schema);
        steps.push(operation.summary());
        schema = crate::simulate::apply_operations(&schema, std::slice::from_ref(operation))?;
        match class {
            ApplyClass::Safe => {}
            ApplyClass::Destructive => destructive = true,
            ApplyClass::Unsupported => {
                return Err(MigrationError::new(format!(
                    "Migration {} cannot be reverted: {}",
                    target.id,
                    reason.unwrap_or_else(|| "This operation has no safe inverse.".to_owned())
                )));
            }
        }
    }
    let plan = RevertPlan {
        id: target.id.clone(),
        steps,
        destructive,
        executed: false,
    };
    if dry_run {
        return Ok(plan);
    }
    if destructive && !allow_destructive {
        return Err(MigrationError::new(format!(
            "Reverting {} drops tables or columns and was not executed. Re-run with --allow-destructive only if that data loss is intentional.",
            target.id
        )));
    }
    let mut reversed = target.clone();
    reversed.from_snapshot = target.to_snapshot.clone();
    reversed.operations = inverse;
    reversed.to_snapshot = schema;
    let history_id = history_key(namespace, &target.id);
    apply_one(db, &reversed, history::HISTORY_TABLE, &history_id, false).await?;
    Ok(RevertPlan {
        executed: true,
        ..plan
    })
}

pub async fn apply_directory(
    db: &DatabaseConnection,
    directory: &Path,
    allow_destructive: bool,
) -> Result<ApplyReport, MigrationError> {
    apply_directory_with_history(db, directory, allow_destructive, history::HISTORY_TABLE).await
}

pub async fn apply_directory_with_history(
    db: &DatabaseConnection,
    directory: &Path,
    allow_destructive: bool,
    history_table: &str,
) -> Result<ApplyReport, MigrationError> {
    apply_scoped(db, directory, allow_destructive, history_table, None).await
}

/// Applies migrations in an app directory. History IDs use `app:file`.
///
/// The file checksum remains unchanged. Existing legacy `0001_initial` history is not remapped.
pub async fn apply_namespaced_directory(
    db: &DatabaseConnection,
    directory: &Path,
    namespace: &str,
    allow_destructive: bool,
) -> Result<ApplyReport, MigrationError> {
    validate_namespace(namespace)?;
    if !directory.is_dir() {
        return Err(MigrationError::new(format!(
            "The migrations directory for app `{namespace}` is missing."
        )));
    }
    apply_scoped(
        db,
        directory,
        allow_destructive,
        history::HISTORY_TABLE,
        Some(namespace),
    )
    .await
}

pub async fn namespaced_status(
    db: &DatabaseConnection,
    directory: &Path,
    namespace: &str,
) -> Result<StatusReport, MigrationError> {
    validate_namespace(namespace)?;
    status_scoped(db, directory, history::HISTORY_TABLE, None, Some(namespace)).await
}

async fn apply_scoped(
    db: &DatabaseConnection,
    directory: &Path,
    allow_destructive: bool,
    history_table: &str,
    namespace: Option<&str>,
) -> Result<ApplyReport, MigrationError> {
    let state = inspect_directory(directory);
    if !state.conflicts.is_empty() {
        return Err(MigrationError::new(state.conflicts.join("\n")));
    }
    history::ensure_history_in(db, history_table).await?;
    let applied = scoped_history(
        &history::read_history_in(db, history_table).await?,
        namespace,
    )?;
    let pending = pending_files(&state.files, &applied)?;
    let mut done = Vec::new();
    for file in pending {
        reject_blocked(file, allow_destructive)?;
        let history_id = history_key(namespace, &file.id);
        apply_one(db, file, history_table, &history_id, true).await?;
        done.push(history_id);
    }
    Ok(ApplyReport { applied: done })
}

pub async fn migration_status(
    db: &DatabaseConnection,
    directory: &Path,
    current: Option<&SchemaSnapshot>,
) -> Result<StatusReport, MigrationError> {
    migration_status_in(db, directory, history::HISTORY_TABLE, current).await
}

pub async fn migration_status_in(
    db: &DatabaseConnection,
    directory: &Path,
    history_table: &str,
    current: Option<&SchemaSnapshot>,
) -> Result<StatusReport, MigrationError> {
    let state = inspect_directory(directory);
    let backend = kind_of(db.get_database_backend())
        .map_err(|error| MigrationError::new(error.to_string()))?;
    let applied = if history::history_exists_in(db, history_table).await? {
        scoped_history(&history::read_history_in(db, history_table).await?, None)?
    } else {
        Vec::new()
    };
    let mut conflicts = state.conflicts;
    let mut lines = Vec::new();
    for file in &state.files {
        if let Some(entry) = applied.iter().find(|entry| entry.id == file.id) {
            if entry.checksum != file.checksum {
                conflicts.push(format!(
                    "Applied migration {} was changed. Stored checksum and file do not match.",
                    file.id
                ));
            }
            lines.push(MigrationLine {
                id: file.id.clone(),
                state: "angewendet".to_owned(),
                checksum: file.checksum.clone(),
            });
        } else {
            lines.push(MigrationLine {
                id: file.id.clone(),
                state: "ausstehend".to_owned(),
                checksum: file.checksum.clone(),
            });
        }
    }
    for entry in &applied {
        if !state.files.iter().any(|file| file.id == entry.id) {
            conflicts.push(format!(
                "Applied migration {} no longer has a file in the migrations directory.",
                entry.id
            ));
        }
    }
    if let Err(error) = pending_files(&state.files, &applied) {
        conflicts.push(error.0);
    }
    let schema_note = schema_note(current, &state.files, &applied);
    Ok(StatusReport {
        backend: history::backend_name(backend).to_owned(),
        lines,
        conflicts,
        schema_note,
    })
}

fn schema_note(
    current: Option<&SchemaSnapshot>,
    files: &[MigrationFile],
    applied: &[HistoryEntry],
) -> String {
    let Some(current) = current else {
        return "The model schema was not loaded and was therefore not compared.".to_owned();
    };
    let latest_applied = applied
        .last()
        .and_then(|entry| files.iter().find(|file| file.id == entry.id))
        .map(|file| &file.to_snapshot);
    let pending: Vec<&MigrationFile> = files
        .iter()
        .filter(|file| !applied.iter().any(|entry| entry.id == file.id))
        .collect();
    if let Some(last_pending) = pending.last() {
        if last_pending.to_snapshot.semantically_eq(current) {
            return "Pending migrations result in the current model schema.".to_owned();
        }
        return "The current model matches neither the last applied target nor the latest pending migration.".to_owned();
    }
    match latest_applied {
        Some(snapshot) if snapshot.semantically_eq(current) => {
            "The current model matches the last applied migration target.".to_owned()
        }
        Some(_) => "The current model differs from the last applied migration target.".to_owned(),
        None if current.semantically_eq(&SchemaSnapshot::empty()) => {
            "No migrations or model tables exist.".to_owned()
        }
        None => "Models exist but no migration has been created yet.".to_owned(),
    }
}

pub(crate) fn pending_files<'a>(
    files: &'a [MigrationFile],
    applied: &[HistoryEntry],
) -> Result<Vec<&'a MigrationFile>, MigrationError> {
    if applied.len() > files.len() {
        return Err(MigrationError::new(
            "The migration history contains more entries than the directory.".to_owned(),
        ));
    }
    for (index, entry) in applied.iter().enumerate() {
        let file = files.get(index).ok_or_else(|| {
            MigrationError::new(format!(
                "Applied migration {} is missing from the directory.",
                entry.id
            ))
        })?;
        if file.id != entry.id {
            return Err(MigrationError::new(format!(
                "History expects position {} to contain migration {}, but found {}.",
                index + 1,
                entry.id,
                file.id
            )));
        }
        if file.checksum != entry.checksum {
            return Err(MigrationError::new(format!(
                "Applied migration {} was changed. Stored checksum and file do not match.",
                file.id
            )));
        }
    }
    Ok(files.iter().skip(applied.len()).collect())
}

fn reject_blocked(file: &MigrationFile, allow_destructive: bool) -> Result<(), MigrationError> {
    let mut schema = file.from_snapshot.clone();
    for operation in &file.operations {
        let (class, reason) = classify(operation, &schema);
        schema = crate::simulate::apply_operations(&schema, std::slice::from_ref(operation))?;
        match class {
            ApplyClass::Safe => {}
            ApplyClass::Destructive if allow_destructive => {}
            ApplyClass::Destructive => {
                return Err(MigrationError::new(format!(
                    "Migration {} includes destructive operation `{}` and was not executed. Re-run with --allow-destructive only if data loss is intentional. {}",
                    file.id,
                    operation.summary(),
                    file.risks.iter().map(|risk| risk.message.as_str()).collect::<Vec<_>>().join(" ")
                )));
            }
            ApplyClass::Unsupported => {
                return Err(MigrationError::new(format!(
                    "Migration {} wurde abgebrochen: {}",
                    file.id,
                    reason
                        .unwrap_or_else(|| "This operation has no safe implementation.".to_owned())
                )));
            }
        }
    }
    Ok(())
}

fn validate_namespace(namespace: &str) -> Result<(), MigrationError> {
    if !crate::snapshot::is_identifier(namespace) || namespace.contains(':') || namespace.len() > 32
    {
        return Err(MigrationError::new(format!(
            "Invalid app name `{namespace}`."
        )));
    }
    Ok(())
}

fn history_key(namespace: Option<&str>, file_id: &str) -> String {
    match namespace {
        Some(namespace) => format!("{namespace}:{file_id}"),
        None => file_id.to_owned(),
    }
}

fn scoped_history(
    entries: &[HistoryEntry],
    namespace: Option<&str>,
) -> Result<Vec<HistoryEntry>, MigrationError> {
    let mut scoped = Vec::new();
    for entry in entries {
        match namespace {
            None => {
                if !entry.id.contains(':') {
                    scoped.push(entry.clone());
                }
            }
            Some(namespace) => {
                let prefix = format!("{namespace}:");
                if let Some(rest) = entry.id.strip_prefix(&prefix) {
                    if rest.is_empty() || rest.contains(':') {
                        return Err(MigrationError::new(format!(
                            "History ID `{}` is not fully qualified.",
                            entry.id
                        )));
                    }
                    scoped.push(HistoryEntry {
                        id: rest.to_owned(),
                        name: entry.name.clone(),
                        checksum: entry.checksum.clone(),
                        applied_at_unix: entry.applied_at_unix,
                    });
                }
            }
        }
    }
    Ok(scoped)
}

async fn status_scoped(
    db: &DatabaseConnection,
    directory: &Path,
    history_table: &str,
    current: Option<&SchemaSnapshot>,
    namespace: Option<&str>,
) -> Result<StatusReport, MigrationError> {
    let state = inspect_directory(directory);
    let backend = kind_of(db.get_database_backend())
        .map_err(|error| MigrationError::new(error.to_string()))?;
    let applied = if history::history_exists_in(db, history_table).await? {
        scoped_history(
            &history::read_history_in(db, history_table).await?,
            namespace,
        )?
    } else {
        Vec::new()
    };
    let mut conflicts = state.conflicts;
    let mut lines = Vec::new();
    for file in &state.files {
        let shown = history_key(namespace, &file.id);
        if let Some(entry) = applied.iter().find(|entry| entry.id == file.id) {
            if entry.checksum != file.checksum {
                conflicts.push(format!(
                    "Applied migration {shown} was changed. Stored checksum and file do not match."
                ));
            }
            lines.push(MigrationLine {
                id: shown,
                state: "angewendet".to_owned(),
                checksum: file.checksum.clone(),
            });
        } else {
            lines.push(MigrationLine {
                id: shown,
                state: "ausstehend".to_owned(),
                checksum: file.checksum.clone(),
            });
        }
    }
    for entry in &applied {
        if !state.files.iter().any(|file| file.id == entry.id) {
            conflicts.push(format!(
                "Applied migration {} no longer has a file in the migrations directory.",
                history_key(namespace, &entry.id)
            ));
        }
    }
    if let Err(error) = pending_files(&state.files, &applied) {
        conflicts.push(error.0);
    }
    Ok(StatusReport {
        backend: history::backend_name(backend).to_owned(),
        lines,
        conflicts,
        schema_note: schema_note(current, &state.files, &applied),
    })
}

async fn apply_one(
    db: &DatabaseConnection,
    file: &MigrationFile,
    history_table: &str,
    history_id: &str,
    record: bool,
) -> Result<(), MigrationError> {
    let backend = db.get_database_backend();
    let kind = kind_of(backend).map_err(|error| MigrationError::new(error.to_string()))?;
    let suspend_foreign_keys = matches!(kind, DatabaseKind::Sqlite)
        && file.operations.iter().any(|operation| {
            crate::rebuild::backfill_parts(operation).is_some()
                || crate::rebuild::added_foreign_key(operation).is_some()
                || crate::rebuild::dropped_foreign_key(operation).is_some()
                || crate::rebuild::alter_parts(operation).is_some()
        });
    if suspend_foreign_keys {
        return apply_sqlite_on_one_connection(db, file, history_table, history_id, record).await;
    }
    apply_one_in_transaction(db, file, history_table, history_id, record).await
}

async fn apply_sqlite_on_one_connection(
    db: &DatabaseConnection,
    file: &MigrationFile,
    history_table: &str,
    history_id: &str,
    record: bool,
) -> Result<(), MigrationError> {
    let connection = db
        .get_sqlite_connection_pool()
        .acquire()
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Could not acquire a SQLite connection for the rebuild: {error}"
            ))
        })?;
    let pinned = PinnedSqlite::new(connection);
    pinned
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys = OFF",
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Could not suspend foreign keys before rebuilding: {error}"
            ))
        })?;
    pinned.enforcement_suspended();
    let outcome = async {
        pinned
            .execute(Statement::from_string(DatabaseBackend::Sqlite, "BEGIN"))
            .await
            .map_err(|error| {
                MigrationError::new(format!("Could not start transaction: {error}"))
            })?;
        let work = apply_open(&pinned, file, history_table, history_id, record).await;
        match work {
            Ok(()) => match pinned
                .execute(Statement::from_string(DatabaseBackend::Sqlite, "COMMIT"))
                .await
            {
                Ok(_) => Ok(()),
                Err(error) => {
                    let _ = pinned
                        .execute(Statement::from_string(DatabaseBackend::Sqlite, "ROLLBACK"))
                        .await;
                    Err(MigrationError::new(format!(
                        "Could not commit migration {}: {error}",
                        file.id
                    )))
                }
            },
            Err(error) => {
                let _ = pinned
                    .execute(Statement::from_string(DatabaseBackend::Sqlite, "ROLLBACK"))
                    .await;
                Err(error)
            }
        }
    }
    .await;
    let restored = restore_sqlite_enforcement(&pinned).await;
    match (outcome, restored) {
        (Err(migration), _) => Err(migration),
        (Ok(()), Err(restore)) => Err(restore),
        (Ok(()), Ok(())) => Ok(()),
    }
}

async fn restore_sqlite_enforcement(connection: &PinnedSqlite) -> Result<(), MigrationError> {
    connection
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys = ON",
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Could not restore foreign key enforcement after rebuilding: {error}"
            ))
        })?;
    let row = connection
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys",
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Could not read foreign key enforcement after rebuilding: {error}"
            ))
        })?
        .ok_or_else(|| {
            MigrationError::new(
                "Could not read foreign key enforcement after rebuilding.".to_owned(),
            )
        })?;
    let enabled = row
        .try_get::<i64>("", "foreign_keys")
        .or_else(|_| row.try_get::<i32>("", "foreign_keys").map(i64::from))
        .map_err(|error| {
            MigrationError::new(format!(
                "Could not read foreign key enforcement after rebuilding: {error}"
            ))
        })?;
    if enabled != 1 {
        return Err(MigrationError::new(
            "Could not restore foreign key enforcement after rebuilding.".to_owned(),
        ));
    }
    connection.enforcement_restored();
    Ok(())
}

async fn apply_one_in_transaction(
    db: &DatabaseConnection,
    file: &MigrationFile,
    history_table: &str,
    history_id: &str,
    record: bool,
) -> Result<(), MigrationError> {
    let transaction = db
        .begin()
        .await
        .map_err(|error| MigrationError::new(format!("Could not start transaction: {error}")))?;
    apply_open(&transaction, file, history_table, history_id, record).await?;
    transaction.commit().await.map_err(|error| {
        MigrationError::new(format!("Could not commit migration {}: {error}", file.id))
    })?;
    Ok(())
}

async fn apply_open(
    transaction: &impl ConnectionTrait,
    file: &MigrationFile,
    history_table: &str,
    history_id: &str,
    record: bool,
) -> Result<(), MigrationError> {
    let backend = transaction.get_database_backend();
    let kind = kind_of(backend).map_err(|error| MigrationError::new(error.to_string()))?;
    let mut schema = file.from_snapshot.clone();
    for operation in &file.operations {
        if let Some((table, column, backfill, foreign_key)) =
            crate::rebuild::backfill_parts(operation)
        {
            crate::rebuild::apply_backfill_column(
                transaction,
                kind,
                &schema,
                table,
                column,
                backfill,
                foreign_key,
            )
            .await
            .map_err(|error| {
                MigrationError::new(format!(
                    "Migration {} was rolled back at `{}`: {}",
                    file.id,
                    operation.summary(),
                    error.0
                ))
            })?;
        } else if let Some((table, name)) = crate::rebuild::dropped_foreign_key(operation) {
            crate::rebuild::apply_drop_foreign_key(transaction, kind, &schema, table, name)
                .await
                .map_err(|error| {
                    MigrationError::new(format!(
                        "Migration {} was rolled back at `{}`: {}",
                        file.id,
                        operation.summary(),
                        error.0
                    ))
                })?;
        } else if let Some((table, foreign_key)) = crate::rebuild::added_foreign_key(operation) {
            crate::rebuild::apply_add_foreign_key(transaction, kind, &schema, table, foreign_key)
                .await
                .map_err(|error| {
                    MigrationError::new(format!(
                        "Migration {} was rolled back at `{}`: {}",
                        file.id,
                        operation.summary(),
                        error.0
                    ))
                })?;
        } else if let Some((table, from, to)) = crate::rebuild::alter_parts(operation) {
            crate::rebuild::apply_alter(transaction, kind, &schema, table, from, to)
                .await
                .map_err(|error| {
                    MigrationError::new(format!(
                        "Migration {} was rolled back at `{}`: {}",
                        file.id,
                        operation.summary(),
                        error.0
                    ))
                })?;
        } else {
            for sql in render_operation(kind, operation)? {
                transaction
                    .execute(Statement::from_string(backend, sql))
                    .await
                    .map_err(|error| map_execute_error(&file.id, operation, &error.to_string()))?;
            }
            for sql in followup_index_renames(kind, &schema, operation) {
                transaction
                    .execute(Statement::from_string(backend, sql))
                    .await
                    .map_err(|error| map_execute_error(&file.id, operation, &error.to_string()))?;
            }
        }
        schema = crate::simulate::apply_operations(&schema, std::slice::from_ref(operation))?;
    }
    if record {
        let applied_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| MigrationError::new(format!("Invalid system time: {error}")))?
            .as_secs() as i64;
        history::insert_history_in(
            transaction,
            history_table,
            history_id,
            &file.name,
            &file.checksum,
            applied_at_unix,
        )
        .await?;
    } else {
        history::delete_history_in(transaction, history_table, history_id).await?;
    }
    Ok(())
}

fn render_operation(
    kind: DatabaseKind,
    operation: &MigrationOperation,
) -> Result<Vec<String>, MigrationError> {
    Ok(match operation {
        MigrationOperation::CreateTable { table } => {
            let columns = physical_columns(&table.columns)?;
            let uniques = table
                .unique_constraints
                .iter()
                .map(|constraint| {
                    (
                        constraint.name.as_str(),
                        constraint
                            .columns
                            .iter()
                            .map(String::as_str)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();
            let unique_refs = uniques
                .iter()
                .map(|(name, columns)| (*name, columns.as_slice()))
                .collect::<Vec<_>>();
            let foreign_keys = table
                .foreign_keys
                .iter()
                .map(|foreign_key| ForeignKeyClause {
                    name: foreign_key.name.as_str(),
                    column: foreign_key.columns[0].as_str(),
                    referenced_table: foreign_key.referenced_table.as_str(),
                    referenced_column: foreign_key.referenced_columns[0].as_str(),
                    on_delete: referential_action(foreign_key.on_delete),
                })
                .collect::<Vec<_>>();
            let indexes = table
                .indexes
                .iter()
                .map(|index| {
                    (
                        index.name.as_str(),
                        index.columns.iter().map(String::as_str).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();
            let index_refs = indexes
                .iter()
                .map(|(name, columns)| (*name, columns.as_slice()))
                .collect::<Vec<_>>();
            let mut statements = render_table_bundle(
                kind,
                &table.name,
                &columns,
                &unique_refs,
                &foreign_keys,
                &index_refs,
            );
            if let Some(create) = statements.first_mut() {
                let checks = choice_check_clauses(&table.columns);
                *create = with_checks(std::mem::take(create), &checks);
            }
            statements
        }
        MigrationOperation::DropTable { name } => vec![render_drop_table(kind, name)],
        MigrationOperation::AddColumn {
            table,
            column,
            foreign_key,
            backfill: _,
        } => {
            let physical = physical_column(column)?;
            let clause = foreign_key.as_ref().map(|foreign_key| ForeignKeyClause {
                name: foreign_key.name.as_str(),
                column: foreign_key.columns[0].as_str(),
                referenced_table: foreign_key.referenced_table.as_str(),
                referenced_column: foreign_key.referenced_columns[0].as_str(),
                on_delete: referential_action(foreign_key.on_delete),
            });
            vec![render_add_column_referencing(
                kind,
                table,
                &physical,
                clause.as_ref(),
            )]
        }
        MigrationOperation::DropColumn { table, name } => {
            vec![render_drop_column(kind, table, name)]
        }
        MigrationOperation::RenameTable { from, to } => vec![render_rename_table(kind, from, to)],
        MigrationOperation::RenameColumn { table, from, to } => {
            vec![render_rename_column(kind, table, from, to)]
        }
        MigrationOperation::AddUniqueConstraint { table, constraint } => {
            let columns = constraint
                .columns
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            vec![render_create_index(
                kind,
                &constraint.name,
                table,
                &columns,
                true,
            )]
        }
        MigrationOperation::DropUniqueConstraint { name, .. }
        | MigrationOperation::DropIndex { name, .. } => vec![render_drop_index(kind, name)],
        MigrationOperation::AddIndex { table, index } => {
            let columns = index.columns.iter().map(String::as_str).collect::<Vec<_>>();
            vec![render_create_index(
                kind,
                &index.name,
                table,
                &columns,
                false,
            )]
        }
        MigrationOperation::AddForeignKey { .. } => {
            return Err(MigrationError::new(
                "Adding a foreign key is handled by the verified table rebuild, not as standalone DDL."
                    .to_owned(),
            ));
        }
        MigrationOperation::DropForeignKey { .. } => {
            return Err(MigrationError::new(
                "Dropping a foreign key is handled by the verified table rebuild, not as standalone DDL."
                    .to_owned(),
            ));
        }
        MigrationOperation::AlterColumn { .. } => {
            return Err(MigrationError::new(
                "ALTER COLUMN is handled by the verified table rebuild, not as standalone DDL."
                    .to_owned(),
            ));
        }
    })
}

fn followup_index_renames(
    kind: DatabaseKind,
    schema: &SchemaSnapshot,
    operation: &MigrationOperation,
) -> Vec<String> {
    match operation {
        MigrationOperation::RenameColumn { table, from, to } => {
            let Some(found) = schema
                .tables
                .iter()
                .find(|candidate| candidate.name == *table)
            else {
                return Vec::new();
            };
            let mut statements = Vec::new();
            for unique in &found.unique_constraints {
                if !unique.columns.iter().any(|column| column == from) {
                    continue;
                }
                let columns = renamed_columns(&unique.columns, from, to);
                let new_name = unique_index_name(table, &columns);
                if new_name != unique.name {
                    statements.push(render_drop_index(kind, &unique.name));
                    statements.push(render_create_index(kind, &new_name, table, &columns, true));
                }
            }
            for index in &found.indexes {
                if !index.columns.iter().any(|column| column == from) {
                    continue;
                }
                let columns = renamed_columns(&index.columns, from, to);
                let new_name = secondary_index_name(table, &columns);
                if new_name != index.name {
                    statements.push(render_drop_index(kind, &index.name));
                    statements.push(render_create_index(kind, &new_name, table, &columns, false));
                }
            }
            statements
        }
        MigrationOperation::RenameTable { from, to } => {
            let Some(found) = schema
                .tables
                .iter()
                .find(|candidate| candidate.name == *from)
            else {
                return Vec::new();
            };
            let mut statements = Vec::new();
            for unique in &found.unique_constraints {
                let columns = unique
                    .columns
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                let new_name = unique_index_name(to, &columns);
                statements.push(render_drop_index(kind, &unique.name));
                statements.push(render_create_index(kind, &new_name, to, &columns, true));
            }
            for index in &found.indexes {
                let columns = index.columns.iter().map(String::as_str).collect::<Vec<_>>();
                let new_name = secondary_index_name(to, &columns);
                statements.push(render_drop_index(kind, &index.name));
                statements.push(render_create_index(kind, &new_name, to, &columns, false));
            }
            statements
        }
        _ => Vec::new(),
    }
}

fn renamed_columns<'a>(columns: &'a [String], from: &str, to: &'a str) -> Vec<&'a str> {
    columns
        .iter()
        .map(|column| if column == from { to } else { column.as_str() })
        .collect()
}

fn referential_action(action: crate::snapshot::OnDelete) -> ReferentialAction {
    match action {
        crate::snapshot::OnDelete::Restrict => ReferentialAction::Restrict,
        crate::snapshot::OnDelete::Cascade => ReferentialAction::Cascade,
        crate::snapshot::OnDelete::SetNull => ReferentialAction::SetNull,
    }
}

pub async fn reseal_unapplied(
    db: &DatabaseConnection,
    path: &Path,
) -> Result<crate::store::MigrationFile, MigrationError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        MigrationError::new(format!(
            "Could not read migration {}: {error}",
            path.display()
        ))
    })?;
    let id = serde_json::from_str::<crate::store::MigrationFile>(&text)
        .map_err(|error| MigrationError::new(format!("Migration is not valid JSON: {error}")))?
        .id;
    let history = history::read_history(db).await?;
    if history.iter().any(|entry| entry.id == id) {
        return Err(MigrationError::new(format!(
            "Migration {id} is already applied. The checksum of an applied migration cannot be rewritten."
        )));
    }
    crate::store::reseal_migration(path)
}

fn choice_check_clauses(columns: &[ColumnSnapshot]) -> Vec<String> {
    columns
        .iter()
        .filter(|column| column.logical_type == crate::snapshot::LogicalType::Choice)
        .map(|column| {
            let list = column
                .choices
                .iter()
                .map(|choice| format!("'{}'", choice.value))
                .collect::<Vec<_>>()
                .join(", ");
            format!("CHECK (\"{}\" IN ({list}))", column.name)
        })
        .collect()
}

fn physical_columns(columns: &[ColumnSnapshot]) -> Result<Vec<PhysicalColumn<'_>>, MigrationError> {
    columns.iter().map(physical_column).collect()
}

fn physical_column(column: &ColumnSnapshot) -> Result<PhysicalColumn<'_>, MigrationError> {
    if column.default.is_some() && column.logical_type != crate::snapshot::LogicalType::Bool {
        return Err(MigrationError::new(format!(
            "Column `{}` has a default that is supported only for bool.",
            column.name
        )));
    }
    Ok(PhysicalColumn {
        name: &column.name,
        kind: column.logical_type.into(),
        nullable: column.nullable,
        primary_key: column.primary_key,
        default_bool: column.default,
    })
}

struct PinnedSqlite {
    connection:
        tokio::sync::Mutex<Option<sea_orm::sqlx::pool::PoolConnection<sea_orm::sqlx::Sqlite>>>,
    enforcement_intact: AtomicBool,
}

impl PinnedSqlite {
    fn new(connection: sea_orm::sqlx::pool::PoolConnection<sea_orm::sqlx::Sqlite>) -> Self {
        Self {
            connection: tokio::sync::Mutex::new(Some(connection)),
            enforcement_intact: AtomicBool::new(true),
        }
    }

    fn enforcement_suspended(&self) {
        self.enforcement_intact.store(false, Ordering::SeqCst);
    }

    fn enforcement_restored(&self) {
        self.enforcement_intact.store(true, Ordering::SeqCst);
    }

    async fn live(
        &self,
    ) -> Result<
        tokio::sync::MutexGuard<
            '_,
            Option<sea_orm::sqlx::pool::PoolConnection<sea_orm::sqlx::Sqlite>>,
        >,
        DbErr,
    > {
        Ok(self.connection.lock().await)
    }
}

impl Drop for PinnedSqlite {
    fn drop(&mut self) {
        let connection = self.connection.get_mut().take();
        if let Some(connection) = connection {
            if self.enforcement_intact.load(Ordering::SeqCst) {
                drop(connection);
            } else {
                drop(connection.detach());
            }
        }
    }
}

fn statement_query(
    statement: &Statement,
) -> sea_orm::sqlx::query::Query<'_, sea_orm::sqlx::Sqlite, SqlxValues> {
    let values = statement
        .values
        .clone()
        .unwrap_or_else(|| sea_orm::sea_query::Values(Vec::new()));
    sea_orm::sqlx::query_with(&statement.sql, SqlxValues(values))
}

#[async_trait::async_trait]
impl ConnectionTrait for PinnedSqlite {
    fn get_database_backend(&self) -> DatabaseBackend {
        DatabaseBackend::Sqlite
    }

    async fn execute(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        let mut guard = self.live().await?;
        let Some(connection) = guard.as_mut() else {
            return Err(DbErr::Conn(RuntimeErr::Internal(
                "SQLite connection is closed".to_owned(),
            )));
        };
        statement_query(&statement)
            .execute(&mut **connection)
            .await
            .map(ExecResult::from)
            .map_err(|error| DbErr::Exec(RuntimeErr::SqlxError(error)))
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        let mut guard = self.live().await?;
        let Some(connection) = guard.as_mut() else {
            return Err(DbErr::Conn(RuntimeErr::Internal(
                "SQLite connection is closed".to_owned(),
            )));
        };
        sea_orm::sqlx::Executor::execute(&mut **connection, sql)
            .await
            .map(ExecResult::from)
            .map_err(|error| DbErr::Exec(RuntimeErr::SqlxError(error)))
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        let mut guard = self.live().await?;
        let Some(connection) = guard.as_mut() else {
            return Err(DbErr::Conn(RuntimeErr::Internal(
                "SQLite connection is closed".to_owned(),
            )));
        };
        match statement_query(&statement)
            .fetch_optional(&mut **connection)
            .await
        {
            Ok(Some(row)) => Ok(Some(QueryResult::from(row))),
            Ok(None) => Ok(None),
            Err(error) => Err(DbErr::Query(RuntimeErr::SqlxError(error))),
        }
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        let mut guard = self.live().await?;
        let Some(connection) = guard.as_mut() else {
            return Err(DbErr::Conn(RuntimeErr::Internal(
                "SQLite connection is closed".to_owned(),
            )));
        };
        statement_query(&statement)
            .fetch_all(&mut **connection)
            .await
            .map(|rows| rows.into_iter().map(QueryResult::from).collect())
            .map_err(|error| DbErr::Query(RuntimeErr::SqlxError(error)))
    }
}

fn map_execute_error(id: &str, operation: &MigrationOperation, detail: &str) -> MigrationError {
    let lower = detail.to_ascii_lowercase();
    if lower.contains("already exists") || lower.contains("duplicate") {
        return MigrationError::new(format!(
            "Migration {id} was rolled back. `{}` conflicts with an existing object. Existing tables are not marked as migrated. Use a new database for a clean migration state; existing files are not deleted.",
            operation.summary()
        ));
    }
    MigrationError::new(format!(
        "Migration {id} was rolled back at `{}`: {detail}",
        operation.summary()
    ))
}
