//! CLI run against a temporary SQLite file. The development database remains untouched.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ferrioxid-cli-{nanos}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn sqlite_url(path: &Path) -> String {
    format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    )
}

fn run(args: &[&str]) -> std::process::Output {
    let output = Command::new(env!("CARGO_BIN_EXE_ferrioxid"))
        .args(args)
        .output()
        .expect("ferrioxid CLI");
    if !output.status.success() {
        panic!(
            "CLI failed: {}\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    output
}

#[test]
fn migrate_make_up_and_status_use_a_temporary_sqlite_database() {
    let directory = temp_dir();
    let schema = directory.join("schema.json");
    let migrations = directory.join("migrations");
    let database = directory.join("cli.sqlite3");
    std::fs::write(
        &schema,
        r#"{"format_version":1,"tables":[{"name":"products","columns":[{"name":"id","type":"uuid","primary_key":true,"nullable":false},{"name":"name","type":"string","primary_key":false,"nullable":false},{"name":"active","type":"bool","primary_key":false,"nullable":false,"default":true}]}]}"#,
    )
    .unwrap();
    let schema_arg = schema.to_string_lossy().to_string();
    let migrations_arg = migrations.to_string_lossy().to_string();
    let url = sqlite_url(&database);

    let made = run(&[
        "migrate",
        "make",
        "initial",
        "--schema-file",
        &schema_arg,
        "--migrations-dir",
        &migrations_arg,
    ]);
    let stdout = String::from_utf8_lossy(&made.stdout);
    assert!(stdout.contains("0001_initial"));

    let applied = run(&[
        "migrate",
        "up",
        "--database-url",
        &url,
        "--migrations-dir",
        &migrations_arg,
    ]);
    assert!(String::from_utf8_lossy(&applied.stdout).contains("Applied: 0001_initial"));

    let status = run(&[
        "migrate",
        "status",
        "--database-url",
        &url,
        "--migrations-dir",
        &migrations_arg,
        "--schema-file",
        &schema_arg,
    ]);
    let status_text = String::from_utf8_lossy(&status.stdout);
    assert!(status_text.contains("Backend: sqlite"));
    assert!(status_text.contains("0001_auth_schema  applied"));
    assert!(status_text.contains("0003_audit_operation  applied"));
    assert!(status_text.contains("0001_initial  applied"));
    assert!(!status_text.contains("pending"));
    assert!(status_text.contains("Conflicts: none"));
    assert!(status_text.contains("matches the last applied migration target"));

    let second = run(&[
        "migrate",
        "up",
        "--database-url",
        &url,
        "--migrations-dir",
        &migrations_arg,
    ]);
    assert!(String::from_utf8_lossy(&second.stdout).contains("No pending migrations."));

    let unchanged = run(&[
        "migrate",
        "make",
        "again",
        "--schema-file",
        &schema_arg,
        "--migrations-dir",
        &migrations_arg,
    ]);
    assert!(String::from_utf8_lossy(&unchanged.stdout).contains("No schema changes"));
    let _ = std::fs::remove_dir_all(&directory);
}
