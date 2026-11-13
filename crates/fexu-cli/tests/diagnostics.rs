//! Diagnostics and upgrades use disposable projects. Protected databases are not touched.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("fexu-v021-{nanos}"));
    std::fs::create_dir_all(path.join("migrations")).unwrap();
    std::fs::create_dir_all(path.join("media")).unwrap();
    path
}

fn fexu(directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fexu"));
    command.current_dir(directory).env("FEXU_LOCALE", "de").env(
        "DATABASE_URL",
        format!(
            "sqlite://{}/missing.sqlite?mode=rwc",
            directory.to_string_lossy().replace('\\', "/")
        ),
    );
    command
}

fn output(command: &mut Command) -> std::process::Output {
    command.output().expect("fexu")
}

fn text(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn write_project(directory: &Path) {
    std::fs::write(
        directory.join("fexu.toml"),
        "migrations_dir = \"migrations\"\n\n[project]\ndefault_locale = \"de\"\n",
    )
    .unwrap();
    std::fs::write(
        directory.join("Cargo.toml"),
        "[package]\nname = \"legacy\"\nversion = \"0.1.0\"\n\n[dependencies]\nfexu = { path = \"D:/04_Projekte_Arbeit/Projekte/fexu/crates/fexu\", features = [\"admin-ui\"] }\n",
    )
    .unwrap();
    std::fs::write(directory.join("secret.txt"), "do not touch").unwrap();
    std::fs::create_dir_all(directory.join("src")).unwrap();
    std::fs::write(directory.join("src/main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(directory.join("src/lib.rs"), "\n").unwrap();
}

fn empty_sqlite(path: &Path) {
    // SQLite accepts an existing zero-byte file as an empty test database.
    std::fs::File::create(path).expect("create empty SQLite test file");
}

#[test]
fn cli_help_is_english_without_a_locale_override() {
    let directory = root();
    write_project(&directory);
    let result = fexu(&directory)
        .env_remove("FEXU_LOCALE")
        .args(["new", "--help"])
        .output()
        .expect("CLI help");
    assert!(result.status.success());
    let help = String::from_utf8_lossy(&result.stdout);
    assert!(help.contains("Creates a project or an app"), "{help}");
    assert!(help.contains("project") && help.contains("app"), "{help}");
    let result = fexu(&directory)
        .args(["new", "project", "--help"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let help = String::from_utf8_lossy(&result.stdout);
    assert!(
        help.contains("--apps-dir") && help.contains("--template"),
        "{help}"
    );
    assert!(!help.contains("Erzeugt"), "{help}");
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn status_is_read_only_and_shows_auth_and_app() {
    let directory = root();
    let database = directory.join("app.sqlite");
    std::fs::write(
        directory.join("schema.json"),
        r#"{"format_version":1,"tables":[{"name":"products","columns":[{"name":"id","type":"i64","primary_key":true,"nullable":false}]}]}"#,
    )
    .unwrap();
    let made = output(
        fexu(&directory)
            .args(["migrate", "make", "initial", "--schema-file"])
            .arg(directory.join("schema.json"))
            .arg("--migrations-dir")
            .arg(directory.join("migrations")),
    );
    assert!(made.status.success(), "{}", text(&made));
    empty_sqlite(&database);
    let before = std::fs::read(&database).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        database.to_string_lossy().replace('\\', "/")
    );
    let status = output(
        fexu(&directory)
            .args([
                "migrate",
                "status",
                "--database-url",
                &url,
                "--migrations-dir",
            ])
            .arg(directory.join("migrations")),
    );
    let body = text(&status);
    assert!(status.status.success(), "{body}");
    assert!(body.contains("Auth-Migrationen:"), "{body}");
    assert!(body.contains("App-Migrationen:"), "{body}");
    assert!(body.contains("0001_initial  ausstehend"), "{body}");
    assert!(body.contains("0002_admin_journal  ausstehend"), "{body}");
    assert!(body.contains("0003_audit_operation  ausstehend"), "{body}");
    assert!(body.contains("migration.auth.pending"), "{body}");
    assert!(body.contains("migration.app.pending"), "{body}");
    assert!(body.contains("migration.history.missing"), "{body}");
    assert!(body.contains("was not loaded"), "{body}");
    assert!(!body.contains("entspricht dem letzten"), "{body}");
    assert!(body.contains("fexu migrate up"), "{body}");
    assert_eq!(std::fs::read(&database).unwrap(), before);
    assert!(!directory.join("missing.sqlite").exists());
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn check_strict_doctor_and_dev_warn_without_writing() {
    let directory = root();
    write_project(&directory);
    let database = directory.join("app.sqlite");
    empty_sqlite(&database);
    let before = std::fs::read(&database).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        database.to_string_lossy().replace('\\', "/")
    );
    let loose = output(
        fexu(&directory)
            .env("DATABASE_URL", &url)
            .env("FEXU_CHECK_SKIP_CARGO", "1")
            .args(["check"]),
    );
    let loose_text = text(&loose);
    assert!(loose.status.success(), "{loose_text}");
    assert!(loose_text.contains("[WARN]"), "{loose_text}");
    assert!(
        loose_text.contains("migration.auth.pending"),
        "{loose_text}"
    );
    assert!(
        loose_text.contains("project.dependency.admin_ui"),
        "{loose_text}"
    );
    assert!(!loose_text.contains("muss entfernt"), "{loose_text}");

    let strict = output(
        fexu(&directory)
            .env("DATABASE_URL", &url)
            .env("FEXU_CHECK_SKIP_CARGO", "1")
            .args(["check", "--strict"]),
    );
    assert_eq!(strict.status.code(), Some(2), "{}", text(&strict));

    let doctor = output(fexu(&directory).env("DATABASE_URL", &url).arg("doctor"));
    let doctor_text = text(&doctor);
    assert!(doctor.status.success(), "{doctor_text}");
    assert!(doctor_text.contains("Fexu Doctor"), "{doctor_text}");
    assert!(doctor_text.contains("fexu migrate up"), "{doctor_text}");

    std::fs::write(
        directory.join("Cargo.toml"),
        "[dependencies]\nfexu = { path = \"D:/04_Projekte_Arbeit/Projekte/fexu/crates/fexu\" }\n",
    )
    .unwrap();
    let dev = output(fexu(&directory).env("DATABASE_URL", &url).arg("dev"));
    let dev_text = text(&dev);
    assert!(!dev.status.success(), "{dev_text}");
    assert!(dev_text.contains("[WARN]"), "{dev_text}");
    assert!(dev_text.contains("fexu migrate up"), "{dev_text}");
    assert_eq!(std::fs::read(&database).unwrap(), before);
    assert_eq!(
        std::fs::read_to_string(directory.join("secret.txt")).unwrap(),
        "do not touch"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn upgrade_check_dry_run_and_yes_keep_unknown_files() {
    let directory = root();
    write_project(&directory);
    let secret = std::fs::read(directory.join("secret.txt")).unwrap();
    let checked = output(fexu(&directory).args(["upgrade", "--check"]));
    let checked_text = text(&checked);
    assert!(checked.status.success(), "{checked_text}");
    assert!(
        checked_text.contains("project.template.legacy"),
        "{checked_text}"
    );
    assert!(
        checked_text.contains("project.dependency.path"),
        "{checked_text}"
    );
    assert!(!directory.join(".fexu").exists(), "{checked_text}");

    let dry = output(fexu(&directory).args(["upgrade", "--dry-run"]));
    let dry_text = text(&dry);
    assert!(dry.status.success(), "{dry_text}");
    assert!(
        dry_text.contains("upgrade.plan.add .fexu/project.json"),
        "{dry_text}"
    );
    assert!(dry_text.contains("Geplante Dateiänderungen"), "{dry_text}");
    assert!(!directory.join(".fexu").exists());

    let applied = output(fexu(&directory).args(["upgrade", "--yes"]));
    assert!(applied.status.success(), "{}", text(&applied));
    let manifest = std::fs::read(directory.join(".fexu/project.json")).unwrap();
    assert!(std::str::from_utf8(&manifest).unwrap().contains("legacy"));
    assert_eq!(std::fs::read(directory.join("secret.txt")).unwrap(), secret);

    let again = output(fexu(&directory).args(["upgrade", "--yes"]));
    assert!(again.status.success(), "{}", text(&again));
    assert_eq!(
        std::fs::read(directory.join(".fexu/project.json")).unwrap(),
        manifest
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn upgrade_does_not_replace_a_modified_file() {
    let directory = root();
    write_project(&directory);
    std::fs::create_dir_all(directory.join(".fexu")).unwrap();
    std::fs::write(directory.join("README.md"), "eigene fassung").unwrap();
    std::fs::write(
        directory.join(".fexu/project.json"),
        "{\n  \"format\": 1,\n  \"template_version\": \"0.21\",\n  \"origin\": \"generated\",\n  \"template\": \"api\",\n  \"files\": [{ \"path\": \"README.md\", \"sha256\": \"deadbeef\" }]\n}\n",
    )
    .unwrap();
    let denied = output(fexu(&directory).args(["upgrade", "--yes"]));
    let denied_text = text(&denied);
    assert_eq!(denied.status.code(), Some(1), "{denied_text}");
    assert!(
        denied_text.contains("project.file.modified"),
        "{denied_text}"
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("README.md")).unwrap(),
        "eigene fassung"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn invalid_database_is_not_reported_as_current() {
    let directory = root();
    write_project(&directory);
    let database = directory.join("broken.sqlite");
    let bytes = b"keine sqlite datenbank";
    std::fs::write(&database, bytes).unwrap();
    let status = output(
        fexu(&directory)
            .args(["migrate", "status", "--database-url"])
            .arg(format!(
                "sqlite://{}?mode=ro",
                database.to_string_lossy().replace('\\', "/")
            ))
            .arg("--migrations-dir")
            .arg(directory.join("migrations")),
    );
    let body = text(&status);
    assert_eq!(status.status.code(), Some(1), "{body}");
    assert!(body.contains("migration.database.unavailable"), "{body}");
    assert!(!body.contains("Konflikte: keine"), "{body}");
    assert!(!body.contains("fully migrated"), "{body}");
    assert_eq!(std::fs::read(&database).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn missing_database_file_is_not_created() {
    let directory = root();
    write_project(&directory);
    let missing = directory.join("missing.sqlite");
    let status = output(
        fexu(&directory)
            .args(["migrate", "status", "--database-url"])
            .arg(format!(
                "sqlite://{}?mode=rwc",
                missing.to_string_lossy().replace('\\', "/")
            ))
            .arg("--migrations-dir")
            .arg(directory.join("migrations")),
    );
    let body = text(&status);
    assert_eq!(status.status.code(), Some(1), "{body}");
    assert!(body.contains("migration.database.missing"), "{body}");
    assert!(!missing.exists());
    let _ = std::fs::remove_dir_all(&directory);
}
