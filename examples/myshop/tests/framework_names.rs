//! Fresh framework schemas. PostgreSQL requires an empty disposable database.

use ferrioxid::sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use std::collections::BTreeSet;

async fn tables(db: &DatabaseConnection) -> BTreeSet<String> {
    let sql = match db.get_database_backend() {
        DatabaseBackend::Sqlite => "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        DatabaseBackend::Postgres => "SELECT table_name AS name FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE'",
        _ => unreachable!(),
    };
    db.query_all(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.try_get("", "name").unwrap())
        .collect()
}

async fn initialize(db: &DatabaseConnection) -> BTreeSet<String> {
    assert!(
        tables(db).await.is_empty(),
        "Use an empty disposable database"
    );
    assert_eq!(
        ferrioxid::auth::apply_migrations(db).await.unwrap().len(),
        4
    );
    assert_eq!(
        ferrioxid::jobs::apply_migrations(db).await.unwrap().len(),
        1
    );
    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    assert_eq!(
        ferrioxid::migrations::apply_directory(db, &migrations, false)
            .await
            .unwrap()
            .applied
            .len(),
        6
    );
    let names = tables(db).await;
    for name in [
        "_ferrioxid_users",
        "_ferrioxid_sessions",
        "_ferrioxid_roles",
        "_ferrioxid_permissions",
        "_ferrioxid_user_roles",
        "_ferrioxid_role_permissions",
        "_ferrioxid_admin_drafts",
        "_ferrioxid_admin_audit",
        "_ferrioxid_password_resets",
        "_ferrioxid_password_reset_exchanges",
        "_ferrioxid_mail_outbox",
        "_ferrioxid_jobs",
        "_ferrioxid_auth_migrations",
        "_ferrioxid_job_migrations",
        "_ferrioxid_migrations",
        "products",
        "categories",
        "internal_notes",
        "events",
        "orders",
        "order_lines",
    ] {
        assert!(names.contains(name), "missing table: {name}");
    }
    assert_eq!(names.len(), 21, "unexpected table in disposable database");
    assert!(!names.iter().any(|name| name.contains("ferrum")));
    assert!(ferrioxid::auth::apply_migrations(db)
        .await
        .unwrap()
        .is_empty());
    assert!(ferrioxid::jobs::apply_migrations(db)
        .await
        .unwrap()
        .is_empty());
    assert!(
        ferrioxid::migrations::apply_directory(db, &migrations, false)
            .await
            .unwrap()
            .applied
            .is_empty()
    );
    names
}

#[tokio::test]
async fn fresh_sqlite_initializes_auth_jobs_and_application() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    initialize(&db).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL pointing to an empty disposable PostgreSQL database"]
async fn fresh_postgres_initializes_auth_jobs_and_application() {
    let url = std::env::var("DATABASE_URL").expect("disposable PostgreSQL DATABASE_URL");
    let db = ferrioxid::db::connect(&url).await.unwrap();
    let created = initialize(&db).await;
    // Drop only the verified tables created in the initially empty test database.
    for name in created {
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        db.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("DROP TABLE IF EXISTS \"{name}\" CASCADE"),
        ))
        .await
        .unwrap();
    }
    assert!(tables(&db).await.is_empty());
}
