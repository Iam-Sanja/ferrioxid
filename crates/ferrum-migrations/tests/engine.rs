//! SQLite-Migrationsläufe ohne Docker und ohne die Entwicklungsdatenbank.

use ferrum_db::connect;
use ferrum_migrations::{
    apply_directory, make_migration, migration_status, plan_change, save_migration, seal,
    ColumnSnapshot, LogicalType, MakeOutcome, SchemaSnapshot, TableSnapshot, FORMAT_VERSION,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ferrum-migrate-{nanos}-{tick}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn sqlite_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    format!("sqlite://{text}?mode=rwc")
}

fn column(
    name: &str,
    logical_type: LogicalType,
    primary_key: bool,
    nullable: bool,
    default: Option<bool>,
) -> ColumnSnapshot {
    ColumnSnapshot {
        name: name.to_owned(),
        logical_type,
        primary_key,
        nullable,
        default,
    }
}

fn products(extra: Vec<ColumnSnapshot>) -> SchemaSnapshot {
    let mut columns = vec![
        column("id", LogicalType::Uuid, true, false, None),
        column("name", LogicalType::String, false, false, None),
        column("active", LogicalType::Bool, false, false, Some(true)),
    ];
    columns.extend(extra);
    SchemaSnapshot {
        format_version: FORMAT_VERSION,
        tables: vec![TableSnapshot {
            name: "products".to_owned(),
            columns,
        }],
    }
}

async fn open_db(path: &Path) -> DatabaseConnection {
    connect(&sqlite_url(path)).await.unwrap()
}

async fn count_history(db: &DatabaseConnection) -> i64 {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS total FROM \"_ferrum_migrations\"",
        ))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<i64>("", "total")
        .or_else(|_| row.try_get::<i32>("", "total").map(i64::from))
        .unwrap()
}

#[tokio::test]
async fn sqlite_chain_preserves_rows_and_detects_tampering() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    let written = make_migration(&migrations, "initial", &initial).unwrap();
    assert!(matches!(written, MakeOutcome::Written { ref id, .. } if id == "0001_initial"));

    let db = open_db(&directory.join("app.sqlite3")).await;
    let report = apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(report.applied, vec!["0001_initial".to_owned()]);
    db.execute(Statement::from_string(
        db.get_database_backend(),
        "INSERT INTO \"products\" (\"id\", \"name\", \"active\") VALUES ('11111111-1111-4111-8111-111111111111', 'Laptop', 1)",
    ))
    .await
    .unwrap();
    assert!(apply_directory(&db, &migrations, false)
        .await
        .unwrap()
        .applied
        .is_empty());

    let expanded = products(vec![column("note", LogicalType::String, false, true, None)]);
    let second = make_migration(&migrations, "add_note", &expanded).unwrap();
    assert!(matches!(second, MakeOutcome::Written { ref id, .. } if id == "0002_add_note"));
    assert_eq!(
        apply_directory(&db, &migrations, false)
            .await
            .unwrap()
            .applied,
        vec!["0002_add_note".to_owned()]
    );

    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            "SELECT \"name\", \"note\" FROM \"products\"",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<String>("", "name").unwrap(), "Laptop");
    assert_eq!(row.try_get::<Option<String>>("", "note").unwrap(), None);
    assert!(apply_directory(&db, &migrations, false)
        .await
        .unwrap()
        .applied
        .is_empty());
    assert_eq!(count_history(&db).await, 2);

    let status = migration_status(&db, &migrations, Some(&expanded))
        .await
        .unwrap();
    let rendered = status.render();
    assert!(rendered.contains("Backend: sqlite"));
    assert!(rendered.contains("0001_initial  angewendet"));
    assert!(rendered.contains("0002_add_note  angewendet"));
    assert!(rendered.contains("Konflikte: keine"));
    assert!(rendered.contains("letzten angewendeten Migrationsziel"));

    let tampered = migrations.join("0002_add_note.json");
    let text = std::fs::read_to_string(&tampered)
        .unwrap()
        .replace("\"note\"", "\"notes\"");
    std::fs::write(&tampered, text).unwrap();
    let error = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(error.0.contains("Checksum") || error.0.contains("passen nicht"));
    assert_eq!(count_history(&db).await, 2);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn destructive_drop_requires_explicit_approval() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();

    let plan = plan_change(&initial, &SchemaSnapshot::empty()).unwrap();
    let file = seal(
        "0002_drop_products".to_owned(),
        "drop_products".to_owned(),
        initial,
        SchemaSnapshot::empty(),
        &plan,
    )
    .unwrap();
    save_migration(&migrations, &file).unwrap();

    let blocked = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(blocked.0.contains("allow-destructive"));
    let present = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
            [Value::from("products")],
        ))
        .await
        .unwrap();
    assert!(present.is_some());

    apply_directory(&db, &migrations, true).await.unwrap();
    let gone = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
            [Value::from("products")],
        ))
        .await
        .unwrap();
    assert!(gone.is_none());
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn unsupported_alter_does_not_apply_itself() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let mut changed = initial.clone();
    changed.tables[0].columns[1].logical_type = LogicalType::I64;
    let plan = plan_change(&initial, &changed).unwrap();
    assert!(plan.refusal.is_some());
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    save_migration(
        &migrations,
        &seal(
            "0002_change_name".to_owned(),
            "change_name".to_owned(),
            initial,
            changed,
            &plan,
        )
        .unwrap(),
    )
    .unwrap();
    let error = apply_directory(&db, &migrations, true).await.unwrap_err();
    assert!(error.0.contains("abgebrochen"));
    assert_eq!(count_history(&db).await, 1);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn existing_table_is_not_marked_migrated() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    make_migration(&migrations, "initial", &products(vec![])).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    db.execute(Statement::from_string(
        db.get_database_backend(),
        "CREATE TABLE \"products\" (\"id\" text PRIMARY KEY NOT NULL, \"name\" text NOT NULL, \"active\" integer NOT NULL)",
    ))
    .await
    .unwrap();
    let error = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(error.0.contains("nicht als migriert"));
    assert_eq!(count_history(&db).await, 0);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
async fn postgres_migration_roundtrip() {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run the ignored PostgreSQL migration test");
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let mut snapshot = products(vec![]);
    snapshot.tables[0].name = "ferrum_v04_mig_products".to_owned();
    make_migration(&migrations, "initial", &snapshot).unwrap();
    let db = connect(&url).await.expect("postgres connection");
    let _ = db
        .execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TABLE IF EXISTS \"ferrum_v04_mig_products\"",
        ))
        .await;
    apply_directory(&db, &migrations, false)
        .await
        .expect("postgres migration");
    db.execute(Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrum_v04_mig_products\"",
    ))
    .await
    .unwrap();
    let _ = std::fs::remove_dir_all(&directory);
}
