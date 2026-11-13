//! Required columns and foreign keys on populated SQLite tables.

use fexu_migrations::{
    apply_directory, make_migration, make_migration_with_backfills, BackfillLiteral,
    ColumnBackfill, ColumnSnapshot, ForeignKeySnapshot, IndexSnapshot, LogicalType, MakeOutcome,
    OnDelete, SchemaSnapshot, TableSnapshot,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
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
    let path = std::env::temp_dir().join(format!("fexu-rebuild-{nanos}-{tick}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn column(name: &str, logical_type: LogicalType, nullable: bool) -> ColumnSnapshot {
    ColumnSnapshot {
        name: name.to_owned(),
        logical_type,
        primary_key: false,
        nullable,
        default: None,
        precision: None,
        scale: None,
        choices: Vec::new(),
    }
}

fn products(extra: Vec<ColumnSnapshot>, indexes: Vec<IndexSnapshot>) -> SchemaSnapshot {
    let mut columns = vec![
        ColumnSnapshot {
            name: "id".to_owned(),
            logical_type: LogicalType::Uuid,
            primary_key: true,
            nullable: false,
            default: None,
            precision: None,
            scale: None,
            choices: Vec::new(),
        },
        column("name", LogicalType::String, false),
    ];
    columns.extend(extra);
    SchemaSnapshot::from_tables(vec![TableSnapshot {
        name: "products".to_owned(),
        columns,
        unique_constraints: Vec::new(),
        indexes,
        foreign_keys: Vec::new(),
    }])
}

fn open_url(path: &Path) -> String {
    format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    )
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), sea_orm::DbErr> {
    db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .map(|_| ())
}

async fn cell(db: &DatabaseConnection, sql: &str) -> String {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<i64>("", "value")
        .map(|value| value.to_string())
        .or_else(|_| {
            row.try_get::<i32>("", "value")
                .map(|value| value.to_string())
        })
        .or_else(|_| row.try_get::<String>("", "value"))
        .unwrap()
}

async fn history_count(db: &DatabaseConnection) -> i64 {
    cell(db, "SELECT COUNT(*) AS value FROM \"_fexu_migrations\"")
        .await
        .parse()
        .unwrap()
}

#[tokio::test]
async fn explicit_backfill_keeps_rows_and_missing_backfill_writes_nothing() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(Vec::new(), Vec::new());
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = fexu_db::connect(&open_url(&directory.join("app.sqlite3")))
        .await
        .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    execute(
        &db,
        "INSERT INTO \"products\" (\"id\", \"name\") VALUES ('11111111-1111-4111-8111-111111111111', 'Laptop')",
    )
    .await
    .unwrap();
    let mut required = initial.clone();
    required.tables[0]
        .columns
        .push(column("stock", LogicalType::I64, false));
    let refused = make_migration(&migrations, "stock", &required).unwrap_err();
    assert!(refused.0.contains("NOT NULL"), "{}", refused.0);
    assert!(refused.0.contains("backfill"), "{}", refused.0);
    assert!(!migrations.join("0002_stock.json").exists());
    assert_eq!(
        cell(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "Laptop"
    );
    assert_eq!(history_count(&db).await, 1);

    let mismatch = make_migration_with_backfills(
        &migrations,
        "stock",
        &required,
        &[ColumnBackfill {
            table: "products".to_owned(),
            column: "stock".to_owned(),
            value: BackfillLiteral::Text("nope".to_owned()),
        }],
    )
    .unwrap_err();
    assert!(mismatch.0.contains("does not match"), "{}", mismatch.0);
    assert!(!migrations.join("0002_stock.json").exists());

    let written = make_migration_with_backfills(
        &migrations,
        "stock",
        &required,
        &[ColumnBackfill {
            table: "products".to_owned(),
            column: "stock".to_owned(),
            value: BackfillLiteral::I64(7),
        }],
    )
    .unwrap();
    assert!(matches!(written, MakeOutcome::Written { .. }));
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        cell(&db, "SELECT \"stock\" AS value FROM \"products\"").await,
        "7"
    );
    assert_eq!(
        cell(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "Laptop"
    );
    assert_eq!(
        cell(&db, "SELECT COUNT(*) AS value FROM \"products\"").await,
        "1"
    );
    assert!(apply_directory(&db, &migrations, false)
        .await
        .unwrap()
        .applied
        .is_empty());
    assert_eq!(history_count(&db).await, 2);
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn staged_not_null_does_not_invent_values_and_rolls_back() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = products(vec![column("note", LogicalType::String, true)], Vec::new());
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = fexu_db::connect(&open_url(&directory.join("app.sqlite3")))
        .await
        .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    execute(
        &db,
        "INSERT INTO \"products\" (\"id\", \"name\", \"note\") VALUES ('11111111-1111-4111-8111-111111111111', 'Laptop', NULL)",
    )
    .await
    .unwrap();
    let mut required = initial.clone();
    required.tables[0].columns[2].nullable = false;
    make_migration(&migrations, "note_required", &required).unwrap();
    let error = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(error.0.contains("NULL"), "{}", error.0);
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
    assert_eq!(history_count(&db).await, 1);
    execute(
        &db,
        "UPDATE \"products\" SET \"note\" = 'kept' WHERE \"id\" = '11111111-1111-4111-8111-111111111111'",
    )
    .await
    .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        cell(&db, "SELECT \"note\" AS value FROM \"products\"").await,
        "kept"
    );
    assert_eq!(history_count(&db).await, 2);
    assert!(apply_directory(&db, &migrations, false)
        .await
        .unwrap()
        .applied
        .is_empty());
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn invalid_foreign_key_rolls_back_and_a_valid_one_is_enforced() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let mut initial = products(Vec::new(), Vec::new());
    initial.tables.push(TableSnapshot {
        name: "cart_items".to_owned(),
        columns: vec![
            ColumnSnapshot {
                name: "id".to_owned(),
                logical_type: LogicalType::Uuid,
                primary_key: true,
                nullable: false,
                default: None,
                precision: None,
                scale: None,
                choices: Vec::new(),
            },
            column("product_id", LogicalType::Uuid, false),
        ],
        unique_constraints: Vec::new(),
        indexes: Vec::new(),
        foreign_keys: Vec::new(),
    });
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = fexu_db::connect(&open_url(&directory.join("app.sqlite3")))
        .await
        .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    execute(
        &db,
        "INSERT INTO \"products\" (\"id\", \"name\") VALUES ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', 'Laptop')",
    )
    .await
    .unwrap();
    execute(
        &db,
        "INSERT INTO \"cart_items\" (\"id\", \"product_id\") VALUES ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', '99999999-9999-4999-8999-999999999999')",
    )
    .await
    .unwrap();
    let mut linked = initial.clone();
    linked.tables[1].foreign_keys.push(ForeignKeySnapshot {
        name: "fk_cart_items_product_id_products_id".to_owned(),
        columns: vec!["product_id".to_owned()],
        referenced_table: "products".to_owned(),
        referenced_columns: vec!["id".to_owned()],
        on_delete: OnDelete::Restrict,
        on_update: "no_action".to_owned(),
    });
    make_migration(&migrations, "product_fk", &linked).unwrap();
    let error = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(error.0.contains("do not match"), "{}", error.0);
    assert_eq!(
        cell(&db, "SELECT COUNT(*) AS value FROM \"cart_items\"").await,
        "1"
    );
    assert_eq!(history_count(&db).await, 1);
    execute(
        &db,
        "UPDATE \"cart_items\" SET \"product_id\" = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'",
    )
    .await
    .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    assert!(execute(
        &db,
        "INSERT INTO \"cart_items\" (\"id\", \"product_id\") VALUES ('cccccccc-cccc-4ccc-8ccc-cccccccccccc', '99999999-9999-4999-8999-999999999999')",
    )
    .await
    .is_err());
    assert!(execute(
        &db,
        "DELETE FROM \"products\" WHERE \"id\" = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'",
    )
    .await
    .is_err());
    assert_eq!(
        cell(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "Laptop"
    );
    assert_eq!(history_count(&db).await, 2);
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn rebuild_preserves_indexes_triggers_and_incoming_keys_but_blocks_views() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let mut initial = products(
        Vec::new(),
        vec![IndexSnapshot {
            name: "ix_products_name".to_owned(),
            columns: vec!["name".to_owned()],
        }],
    );
    initial.tables.push(TableSnapshot {
        name: "cart_items".to_owned(),
        columns: vec![
            ColumnSnapshot {
                name: "id".to_owned(),
                logical_type: LogicalType::Uuid,
                primary_key: true,
                nullable: false,
                default: None,
                precision: None,
                scale: None,
                choices: Vec::new(),
            },
            column("product_id", LogicalType::Uuid, false),
        ],
        unique_constraints: Vec::new(),
        indexes: Vec::new(),
        foreign_keys: vec![ForeignKeySnapshot {
            name: "fk_cart_items_product_id_products_id".to_owned(),
            columns: vec!["product_id".to_owned()],
            referenced_table: "products".to_owned(),
            referenced_columns: vec!["id".to_owned()],
            on_delete: OnDelete::Restrict,
            on_update: "no_action".to_owned(),
        }],
    });
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = fexu_db::connect(&open_url(&directory.join("app.sqlite3")))
        .await
        .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    execute(
        &db,
        "INSERT INTO \"products\" (\"id\", \"name\") VALUES ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', 'Laptop')",
    )
    .await
    .unwrap();
    execute(
        &db,
        "INSERT INTO \"cart_items\" (\"id\", \"product_id\") VALUES ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa')",
    )
    .await
    .unwrap();
    execute(
        &db,
        "CREATE TRIGGER products_guard BEFORE DELETE ON products BEGIN SELECT RAISE(ABORT, 'no delete'); END",
    )
    .await
    .unwrap();
    execute(
        &db,
        "CREATE VIEW product_names AS SELECT name FROM products",
    )
    .await
    .unwrap();
    let mut stocked = initial.clone();
    stocked.tables[0]
        .columns
        .push(column("stock", LogicalType::I64, false));
    make_migration_with_backfills(
        &migrations,
        "stock",
        &stocked,
        &[ColumnBackfill {
            table: "products".to_owned(),
            column: "stock".to_owned(),
            value: BackfillLiteral::I64(3),
        }],
    )
    .unwrap();
    let blocked = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(blocked.0.contains("view"), "{}", blocked.0);
    assert_eq!(
        cell(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "Laptop"
    );
    assert_eq!(history_count(&db).await, 1);
    execute(&db, "DROP VIEW product_names").await.unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        cell(&db, "SELECT \"stock\" AS value FROM \"products\"").await,
        "3"
    );
    assert_eq!(
        cell(
            &db,
            "SELECT COUNT(*) AS value FROM sqlite_master WHERE type = 'index' AND name = 'ix_products_name'",
        )
        .await,
        "1"
    );
    execute(&db, "DELETE FROM \"cart_items\"").await.unwrap();
    assert!(execute(
        &db,
        "DELETE FROM \"products\" WHERE \"id\" = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'",
    )
    .await
    .is_err());
    assert_eq!(
        cell(&db, "SELECT COUNT(*) AS value FROM \"products\"").await,
        "1"
    );
    assert_eq!(history_count(&db).await, 2);
    let _ = std::fs::remove_dir_all(directory);
}
