//! SQLite-Migrationsläufe ohne Docker und ohne die Entwicklungsdatenbank.

use ferrum_core::{ColumnKind, DatabaseKind};
use ferrum_db::{connect, render_create_table, PhysicalColumn};
use ferrum_migrations::{
    apply_directory, baseline_database, history_exists, make_migration, migration_status,
    plan_change, reseal_migration, reseal_unapplied, save_migration, seal,
    write_explicit_migration, ColumnSnapshot, LogicalType, MakeOutcome, MigrationOperation,
    SchemaSnapshot, TableSnapshot, FORMAT_VERSION,
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
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
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
    changed.tables[0].columns[2].logical_type = LogicalType::String;
    changed.tables[0].columns[2].default = None;
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

async fn cell_text(db: &DatabaseConnection, sql: &str) -> String {
    let row = db
        .query_one(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<String>("", "value")
        .or_else(|_| {
            row.try_get::<i64>("", "value")
                .map(|number| number.to_string())
        })
        .or_else(|_| {
            row.try_get::<i32>("", "value")
                .map(|number| number.to_string())
        })
        .unwrap()
}

fn product_sql() -> String {
    render_create_table(
        DatabaseKind::Sqlite,
        "products",
        &[
            PhysicalColumn {
                name: "id",
                kind: ColumnKind::Uuid,
                nullable: false,
                primary_key: true,
                default_bool: None,
            },
            PhysicalColumn {
                name: "name",
                kind: ColumnKind::Text,
                nullable: false,
                primary_key: false,
                default_bool: None,
            },
            PhysicalColumn {
                name: "active",
                kind: ColumnKind::Boolean,
                nullable: false,
                primary_key: false,
                default_bool: Some(true),
            },
        ],
        false,
    )
}

#[tokio::test]
async fn rename_column_keeps_values_snapshot_and_history() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO \"products\" (\"id\", \"name\", \"active\") VALUES ('11111111-1111-4111-8111-111111111111', 'Laptop', 1)",
    ))
    .await
    .unwrap();
    let mut renamed = initial.clone();
    renamed.tables[0].columns[1].name = "title".to_owned();
    let written = write_explicit_migration(
        &migrations,
        "rename_title",
        &renamed,
        MigrationOperation::RenameColumn {
            table: "products".to_owned(),
            from: "name".to_owned(),
            to: "title".to_owned(),
        },
    )
    .unwrap();
    assert!(matches!(written, MakeOutcome::Written { ref id, .. } if id == "0002_rename_title"));
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        cell_text(&db, "SELECT \"title\" AS value FROM \"products\"").await,
        "Laptop"
    );
    let names = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name AS value FROM pragma_table_info('products')",
        ))
        .await
        .unwrap();
    let names = names
        .iter()
        .map(|row| row.try_get::<String>("", "value").unwrap())
        .collect::<Vec<_>>();
    assert!(names.contains(&"title".to_owned()));
    assert!(!names.contains(&"name".to_owned()));
    assert_eq!(count_history(&db).await, 2);
    let again = make_migration(&migrations, "again", &renamed).unwrap();
    assert!(matches!(again, MakeOutcome::Unchanged));
    let status = migration_status(&db, &migrations, Some(&renamed))
        .await
        .unwrap();
    assert!(status.schema_note.contains("entspricht"));
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn rename_table_moves_rows_and_removes_the_old_name() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO \"products\" (\"id\", \"name\", \"active\") VALUES ('11111111-1111-4111-8111-111111111111', 'Laptop', 1)",
    ))
    .await
    .unwrap();
    let mut renamed = initial.clone();
    renamed.tables[0].name = "items".to_owned();
    write_explicit_migration(
        &migrations,
        "rename_items",
        &renamed,
        MigrationOperation::RenameTable {
            from: "products".to_owned(),
            to: "items".to_owned(),
        },
    )
    .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        cell_text(&db, "SELECT \"name\" AS value FROM \"items\"").await,
        "Laptop"
    );
    let old = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
            [Value::from("products")],
        ))
        .await
        .unwrap();
    assert!(old.is_none());
    let new_table = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
            [Value::from("items")],
        ))
        .await
        .unwrap();
    assert!(new_table.is_some());
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn sqlite_rebuild_converts_canonical_integers_and_keeps_the_row_count() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO \"products\" (\"id\", \"name\", \"active\") VALUES ('11111111-1111-4111-8111-111111111111', '42', 1)",
    ))
    .await
    .unwrap();
    let mut changed = initial.clone();
    changed.tables[0].columns[1].logical_type = LogicalType::I64;
    make_migration(&migrations, "price_code", &changed).unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        cell_text(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "42"
    );
    assert_eq!(
        cell_text(&db, "SELECT COUNT(*) AS value FROM \"products\"").await,
        "1"
    );
    let info = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT \"notnull\" AS value FROM pragma_table_info('products') WHERE name = 'name'",
        ))
        .await
        .unwrap()
        .unwrap();
    let notnull: i64 = info
        .try_get::<i64>("", "value")
        .or_else(|_| info.try_get::<i32>("", "value").map(i64::from))
        .unwrap();
    assert_eq!(notnull, 1);
    assert_eq!(count_history(&db).await, 2);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn failed_rebuild_keeps_the_original_table_and_writes_no_history() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO \"products\" (\"id\", \"name\", \"active\") VALUES ('11111111-1111-4111-8111-111111111111', 'Laptop', 1)",
    ))
    .await
    .unwrap();
    let mut changed = initial.clone();
    changed.tables[0].columns[1].logical_type = LogicalType::I64;
    make_migration(&migrations, "bad_number", &changed).unwrap();
    let error = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(error.0.contains("zurückgerollt"));
    assert_eq!(
        cell_text(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "Laptop"
    );
    assert_eq!(count_history(&db).await, 1);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn not_null_change_is_blocked_while_nulls_exist() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![column("note", LogicalType::String, false, true, None)]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO \"products\" (\"id\", \"name\", \"active\", \"note\") VALUES ('11111111-1111-4111-8111-111111111111', 'Laptop', 1, NULL)",
    ))
    .await
    .unwrap();
    let mut tightened = initial.clone();
    tightened.tables[0].columns[3].nullable = false;
    make_migration(&migrations, "note_required", &tightened).unwrap();
    let error = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(error.0.contains("NULL"));
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT \"note\" AS value FROM \"products\"",
        ))
        .await
        .unwrap()
        .unwrap();
    let note: Option<String> = row.try_get("", "value").unwrap();
    assert!(note.is_none());
    assert_eq!(count_history(&db).await, 1);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn baseline_marks_history_without_touching_rows() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        product_sql(),
    ))
    .await
    .unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO \"products\" (\"id\", \"name\", \"active\") VALUES ('11111111-1111-4111-8111-111111111111', 'Laptop', 1)",
    ))
    .await
    .unwrap();
    let refused = baseline_database(&db, &migrations, false)
        .await
        .unwrap_err();
    assert!(refused.0.contains("Prüfung erfolgreich"));
    assert!(!history_exists(&db).await.unwrap());
    let ids = baseline_database(&db, &migrations, true).await.unwrap();
    assert_eq!(ids, vec!["0001_initial".to_owned()]);
    assert_eq!(
        cell_text(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "Laptop"
    );
    let second = apply_directory(&db, &migrations, false).await.unwrap();
    assert!(second.applied.is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn mismatched_baseline_writes_no_history() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    make_migration(&migrations, "initial", &products(vec![])).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "CREATE TABLE \"products\" (\"id\" text PRIMARY KEY NOT NULL, \"name\" text NOT NULL)",
    ))
    .await
    .unwrap();
    let error = baseline_database(&db, &migrations, true).await.unwrap_err();
    assert!(error.0.contains("Baseline abgebrochen"));
    assert!(!history_exists(&db).await.unwrap());
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn destructive_chain_cannot_be_baselined() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let with_note = products(vec![column("note", LogicalType::String, false, true, None)]);
    make_migration(&migrations, "initial", &with_note).unwrap();
    make_migration(&migrations, "drop_note", &products(vec![])).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        product_sql(),
    ))
    .await
    .unwrap();
    let error = baseline_database(&db, &migrations, true).await.unwrap_err();
    assert!(error.0.contains("manuell"));
    assert!(!history_exists(&db).await.unwrap());
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn reseal_refuses_an_applied_migration_and_repairs_an_unapplied_checksum() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![]);
    make_migration(&migrations, "initial", &initial).unwrap();
    let path = migrations.join("0001_initial.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["checksum"] = serde_json::Value::String("0".repeat(64));
    std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    assert!(ferrum_migrations::load_migration(&path).is_err());
    reseal_migration(&path).unwrap();
    ferrum_migrations::load_migration(&path).unwrap();
    let db = open_db(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    let error = reseal_unapplied(&db, &path).await.unwrap_err();
    assert!(error.0.contains("bereits angewendet"));
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
