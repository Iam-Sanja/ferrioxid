//! Disposable PostgreSQL dump and restore.
//!
//! `cargo test -p fexu-db --test backup_restore -- --ignored`
//! requires `DATABASE_URL`, `pg_dump` and `pg_restore`. The test creates and
//! drops only `fexu_test_<32 hex>` databases. It does not touch the
//! administrator database.

use std::process::Command;

use fexu_db::DisposablePostgres;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[tokio::test]
#[ignore = "needs DATABASE_URL, pg_dump and pg_restore; disposable databases only"]
async fn postgres_dump_and_restore_roundtrip() {
    let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let source = DisposablePostgres::create(&admin)
        .await
        .expect("source database");
    let target = DisposablePostgres::create(&admin)
        .await
        .expect("target database");
    let dump = std::env::temp_dir().join(format!("{}.dump", source.name()));
    let outcome = dump_roundtrip(source.url(), target.url(), dump.as_path()).await;
    let _ = std::fs::remove_file(&dump);
    let source_drop = source.drop_database().await;
    let target_drop = target.drop_database().await;
    outcome.unwrap();
    source_drop.unwrap();
    target_drop.unwrap();
}

async fn dump_roundtrip(
    source_url: &str,
    target_url: &str,
    dump: &std::path::Path,
) -> Result<(), String> {
    assert!(
        source_url.contains("fexu_test_"),
        "refusing to dump a database that this helper did not create"
    );
    assert!(target_url.contains("fexu_test_"));
    let source = fexu_db::connect(source_url)
        .await
        .map_err(|error| error.to_string())?;
    source
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "CREATE TABLE backup_probe (id INTEGER PRIMARY KEY, label TEXT NOT NULL)",
        ))
        .await
        .map_err(|error| error.to_string())?;
    source
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "INSERT INTO backup_probe (id, label) VALUES (1, 'kept')",
        ))
        .await
        .map_err(|error| error.to_string())?;
    source.close().await.map_err(|error| error.to_string())?;

    run(
        "pg_dump",
        &[
            "--format=custom",
            "--file",
            &dump.display().to_string(),
            "--dbname",
            source_url,
        ],
    )?;
    run(
        "pg_restore",
        &[
            "--no-owner",
            "--dbname",
            target_url,
            &dump.display().to_string(),
        ],
    )?;

    let target = fexu_db::connect(target_url)
        .await
        .map_err(|error| error.to_string())?;
    let row = target
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT label FROM backup_probe WHERE id = 1",
        ))
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "restored row is missing".to_owned())?;
    let label: String = row
        .try_get("", "label")
        .map_err(|error| error.to_string())?;
    if label != "kept" {
        return Err(format!("restored label was {label}"));
    }
    target.close().await.map_err(|error| error.to_string())?;
    Ok(())
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("{program} is not available: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let redacted = stderr.replace(args.last().unwrap_or(&""), "[database]");
        Err(format!("{program} failed: {redacted}"))
    }
}
