//! Embedded framework migrations and the read-only readiness split.

use fexu::auth::{AuthConfig, AuthService};
use fexu::db::DisposablePostgres;

#[tokio::test]
async fn readiness_reports_auth_jobs_and_an_absent_app_without_migrating() {
    let db = fexu::db::connect("sqlite::memory:").await.unwrap();
    let auth_files = fexu::auth::embedded_migrations().unwrap();
    let jobs_files = fexu::jobs::embedded_migrations().unwrap();
    let absent = std::env::temp_dir().join(format!("fexu-absent-app-{}", std::process::id()));
    let report = fexu::migrations::inspect_framework(
        &db,
        &auth_files,
        &jobs_files,
        fexu::jobs::JOB_HISTORY,
        &absent,
        None,
        &[],
    )
    .await
    .unwrap();
    assert!(report.pending_auth() >= 6, "{:?}", report.auth.lines);
    assert_eq!(report.pending_jobs(), 2);
    assert_eq!(report.pending_app(), 0);
    assert!(
        report.app.schema_note.contains("absent"),
        "{}",
        report.app.schema_note
    );
    assert!(
        !fexu::migrations::table_exists(&db, "_fexu_auth_migrations")
            .await
            .unwrap()
    );
    assert!(
        !fexu::migrations::table_exists(&db, fexu::jobs::JOB_HISTORY)
            .await
            .unwrap()
    );
    assert!(!fexu::migrations::table_exists(&db, "_fexu_users")
        .await
        .unwrap());
}

#[tokio::test]
#[ignore = "requires DATABASE_URL pointing at an approved PostgreSQL admin database"]
async fn postgres_embedded_auth_and_jobs() {
    let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let isolated = DisposablePostgres::create(&admin)
        .await
        .expect("isolated database");
    let result = exercise(isolated.url()).await;
    let dropped = isolated.drop_database().await;
    result.unwrap();
    dropped.expect("drop isolated database");
}

async fn exercise(url: &str) -> Result<(), String> {
    let db = fexu::db::connect(url)
        .await
        .map_err(|error| error.to_string())?;
    let auth_applied = fexu::auth::apply_migrations(&db)
        .await
        .map_err(|error| error.to_string())?;
    if auth_applied.len() != 6 {
        return Err(format!(
            "expected 6 auth migrations, applied {auth_applied:?}"
        ));
    }
    let jobs_applied = fexu::jobs::apply_migrations(&db)
        .await
        .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
    if jobs_applied.len() != 2 {
        return Err(format!(
            "expected 2 job migrations, applied {jobs_applied:?}"
        ));
    }
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let password = ephemeral_password();
    auth.create_user("postgres-embedded@example.com", &password, true, false)
        .await
        .map_err(|error| error.to_string())?;
    auth.login("postgres-embedded@example.com", &password, "127.0.0.1")
        .await
        .map_err(|error| error.to_string())?;
    let again = fexu::auth::apply_migrations(&db)
        .await
        .map_err(|error| error.to_string())?;
    if !again.is_empty() {
        return Err(format!("second auth apply changed history: {again:?}"));
    }
    let status = fexu::auth::migration_status(&db)
        .await
        .map_err(|error| error.to_string())?;
    if status.lines.iter().any(|line| line.state != "angewendet") || !status.conflicts.is_empty() {
        return Err(format!("auth status is not applied: {status:?}"));
    }
    let queue = fexu::jobs::Queue::new(
        db,
        std::sync::Arc::new(fexu::jobs::SystemClock),
        fexu::jobs::JobPolicy::default(),
    );
    let created = queue
        .enqueue_json(
            "embedded_probe",
            1,
            &serde_json::json!({"kind": "probe"}),
            fexu::jobs::When::now(),
        )
        .await
        .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
    let fexu::jobs::EnqueueOutcome::Created(id) = created else {
        return Err(format!("enqueue did not create a job: {created:?}"));
    };
    let loaded = queue
        .get(id)
        .await
        .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
    if loaded.handler != "embedded_probe" {
        return Err(format!("loaded job has handler {}", loaded.handler));
    }
    Ok(())
}

fn ephemeral_password() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
