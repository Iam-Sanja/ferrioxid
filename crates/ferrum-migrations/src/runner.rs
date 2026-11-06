//! Ausführung eines geprüften Migrationsplans. Unsichere Operationen werden nicht angefangen.

use crate::diff::{classify, ApplyClass, MigrationOperation};
use crate::error::MigrationError;
use crate::history::{self, HistoryEntry};
use crate::snapshot::{ColumnSnapshot, SchemaSnapshot};
use crate::store::{inspect_directory, MigrationFile};
use ferrum_core::DatabaseKind;
use ferrum_db::{
    kind_of, render_add_column, render_create_table, render_drop_column, render_drop_table,
    PhysicalColumn,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct ApplyReport {
    pub applied: Vec<String>,
}

#[derive(Debug)]
pub struct MigrationLine {
    pub id: String,
    pub state: String,
    pub checksum: String,
}

#[derive(Debug)]
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
            text.push_str("Migrationen: keine\n");
        } else {
            text.push_str("Migrationen:\n");
            for line in &self.lines {
                text.push_str(&format!(
                    "  {}  {}  checksum {}\n",
                    line.id, line.state, line.checksum
                ));
            }
        }
        if self.conflicts.is_empty() {
            text.push_str("Konflikte: keine\n");
        } else {
            text.push_str("Konflikte:\n");
            for conflict in &self.conflicts {
                text.push_str(&format!("  {conflict}\n"));
            }
        }
        text.push_str(&format!("Schema: {}\n", self.schema_note));
        text
    }
}

pub async fn apply_directory(
    db: &DatabaseConnection,
    directory: &Path,
    allow_destructive: bool,
) -> Result<ApplyReport, MigrationError> {
    let state = inspect_directory(directory);
    if !state.conflicts.is_empty() {
        return Err(MigrationError::new(state.conflicts.join("\n")));
    }
    history::ensure_history(db).await?;
    let applied = history::read_history(db).await?;
    let pending = pending_files(&state.files, &applied)?;
    let mut done = Vec::new();
    for file in pending {
        reject_blocked(file, allow_destructive)?;
        apply_one(db, file).await?;
        done.push(file.id.clone());
    }
    Ok(ApplyReport { applied: done })
}

pub async fn migration_status(
    db: &DatabaseConnection,
    directory: &Path,
    current: Option<&SchemaSnapshot>,
) -> Result<StatusReport, MigrationError> {
    let state = inspect_directory(directory);
    let backend = kind_of(db.get_database_backend())
        .map_err(|error| MigrationError::new(error.to_string()))?;
    let applied = if history::history_exists(db).await? {
        history::read_history(db).await?
    } else {
        Vec::new()
    };
    let mut conflicts = state.conflicts;
    let mut lines = Vec::new();
    for file in &state.files {
        if let Some(entry) = applied.iter().find(|entry| entry.id == file.id) {
            if entry.checksum != file.checksum {
                conflicts.push(format!(
                    "Angewendete Migration {} wurde verändert. Gespeicherte Checksum und Datei stimmen nicht überein.",
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
                "Angewendete Migration {} hat keine Datei mehr im Migrationsverzeichnis.",
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
        return "Modellschema wurde nicht geladen und deshalb nicht verglichen.".to_owned();
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
            return "Ausstehende Migrationen führen zum aktuellen Modell.".to_owned();
        }
        return "Das aktuelle Modell entspricht weder dem letzten angewendeten Ziel noch der letzten ausstehenden Migration.".to_owned();
    }
    match latest_applied {
        Some(snapshot) if snapshot.semantically_eq(current) => {
            "Das aktuelle Modell entspricht dem letzten angewendeten Migrationsziel.".to_owned()
        }
        Some(_) => {
            "Das aktuelle Modell weicht vom letzten angewendeten Migrationsziel ab.".to_owned()
        }
        None if current.semantically_eq(&SchemaSnapshot::empty()) => {
            "Es gibt weder Migrationen noch Modelltabellen.".to_owned()
        }
        None => "Es gibt Modelle, aber noch keine Migration.".to_owned(),
    }
}

fn pending_files<'a>(
    files: &'a [MigrationFile],
    applied: &[HistoryEntry],
) -> Result<Vec<&'a MigrationFile>, MigrationError> {
    if applied.len() > files.len() {
        return Err(MigrationError::new(
            "Die Historie enthält mehr Migrationen als das Verzeichnis.".to_owned(),
        ));
    }
    for (index, entry) in applied.iter().enumerate() {
        let file = files.get(index).ok_or_else(|| {
            MigrationError::new(format!(
                "Angewendete Migration {} fehlt im Verzeichnis.",
                entry.id
            ))
        })?;
        if file.id != entry.id {
            return Err(MigrationError::new(format!(
                "Die Historie erwartet an Position {} die Migration {}, gefunden wurde {}.",
                index + 1,
                entry.id,
                file.id
            )));
        }
        if file.checksum != entry.checksum {
            return Err(MigrationError::new(format!(
                "Angewendete Migration {} wurde verändert. Gespeicherte Checksum und Datei stimmen nicht überein.",
                file.id
            )));
        }
    }
    Ok(files.iter().skip(applied.len()).collect())
}

fn reject_blocked(file: &MigrationFile, allow_destructive: bool) -> Result<(), MigrationError> {
    for operation in &file.operations {
        let (class, reason) = classify(operation, &file.from_snapshot);
        match class {
            ApplyClass::Safe => {}
            ApplyClass::Destructive if allow_destructive => {}
            ApplyClass::Destructive => {
                return Err(MigrationError::new(format!(
                    "Migration {} enthält die riskante Operation `{}` und wurde nicht ausgeführt. Wiederhole den Lauf mit --allow-destructive, wenn der Datenverlust beabsichtigt ist. {}",
                    file.id,
                    operation.summary(),
                    file.risks.iter().map(|risk| risk.message.as_str()).collect::<Vec<_>>().join(" ")
                )));
            }
            ApplyClass::Unsupported => {
                return Err(MigrationError::new(format!(
                    "Migration {} wurde abgebrochen: {}",
                    file.id,
                    reason.unwrap_or_else(
                        || "Die Operation ist nicht sicher implementiert.".to_owned()
                    )
                )));
            }
        }
    }
    Ok(())
}

async fn apply_one(db: &DatabaseConnection, file: &MigrationFile) -> Result<(), MigrationError> {
    let backend = db.get_database_backend();
    let kind = kind_of(backend).map_err(|error| MigrationError::new(error.to_string()))?;
    let transaction = db.begin().await.map_err(|error| {
        MigrationError::new(format!(
            "Transaktion konnte nicht gestartet werden: {error}"
        ))
    })?;
    for operation in &file.operations {
        let sql = render_operation(kind, operation)?;
        transaction
            .execute(Statement::from_string(backend, sql))
            .await
            .map_err(|error| map_execute_error(&file.id, operation, &error.to_string()))?;
    }
    let applied_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| MigrationError::new(format!("Systemzeit ist ungültig: {error}")))?
        .as_secs() as i64;
    history::insert_history(
        &transaction,
        &file.id,
        &file.name,
        &file.checksum,
        applied_at_unix,
    )
    .await?;
    transaction.commit().await.map_err(|error| {
        MigrationError::new(format!(
            "Migration {} konnte nicht festgeschrieben werden: {error}",
            file.id
        ))
    })?;
    Ok(())
}

fn render_operation(
    kind: DatabaseKind,
    operation: &MigrationOperation,
) -> Result<String, MigrationError> {
    Ok(match operation {
        MigrationOperation::CreateTable { table } => {
            let columns = physical_columns(&table.columns)?;
            render_create_table(kind, &table.name, &columns, false)
        }
        MigrationOperation::DropTable { name } => render_drop_table(kind, name),
        MigrationOperation::AddColumn { table, column } => {
            let physical = physical_column(column)?;
            render_add_column(kind, table, &physical)
        }
        MigrationOperation::DropColumn { table, name } => render_drop_column(kind, table, name),
        MigrationOperation::AlterColumn { .. } => {
            return Err(MigrationError::new(
                "ALTER COLUMN ist nicht als sichere Operation implementiert.".to_owned(),
            ));
        }
    })
}

fn physical_columns(columns: &[ColumnSnapshot]) -> Result<Vec<PhysicalColumn<'_>>, MigrationError> {
    columns.iter().map(physical_column).collect()
}

fn physical_column(column: &ColumnSnapshot) -> Result<PhysicalColumn<'_>, MigrationError> {
    if column.default.is_some() && column.logical_type != crate::snapshot::LogicalType::Bool {
        return Err(MigrationError::new(format!(
            "Spalte `{}` hat einen Default, der nur für bool erlaubt ist.",
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

fn map_execute_error(id: &str, operation: &MigrationOperation, detail: &str) -> MigrationError {
    let lower = detail.to_ascii_lowercase();
    if lower.contains("already exists") || lower.contains("duplicate") {
        return MigrationError::new(format!(
            "Migration {id} wurde zurückgerollt. `{}` trifft auf ein vorhandenes Objekt. Vorhandene Tabellen werden nicht als migriert markiert. Lege für einen sauberen Migrationsstand eine neue Datenbank an; bestehende Dateien werden nicht gelöscht.",
            operation.summary()
        ));
    }
    MigrationError::new(format!(
        "Migration {id} wurde zurückgerollt bei `{}`: {detail}",
        operation.summary()
    ))
}
