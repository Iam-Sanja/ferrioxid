//! Cross-pull-request regressions for inspect, plan, embedded packages and JSON.
//!
//! These tests do not open the populated `ferrioxid` or
//! `ferrioxid_migrations_down_test` catalogs. The ignored PostgreSQL case
//! creates one `ferrioxid_test_*` database and drops it.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ferrioxid::db::DisposablePostgres;
use ferrioxid::migrations::{plan_database, table_exists, SchemaSnapshot};

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ferrioxid-integration-{label}-{nanos}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

async fn inspect_and_plan(url: &str) -> Result<(), String> {
    let db = ferrioxid::db::connect(url)
        .await
        .map_err(|error| error.to_string())?;
    let auth_files = ferrioxid::auth::embedded_migrations().map_err(|error| error.to_string())?;
    let jobs_files = ferrioxid::jobs::embedded_migrations()
        .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
    let app = scratch("app");
    let pending = ferrioxid::migrations::inspect_framework(
        &db,
        &auth_files,
        &jobs_files,
        ferrioxid::jobs::JOB_HISTORY,
        &app,
        None,
        &[],
    )
    .await
    .map_err(|error| error.0)?;
    if pending.pending_auth() < 5 {
        return Err(format!(
            "expected pending auth, got {:?}",
            pending.auth.lines
        ));
    }
    if pending.pending_jobs() != 1 {
        return Err(format!(
            "expected one pending job, got {:?}",
            pending.jobs.lines
        ));
    }
    if table_exists(&db, "_ferrioxid_auth_migrations")
        .await
        .map_err(|error| error.0)?
        || table_exists(&db, ferrioxid::jobs::JOB_HISTORY)
            .await
            .map_err(|error| error.0)?
    {
        return Err("inspect_framework created history tables".into());
    }
    let unread = plan_database(&db, &app, Some(&SchemaSnapshot::empty()))
        .await
        .map_err(|error| error.0)?;
    if unread.has_error() || unread.has_drift() {
        return Err(format!("empty plan was not quiet: {unread:?}"));
    }
    ferrioxid::auth::apply_migrations(&db)
        .await
        .map_err(|error| error.to_string())?;
    ferrioxid::jobs::apply_migrations(&db)
        .await
        .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
    let applied = ferrioxid::migrations::inspect_framework(
        &db,
        &auth_files,
        &jobs_files,
        ferrioxid::jobs::JOB_HISTORY,
        &app,
        None,
        &[],
    )
    .await
    .map_err(|error| error.0)?;
    if applied.pending_auth() != 0 || applied.pending_jobs() != 0 {
        return Err(format!(
            "embedded packages stayed pending: auth={:?} jobs={:?}",
            applied.auth.lines, applied.jobs.lines
        ));
    }
    let quiet = plan_database(&db, &app, Some(&SchemaSnapshot::empty()))
        .await
        .map_err(|error| error.0)?;
    if quiet.has_error() || quiet.has_drift() {
        return Err(format!(
            "plan treated framework tables as application drift: {quiet:?}"
        ));
    }
    let _ = std::fs::remove_dir_all(&app);
    Ok(())
}

#[tokio::test]
async fn inspect_framework_and_plan_stay_read_only_until_apply() {
    inspect_and_plan("sqlite::memory:").await.unwrap();
}

#[tokio::test]
#[ignore = "requires DATABASE_URL pointing at an approved PostgreSQL admin database"]
async fn postgres_inspect_framework_and_plan_stay_read_only() {
    let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let isolated = DisposablePostgres::create(&admin)
        .await
        .expect("isolated database");
    let result = inspect_and_plan(isolated.url()).await;
    let dropped = isolated.drop_database().await;
    result.unwrap();
    dropped.expect("drop isolated database");
}
