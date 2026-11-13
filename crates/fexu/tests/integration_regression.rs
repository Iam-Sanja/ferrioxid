//! Cross-pull-request regressions for inspect, plan, embedded packages and JSON.
//!
//! These tests do not open the populated `fexu` or
//! `fexu_migrations_down_test` catalogs. The ignored PostgreSQL case
//! creates one `fexu_test_*` database and drops it.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fexu::db::DisposablePostgres;
use fexu::migrations::{plan_database, table_exists, SchemaSnapshot};

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("fexu-integration-{label}-{nanos}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

async fn inspect_and_plan(url: &str) -> Result<(), String> {
    let db = fexu::db::connect(url)
        .await
        .map_err(|error| error.to_string())?;
    let auth_files = fexu::auth::embedded_migrations().map_err(|error| error.to_string())?;
    let jobs_files = fexu::jobs::embedded_migrations()
        .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
    let app = scratch("app");
    let pending = fexu::migrations::inspect_framework(
        &db,
        &auth_files,
        &jobs_files,
        fexu::jobs::JOB_HISTORY,
        &app,
        None,
        &[],
    )
    .await
    .map_err(|error| error.0)?;
    if pending.pending_auth() < 6 {
        return Err(format!(
            "expected pending auth, got {:?}",
            pending.auth.lines
        ));
    }
    if pending.pending_jobs() != 2 {
        return Err(format!(
            "expected two pending jobs, got {:?}",
            pending.jobs.lines
        ));
    }
    if table_exists(&db, "_fexu_auth_migrations")
        .await
        .map_err(|error| error.0)?
        || table_exists(&db, fexu::jobs::JOB_HISTORY)
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
    fexu::auth::apply_migrations(&db)
        .await
        .map_err(|error| error.to_string())?;
    fexu::jobs::apply_migrations(&db)
        .await
        .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
    let applied = fexu::migrations::inspect_framework(
        &db,
        &auth_files,
        &jobs_files,
        fexu::jobs::JOB_HISTORY,
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
