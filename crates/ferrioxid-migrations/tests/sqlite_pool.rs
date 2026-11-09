//! A rebuild must suspend and restore foreign keys on one pooled SQLite connection.

use ferrioxid_migrations::{
    apply_directory, make_migration, make_migration_with_backfills, BackfillLiteral,
    ColumnBackfill, ColumnSnapshot, ForeignKeySnapshot, LogicalType, OnDelete, SchemaSnapshot,
    TableSnapshot,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const POOL_SIZE: u32 = 4;
const PRODUCT: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const CHILD: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ferrioxid-pool-{nanos}-{tick}"));
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
    }
}

fn schema() -> SchemaSnapshot {
    SchemaSnapshot::from_tables(vec![
        TableSnapshot {
            name: "products".to_owned(),
            columns: vec![
                ColumnSnapshot {
                    name: "id".to_owned(),
                    logical_type: LogicalType::Uuid,
                    primary_key: true,
                    nullable: false,
                    default: None,
                },
                column("name", LogicalType::String, false),
            ],
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
        },
        TableSnapshot {
            name: "cart_items".to_owned(),
            columns: vec![
                ColumnSnapshot {
                    name: "id".to_owned(),
                    logical_type: LogicalType::Uuid,
                    primary_key: true,
                    nullable: false,
                    default: None,
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
        },
    ])
}

async fn open_pool(path: &Path) -> DatabaseConnection {
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    );
    let mut options = sea_orm::ConnectOptions::new(url);
    options
        .max_connections(POOL_SIZE)
        .min_connections(POOL_SIZE);
    options.map_sqlx_sqlite_opts(|sqlite| sqlite.pragma("foreign_keys", "ON"));
    sea_orm::Database::connect(options).await.unwrap()
}

async fn execute(db: &impl ConnectionTrait, sql: &str) {
    db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .unwrap();
}

async fn cell(db: &impl ConnectionTrait, sql: &str) -> String {
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

async fn foreign_keys_enabled(db: &impl ConnectionTrait) -> i64 {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys",
        ))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<i64>("", "foreign_keys")
        .or_else(|_| row.try_get::<i32>("", "foreign_keys").map(i64::from))
        .unwrap()
}

async fn assert_every_connection_enforces(db: &DatabaseConnection) {
    let mut held = Vec::new();
    for _ in 0..POOL_SIZE {
        let transaction = db.begin().await.unwrap();
        assert_eq!(foreign_keys_enabled(&transaction).await, 1);
        held.push(transaction);
    }
}

async fn seed(db: &DatabaseConnection) {
    execute(
        db,
        &format!("INSERT INTO \"products\" (\"id\", \"name\") VALUES ('{PRODUCT}', 'Laptop')"),
    )
    .await;
    execute(
        db,
        &format!(
            "INSERT INTO \"cart_items\" (\"id\", \"product_id\") VALUES ('{CHILD}', '{PRODUCT}')"
        ),
    )
    .await;
}

#[tokio::test]
async fn rebuild_keeps_children_and_returns_every_connection_with_foreign_keys_on() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = schema();
    make_migration(&migrations, "initial", &initial).unwrap();
    let database = directory.join("app.sqlite3");
    let db = open_pool(&database).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    seed(&db).await;
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
    apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        cell(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "Laptop"
    );
    assert_eq!(
        cell(&db, "SELECT \"stock\" AS value FROM \"products\"").await,
        "3"
    );
    assert_eq!(
        cell(&db, "SELECT COUNT(*) AS value FROM \"cart_items\"").await,
        "1"
    );
    assert_eq!(
        cell(
            &db,
            "SELECT COUNT(*) AS value FROM \"_ferrioxid_migrations\"",
        )
        .await,
        "2"
    );
    let orphan = db
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "INSERT INTO \"cart_items\" (\"id\", \"product_id\") VALUES ('cccccccc-cccc-4ccc-8ccc-cccccccccccc', '99999999-9999-4999-8999-999999999999')",
        ))
        .await;
    assert!(orphan.is_err());
    let deleted = db
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("DELETE FROM \"products\" WHERE \"id\" = '{PRODUCT}'"),
        ))
        .await;
    assert!(deleted.is_err());
    assert_every_connection_enforces(&db).await;
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn failed_rebuild_rolls_back_and_leaves_no_connection_with_foreign_keys_off() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let initial = schema();
    make_migration(&migrations, "initial", &initial).unwrap();
    let db = open_pool(&directory.join("app.sqlite3")).await;
    apply_directory(&db, &migrations, false).await.unwrap();
    seed(&db).await;
    execute(
        &db,
        "CREATE VIEW product_names AS SELECT name FROM products",
    )
    .await;
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
            value: BackfillLiteral::I64(9),
        }],
    )
    .unwrap();
    let blocked = apply_directory(&db, &migrations, false).await.unwrap_err();
    assert!(blocked.0.contains("view"), "{}", blocked.0);
    assert_eq!(
        cell(&db, "SELECT \"name\" AS value FROM \"products\"").await,
        "Laptop"
    );
    assert_eq!(
        cell(&db, "SELECT COUNT(*) AS value FROM \"cart_items\"").await,
        "1"
    );
    assert_eq!(
        cell(
            &db,
            "SELECT COUNT(*) AS value FROM \"_ferrioxid_migrations\"",
        )
        .await,
        "1"
    );
    let orphan = db
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "INSERT INTO \"cart_items\" (\"id\", \"product_id\") VALUES ('cccccccc-cccc-4ccc-8ccc-cccccccccccc', '99999999-9999-4999-8999-999999999999')",
        ))
        .await;
    assert!(orphan.is_err());
    assert_every_connection_enforces(&db).await;
    let _ = std::fs::remove_dir_all(directory);
}
