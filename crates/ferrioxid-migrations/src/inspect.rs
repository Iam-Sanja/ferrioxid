//! Read-only migration diagnostics for authentication and app directories.
//!
//! Diese Funktionen legen keine Historientabellen an und wenden keine Migration an.

use crate::error::MigrationError;
use crate::history::{self, AUTH_HISTORY_TABLE, HISTORY_TABLE};
use crate::packages::{package_reports, PackageMigration};
use crate::runner::{migration_status_in, StatusReport};
use crate::snapshot::SchemaSnapshot;
use std::path::Path;

const AUDIT_TABLE: &str = "_ferrum_admin_audit";
const DRAFTS_TABLE: &str = "_ferrum_admin_drafts";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationScope {
    Auth,
    App,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub scope: MigrationScope,
    pub migration_id: String,
    pub level: Level,
    pub code: &'static str,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageStatus {
    pub name: String,
    pub status: StatusReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualReport {
    pub backend: String,
    pub auth: StatusReport,
    pub app: StatusReport,
    pub packages: Vec<PackageStatus>,
    pub diagnostics: Vec<Diagnostic>,
}

impl DualReport {
    pub fn pending_auth(&self) -> usize {
        pending_lines(&self.auth)
    }

    pub fn pending_app(&self) -> usize {
        pending_lines(&self.app)
    }

    pub fn pending_packages(&self) -> usize {
        self.packages
            .iter()
            .map(|package| pending_lines(&package.status))
            .sum()
    }

    pub fn has_error(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|item| item.level == Level::Error)
    }

    pub fn has_warning(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|item| item.level == Level::Warn)
    }
}

pub fn readonly_database_url(url: &str) -> Result<String, MigrationError> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        return Ok(url.to_owned());
    }
    if url.starts_with("sqlite::memory:") || url.contains("mode=memory") {
        return Ok(url.to_owned());
    }
    let Some(rest) = url.strip_prefix("sqlite://") else {
        return Err(MigrationError::new(
            "migration.database.unavailable: Database URL must start with sqlite:// or postgres://.".to_owned(),
        ));
    };
    let path = rest.split('?').next().unwrap_or(rest);
    if path.is_empty() || path == ":memory:" {
        return Ok(url.to_owned());
    }
    if !Path::new(path).is_file() {
        return Err(MigrationError::new(format!(
            "migration.database.missing: SQLite file {path} does not exist. No file will be created."
        )));
    }
    Ok(format!("sqlite://{path}?mode=ro"))
}

pub async fn inspect_migrations(
    db: &sea_orm::DatabaseConnection,
    auth_dir: &Path,
    app_dir: &Path,
    current: Option<&SchemaSnapshot>,
) -> Result<DualReport, MigrationError> {
    inspect_migrations_with_packages(db, auth_dir, app_dir, current, &[]).await
}

pub async fn inspect_migrations_with_packages(
    db: &sea_orm::DatabaseConnection,
    auth_dir: &Path,
    app_dir: &Path,
    current: Option<&SchemaSnapshot>,
    packages: &[PackageMigration],
) -> Result<DualReport, MigrationError> {
    diagnose(db, auth_dir, app_dir, current, packages)
        .await
        .map_err(as_unavailable)
}

async fn diagnose(
    db: &sea_orm::DatabaseConnection,
    auth_dir: &Path,
    app_dir: &Path,
    current: Option<&SchemaSnapshot>,
    packages: &[PackageMigration],
) -> Result<DualReport, MigrationError> {
    let auth = migration_status_in(db, auth_dir, AUTH_HISTORY_TABLE, None).await?;
    let app = migration_status_in(db, app_dir, HISTORY_TABLE, current).await?;
    let mut diagnostics = Vec::new();
    let package_status = package_reports(db, packages).await?;
    let packages = package_status
        .into_iter()
        .map(|(name, status)| PackageStatus { name, status })
        .collect::<Vec<_>>();
    diagnostics.extend(line_diagnostics(MigrationScope::Auth, &auth));
    diagnostics.extend(line_diagnostics(MigrationScope::App, &app));
    for package in &packages {
        diagnostics.extend(line_diagnostics(MigrationScope::App, &package.status));
    }
    if !history::history_exists_in(db, AUTH_HISTORY_TABLE).await?
        && auth.lines.iter().any(|line| line.state == "ausstehend")
    {
        diagnostics.push(note(
            MigrationScope::Auth,
            "",
            Level::Warn,
            "migration.history.missing",
            "Authentication migration history is missing. The table will not be created.",
        ));
    }
    if !history::history_exists_in(db, HISTORY_TABLE).await?
        && app.lines.iter().any(|line| line.state == "ausstehend")
    {
        diagnostics.push(note(
            MigrationScope::App,
            "",
            Level::Warn,
            "migration.history.missing",
            "App migration history is missing. The table will not be created.",
        ));
    }
    diagnose_auth_tables(db, &auth, &mut diagnostics).await?;
    if app.schema_note.contains("differs") || app.schema_note.contains("neither") {
        diagnostics.push(note(
            MigrationScope::App,
            "",
            Level::Warn,
            "migration.schema_conflict",
            &app.schema_note,
        ));
    }
    Ok(DualReport {
        backend: app.backend.clone(),
        auth,
        app,
        packages,
        diagnostics,
    })
}

pub fn render_diagnosis(locale: &str, report: &DualReport) -> String {
    let mut text = format!("Backend: {}\n\n", report.backend);
    text.push_str(&section(
        locale,
        "Auth-Migrationen",
        "Auth migrations",
        &report.auth,
    ));
    text.push('\n');
    text.push_str(&section(
        locale,
        "App-Migrationen",
        "App migrations",
        &report.app,
    ));
    for package in &report.packages {
        text.push('\n');
        text.push_str(&section(
            locale,
            &format!("App {}", package.name),
            &format!("App {}", package.name),
            &package.status,
        ));
    }
    text.push('\n');
    let errors: Vec<&Diagnostic> = report
        .diagnostics
        .iter()
        .filter(|item| item.level == Level::Error)
        .collect();
    if errors.is_empty() {
        text.push_str(&phrase(locale, "Konflikte: keine\n", "Conflicts: none\n"));
    } else {
        text.push_str(&phrase(locale, "Konflikte:\n", "Conflicts:\n"));
        for item in errors {
            text.push_str(&format!("  {} {}\n", item.code, item.description));
        }
    }
    let pending = report.pending_auth() + report.pending_app() + report.pending_packages();
    text.push_str(&format!(
        "{} {pending}\n",
        phrase(locale, "Ausstehende Migrationen:", "Pending migrations:")
    ));
    for item in report
        .diagnostics
        .iter()
        .filter(|item| item.code.ends_with(".pending") || item.level == Level::Warn)
    {
        if item.code == "migration.history.missing" || item.code.ends_with(".pending") {
            text.push_str(&format!("  {} {}\n", item.code, item.description));
        }
    }
    if !report.app.schema_note.is_empty() {
        text.push_str(&format!("Schema: {}\n", report.app.schema_note));
    }
    if pending > 0 {
        text.push_str(&phrase(
            locale,
            "\nNächster Schritt:\n  ferrioxid migrate up\n",
            "\nNext step:\n  ferrioxid migrate up\n",
        ));
    }
    text
}

pub fn dev_notice(locale: &str, report: &Result<DualReport, MigrationError>) -> String {
    match report {
        Ok(report)
            if report.pending_auth() == 0
                && report.pending_app() == 0
                && report.pending_packages() == 0
                && !report.has_error() =>
        {
            String::new()
        }
        Ok(report) if report.has_error() => phrase(
            locale,
            &format!(
                "[WARN] Die Migrationsdiagnose hat Konflikte gefunden.\n       Auth: {} ausstehend. App: {} ausstehend.\nBitte prüfen:\n  ferrioxid migrate status\n",
                report.pending_auth(),
                report.pending_app()
            ),
            &format!(
                "[WARN] Migration diagnosis found conflicts.\n       Auth: {} pending. App: {} pending.\nCheck:\n  ferrioxid migrate status\n",
                report.pending_auth(),
                report.pending_app()
            ),
        ),
        Ok(report) => phrase(
            locale,
            &format!(
                "[WARN] Die Datenbank ist nicht vollständig migriert.\n       Auth: {} Migrationen ausstehend.\n       App: {} Migrationen ausstehend.\n\nBitte ausführen:\n  ferrioxid migrate up\n",
                report.pending_auth(),
                report.pending_app()
            ),
            &format!(
                "[WARN] The database is not fully migrated.\n       Auth: {} migrations pending.\n       App: {} migrations pending.\n\nRun:\n  ferrioxid migrate up\n",
                report.pending_auth(),
                report.pending_app()
            ),
        ),
        Err(error) => phrase(
            locale,
            &format!(
                "[WARN] Der Migrationsstatus konnte nicht geprüft werden.\n       {error}\nEs wird nicht behauptet, dass die Datenbank aktuell ist.\n"
            ),
            &format!(
                "[WARN] The migration status could not be checked.\n       {error}\nThe database is not reported as up to date.\n"
            ),
        ),
    }
}

async fn diagnose_auth_tables(
    db: &sea_orm::DatabaseConnection,
    auth: &StatusReport,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), MigrationError> {
    let applied = |id: &str| {
        auth.lines
            .iter()
            .any(|line| line.id == id && line.state == "angewendet")
    };
    if applied("0002_admin_journal") {
        for table in [AUDIT_TABLE, DRAFTS_TABLE] {
            if !history::table_exists(db, table).await? {
                diagnostics.push(note(
                    MigrationScope::Auth,
                    "0002_admin_journal",
                    Level::Error,
                    "migration.auth.table_missing",
                    &format!("Expected table {table} is missing."),
                ));
            }
        }
    }
    if applied("0003_audit_operation")
        && history::table_exists(db, AUDIT_TABLE).await?
        && !history::column_exists(db, AUDIT_TABLE, "operation_id").await?
    {
        diagnostics.push(note(
            MigrationScope::Auth,
            "0003_audit_operation",
            Level::Error,
            "migration.auth.table_missing",
            "Column operation_id is missing from _ferrum_admin_audit.",
        ));
    }
    Ok(())
}

fn as_unavailable(error: MigrationError) -> MigrationError {
    if error.0.contains("migration.") {
        error
    } else {
        MigrationError::new(format!("migration.database.unavailable: {}", error.0))
    }
}

fn line_diagnostics(scope: MigrationScope, report: &StatusReport) -> Vec<Diagnostic> {
    let mut items = Vec::new();
    for line in &report.lines {
        if line.state == "ausstehend" {
            let code = match scope {
                MigrationScope::Auth => "migration.auth.pending",
                MigrationScope::App => "migration.app.pending",
            };
            items.push(note(
                scope,
                &line.id,
                Level::Warn,
                code,
                &format!("{id} is pending.", id = line.id),
            ));
        }
    }
    for conflict in &report.conflicts {
        items.push(note(
            scope,
            "",
            Level::Error,
            conflict_code(conflict),
            conflict,
        ));
    }
    items
}

fn conflict_code(text: &str) -> &'static str {
    let normalized = text.to_ascii_lowercase();
    if normalized.contains("checksum") {
        "migration.checksum_mismatch"
    } else if normalized.contains("no file")
        || normalized.contains("missing from the directory")
        || normalized.contains("no longer has a file")
    {
        "migration.unknown"
    } else {
        "migration.order"
    }
}

#[cfg(test)]
#[test]
fn translated_conflicts_preserve_machine_readable_codes() {
    assert_eq!(
        conflict_code("Stored checksum does not match file"),
        "migration.checksum_mismatch"
    );
    assert_eq!(
        conflict_code("Applied migration has no file"),
        "migration.unknown"
    );
}

fn pending_lines(report: &StatusReport) -> usize {
    report
        .lines
        .iter()
        .filter(|line| line.state == "ausstehend")
        .count()
}

fn note(
    scope: MigrationScope,
    migration_id: &str,
    level: Level,
    code: &'static str,
    description: &str,
) -> Diagnostic {
    Diagnostic {
        scope,
        migration_id: migration_id.to_owned(),
        level,
        code,
        description: description.to_owned(),
    }
}

fn section(locale: &str, de: &str, en: &str, report: &StatusReport) -> String {
    let mut text = format!("{}:\n", phrase(locale, de, en));
    if report.lines.is_empty() {
        text.push_str(&phrase(locale, "  keine\n", "  none\n"));
        return text;
    }
    for line in &report.lines {
        let visible_state = if locale == "de" {
            line.state.as_str()
        } else {
            match line.state.as_str() {
                "angewendet" => "applied",
                "ausstehend" => "pending",
                other => other,
            }
        };
        text.push_str(&format!(
            "  {}  {}  checksum {}\n",
            line.id, visible_state, line.checksum
        ));
    }
    text
}

fn phrase(locale: &str, de: &str, en: &str) -> String {
    if locale == "de" {
        de.to_owned()
    } else {
        en.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{ColumnSnapshot, LogicalType, TableSnapshot, FORMAT_VERSION};
    use crate::store::make_migration;
    use sea_orm::ConnectionTrait;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn products() -> SchemaSnapshot {
        SchemaSnapshot {
            format_version: FORMAT_VERSION,
            tables: vec![TableSnapshot {
                name: "products".to_owned(),
                columns: vec![ColumnSnapshot {
                    name: "id".to_owned(),
                    logical_type: LogicalType::I64,
                    primary_key: true,
                    nullable: false,
                    default: None,
                }],
                unique_constraints: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
            }],
        }
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ferrioxid-inspect-{label}-{nanos}"));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn missing_sqlite_file_is_not_created() {
        let path = scratch("missing").join("absent.sqlite");
        let url = format!(
            "sqlite://{}?mode=rwc",
            path.to_string_lossy().replace('\\', "/")
        );
        let error = readonly_database_url(&url).unwrap_err();
        assert!(error.0.contains("migration.database.missing"));
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn pending_auth_and_app_do_not_write() {
        let root = scratch("pending");
        let app = root.join("migrations");
        std::fs::create_dir_all(&app).unwrap();
        make_migration(&app, "initial", &products()).unwrap();
        let file = root.join("db.sqlite");
        let write_url = format!(
            "sqlite://{}?mode=rwc",
            file.to_string_lossy().replace('\\', "/")
        );
        let db = ferrioxid_db::connect(&write_url).await.unwrap();
        drop(db);
        let before = std::fs::read(&file).unwrap();
        let read_url = readonly_database_url(&write_url).unwrap();
        assert!(read_url.contains("mode=ro"));
        let db = ferrioxid_db::connect(&read_url).await.unwrap();
        let auth_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ferrioxid-auth/migrations");
        let report = inspect_migrations(&db, &auth_dir, &app, Some(&products()))
            .await
            .unwrap();
        drop(db);
        assert!(report.pending_auth() >= 2);
        assert_eq!(report.pending_app(), 1);
        assert!(report
            .diagnostics
            .iter()
            .any(|item| item.code == "migration.auth.pending"
                && item.migration_id == "0002_admin_journal"));
        assert!(report
            .diagnostics
            .iter()
            .any(|item| item.code == "migration.auth.pending"
                && item.migration_id == "0003_audit_operation"));
        assert!(!report.has_error());
        let text = render_diagnosis("de", &report);
        assert!(text.contains("Auth-Migrationen:"));
        assert!(text.contains("App-Migrationen:"));
        assert!(text.contains("0002_admin_journal  ausstehend"));
        assert!(text.contains("ferrioxid migrate up"));
        let again = {
            let db = ferrioxid_db::connect(&read_url).await.unwrap();
            let second = inspect_migrations(&db, &auth_dir, &app, Some(&products()))
                .await
                .unwrap();
            drop(db);
            second
        };
        assert_eq!(report, again);
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn checksum_conflict_is_an_error_and_stays_read_only() {
        let root = scratch("checksum");
        let app = root.join("migrations");
        std::fs::create_dir_all(&app).unwrap();
        make_migration(&app, "initial", &products()).unwrap();
        let file = root.join("db.sqlite");
        let write_url = format!(
            "sqlite://{}?mode=rwc",
            file.to_string_lossy().replace('\\', "/")
        );
        let db = ferrioxid_db::connect(&write_url).await.unwrap();
        crate::apply_directory(&db, &app, false).await.unwrap();
        drop(db);
        let db = ferrioxid_db::connect(&write_url).await.unwrap();
        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "UPDATE _ferrum_migrations SET checksum = '00' WHERE id LIKE '0001%'",
        ))
        .await
        .unwrap();
        drop(db);
        let before = std::fs::read(&file).unwrap();
        let read_url = readonly_database_url(&write_url).unwrap();
        let db = ferrioxid_db::connect(&read_url).await.unwrap();
        let auth_dir = root.join("empty-auth");
        std::fs::create_dir_all(&auth_dir).unwrap();
        let report = inspect_migrations(&db, &auth_dir, &app, Some(&products()))
            .await
            .unwrap();
        drop(db);
        assert!(report.has_error());
        assert!(report
            .diagnostics
            .iter()
            .any(|item| item.code == "migration.checksum_mismatch"));
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let db = ferrioxid_db::connect(&write_url).await.unwrap();
        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO _ferrum_migrations (id, name, checksum, applied_at_unix) VALUES ('9999_foreign', 'foreign', 'abc', 1)",
        ))
        .await
        .unwrap();
        drop(db);
        let before_unknown = std::fs::read(&file).unwrap();
        let db = ferrioxid_db::connect(&read_url).await.unwrap();
        let unknown = inspect_migrations(&db, &auth_dir, &app, None)
            .await
            .unwrap();
        drop(db);
        assert!(unknown
            .diagnostics
            .iter()
            .any(|item| item.code == "migration.unknown"));
        let unknown_text = render_diagnosis("de", &unknown);
        assert!(unknown_text.contains("not loaded"));
        assert!(!unknown_text.contains("entspricht dem letzten"));
        assert_eq!(std::fs::read(&file).unwrap(), before_unknown);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unreachable_notice_does_not_claim_success() {
        let notice = dev_notice(
            "de",
            &Err(MigrationError::new(
                "migration.database.missing: fehlt".to_owned(),
            )),
        );
        assert!(notice.contains("[WARN]"));
        assert!(notice.contains("nicht geprüft"));
        assert!(!notice.contains("vollständig migriert"));
    }
}
