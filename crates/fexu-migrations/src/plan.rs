//! Read-only comparison of migration files, history, and the live schema.
//! Nothing in this module creates, alters, or drops a database object.

use crate::error::MigrationError;
use crate::history::{self, HistoryEntry};
use crate::introspection::{compare_snapshot, introspect_live};
use crate::runner::pending_files;
use crate::snapshot::SchemaSnapshot;
use crate::store::{inspect_directory, MigrationFile};
use fexu_db::driver::{ConnectionTrait, DatabaseConnection};
use fexu_db::kind_of;
use std::fmt::{Display, Formatter};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanLevel {
    Error,
    Drift,
    Warning,
}

impl Display for PlanLevel {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Error => "error",
            Self::Drift => "drift",
            Self::Warning => "warning",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanFinding {
    pub level: PlanLevel,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanReport {
    pub backend: String,
    pub pending: Vec<String>,
    pub findings: Vec<PlanFinding>,
}

impl PlanReport {
    pub fn has_error(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.level == PlanLevel::Error)
    }

    pub fn has_drift(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.level == PlanLevel::Drift)
    }
}

pub async fn plan_database(
    db: &DatabaseConnection,
    directory: &Path,
    current: Option<&SchemaSnapshot>,
) -> Result<PlanReport, MigrationError> {
    plan_database_in(db, directory, history::HISTORY_TABLE, current).await
}

pub async fn plan_database_in(
    db: &DatabaseConnection,
    directory: &Path,
    history_table: &str,
    current: Option<&SchemaSnapshot>,
) -> Result<PlanReport, MigrationError> {
    let state = inspect_directory(directory);
    let backend = kind_of(db.get_database_backend())
        .map_err(|error| MigrationError::new(error.to_string()))?;
    let mut findings = state
        .conflicts
        .iter()
        .map(|message| finding(PlanLevel::Error, message))
        .collect::<Vec<_>>();
    let history_present = history::history_exists_in(db, history_table).await?;
    let applied = if history_present {
        history::read_history_in(db, history_table).await?
    } else {
        Vec::new()
    };
    for entry in &applied {
        if !state.files.iter().any(|file| file.id == entry.id) {
            findings.push(finding(
                PlanLevel::Error,
                format!(
                    "Applied migration {} no longer has a readable file in the migrations directory.",
                    entry.id
                ),
            ));
        }
    }
    let pending = match pending_files(&state.files, &applied) {
        Ok(files) => files.iter().map(|file| file.id.clone()).collect::<Vec<_>>(),
        Err(error) => {
            findings.push(finding(PlanLevel::Error, error.0));
            state
                .files
                .iter()
                .filter(|file| !applied.iter().any(|entry| entry.id == file.id))
                .map(|file| file.id.clone())
                .collect()
        }
    };
    let trusted = !findings.iter().any(|item| item.level == PlanLevel::Error);
    if trusted {
        if let Some(file) = last_applied(&state.files, &applied) {
            let live = introspect_live(db).await?;
            for difference in compare_snapshot(&file.to_snapshot, &live) {
                findings.push(finding(PlanLevel::Drift, drift_text(&difference)));
            }
        } else if !state.files.is_empty() {
            findings.push(finding(
                PlanLevel::Warning,
                "No migration has been applied. The live schema was not compared with a pending target.",
            ));
        }
        if let Some(current) = current {
            note_model(
                &state.files,
                &pending,
                last_applied(&state.files, &applied),
                current,
                &mut findings,
            );
        }
    }
    Ok(PlanReport {
        backend: history::backend_name(backend).to_owned(),
        pending,
        findings,
    })
}

fn last_applied<'a>(
    files: &'a [MigrationFile],
    applied: &[HistoryEntry],
) -> Option<&'a MigrationFile> {
    applied
        .last()
        .and_then(|entry| files.iter().find(|file| file.id == entry.id))
}

fn note_model(
    files: &[MigrationFile],
    pending: &[String],
    applied: Option<&MigrationFile>,
    current: &SchemaSnapshot,
    findings: &mut Vec<PlanFinding>,
) {
    if let Some(id) = pending.last() {
        if let Some(file) = files.iter().find(|file| &file.id == id) {
            if !file.to_snapshot.semantically_eq(current) {
                findings.push(finding(
                    PlanLevel::Warning,
                    "The current model does not match the latest pending migration. That difference is not reported as live drift.",
                ));
            }
        }
        return;
    }
    if let Some(file) = applied {
        if !file.to_snapshot.semantically_eq(current) {
            findings.push(finding(
                PlanLevel::Warning,
                "The current model differs from the last applied migration. Create a migration before changing the database.",
            ));
        }
        return;
    }
    if !current.semantically_eq(&SchemaSnapshot::empty()) {
        findings.push(finding(
            PlanLevel::Warning,
            "Models exist but no migration has been created yet.",
        ));
    }
}

fn drift_text(message: &str) -> String {
    if let Some(name) = message
        .strip_prefix("Unbekannte Tabelle `")
        .and_then(|rest| rest.strip_suffix("` blockiert die Baseline."))
    {
        return format!("Table `{name}` is not part of the last applied migration.");
    }
    message.replace(
        "prevents baselining.",
        "is not part of the last applied migration.",
    )
}

fn finding(level: PlanLevel, message: impl Into<String>) -> PlanFinding {
    PlanFinding {
        level,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{ColumnSnapshot, LogicalType, TableSnapshot};
    use crate::store::make_migration;
    use fexu_db::driver::{ConnectionTrait, DatabaseBackend, Statement};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn products() -> SchemaSnapshot {
        SchemaSnapshot::from_tables(vec![TableSnapshot {
            name: "products".to_owned(),
            columns: vec![ColumnSnapshot {
                name: "id".to_owned(),
                logical_type: LogicalType::I64,
                primary_key: true,
                nullable: false,
                default: None,
                precision: None,
                scale: None,
                choices: Vec::new(),
            }],
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
        }])
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fexu-plan-{label}-{nanos}"));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn sqlite_url(file: &std::path::Path) -> String {
        format!(
            "sqlite://{}?mode=rwc",
            file.to_string_lossy().replace('\\', "/")
        )
    }

    async fn names(db: &DatabaseConnection) -> Vec<String> {
        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
            ))
            .await
            .unwrap();
        rows.into_iter()
            .map(|row| row.try_get::<String>("", "name").unwrap())
            .collect()
    }

    #[tokio::test]
    async fn pending_plan_does_not_create_tables() {
        let root = scratch("pending");
        let app = root.join("migrations");
        std::fs::create_dir_all(&app).unwrap();
        make_migration(&app, "initial", &products()).unwrap();
        let db = fexu_db::connect("sqlite::memory:").await.unwrap();
        let before = names(&db).await;
        let report = plan_database(&db, &app, Some(&SchemaSnapshot::empty()))
            .await
            .unwrap();
        assert!(!report.has_error());
        assert!(!report.has_drift());
        assert_eq!(report.pending.len(), 1);
        assert!(report
            .findings
            .iter()
            .any(|item| item.level == PlanLevel::Warning));
        assert_eq!(names(&db).await, before);
        assert!(!history::table_exists(&db, "products").await.unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn applied_plan_is_quiet_and_drift_is_read_only() {
        let root = scratch("drift");
        let app = root.join("migrations");
        std::fs::create_dir_all(&app).unwrap();
        make_migration(&app, "initial", &products()).unwrap();
        let file = root.join("db.sqlite");
        let db = fexu_db::connect(&sqlite_url(&file)).await.unwrap();
        crate::apply_directory(&db, &app, false).await.unwrap();
        drop(db);
        let read_url = crate::readonly_database_url(&sqlite_url(&file)).unwrap();
        let before = std::fs::read(&file).unwrap();
        let db = fexu_db::connect(&read_url).await.unwrap();
        let clean = plan_database(&db, &app, Some(&products())).await.unwrap();
        drop(db);
        assert!(!clean.has_error(), "{clean:?}");
        assert!(!clean.has_drift(), "{clean:?}");
        assert!(clean.pending.is_empty());
        assert!(clean.findings.is_empty(), "{clean:?}");
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let db = fexu_db::connect(&sqlite_url(&file)).await.unwrap();
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "ALTER TABLE products ADD COLUMN extra integer",
        ))
        .await
        .unwrap();
        drop(db);
        let changed = std::fs::read(&file).unwrap();
        let db = fexu_db::connect(&read_url).await.unwrap();
        let drifted = plan_database(&db, &app, Some(&products())).await.unwrap();
        drop(db);
        assert!(drifted.has_drift(), "{drifted:?}");
        assert!(drifted
            .findings
            .iter()
            .any(|item| item.message.contains("extra")));
        assert_eq!(std::fs::read(&file).unwrap(), changed);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn checksum_conflict_is_an_error_without_a_false_drift_report() {
        let root = scratch("checksum");
        let app = root.join("migrations");
        std::fs::create_dir_all(&app).unwrap();
        make_migration(&app, "initial", &products()).unwrap();
        let file = root.join("db.sqlite");
        let db = fexu_db::connect(&sqlite_url(&file)).await.unwrap();
        crate::apply_directory(&db, &app, false).await.unwrap();
        drop(db);
        let migration = std::fs::read_dir(&app)
            .unwrap()
            .find_map(|entry| entry.ok())
            .unwrap()
            .path();
        let mut parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&migration).unwrap()).unwrap();
        parsed["checksum"] = serde_json::Value::String("00".repeat(32));
        std::fs::write(&migration, serde_json::to_string_pretty(&parsed).unwrap()).unwrap();
        let before = std::fs::read(&file).unwrap();
        let db = fexu_db::connect(&crate::readonly_database_url(&sqlite_url(&file)).unwrap())
            .await
            .unwrap();
        let report = plan_database(&db, &app, Some(&products())).await.unwrap();
        drop(db);
        assert!(report.has_error(), "{report:?}");
        assert!(!report.has_drift(), "{report:?}");
        assert!(report
            .findings
            .iter()
            .any(|item| item.message.to_ascii_lowercase().contains("checksum")));
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL pointing at an approved PostgreSQL admin database"]
    async fn postgres_plan_detects_live_drift_on_a_disposable_database() {
        let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let isolated = fexu_db::DisposablePostgres::create(&admin)
            .await
            .expect("isolated database");
        let root = scratch("postgres");
        let app = root.join("migrations");
        std::fs::create_dir_all(&app).unwrap();
        make_migration(&app, "initial", &products()).unwrap();
        let result = async {
            let db = fexu_db::connect(isolated.url())
                .await
                .map_err(|error| error.to_string())?;
            crate::apply_directory(&db, &app, false)
                .await
                .map_err(|error| error.0)?;
            let clean = plan_database(&db, &app, Some(&products()))
                .await
                .map_err(|error| error.0)?;
            if clean.has_error() || clean.has_drift() || !clean.findings.is_empty() {
                return Err(format!("clean postgres plan was not quiet: {clean:?}"));
            }
            db.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                "ALTER TABLE products ADD COLUMN extra integer",
            ))
            .await
            .map_err(|error| error.to_string())?;
            let drifted = plan_database(&db, &app, Some(&products()))
                .await
                .map_err(|error| error.0)?;
            if !drifted.has_drift()
                || !drifted
                    .findings
                    .iter()
                    .any(|item| item.message.contains("extra"))
            {
                return Err(format!("postgres drift was not reported: {drifted:?}"));
            }
            Ok(())
        }
        .await;
        let dropped = isolated.drop_database().await;
        result.expect("postgres plan");
        dropped.expect("drop isolated database");
        let _ = std::fs::remove_dir_all(&root);
    }
}
