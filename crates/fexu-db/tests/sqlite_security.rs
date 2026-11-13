//! Exercise the SQLite engine actually loaded through SeaORM and SQLx.
use fexu_db::connect;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};

async fn execute(db: &DatabaseConnection, sql: &str) {
    db.execute(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .unwrap();
}

#[tokio::test]
async fn patched_sqlite_preserves_wal_data_and_history_after_reopening() {
    let directory =
        std::env::temp_dir().join(format!("fexu-sqlite-security-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let file = directory.join("existing.sqlite3");
    let url = format!(
        "sqlite://{}?mode=rwc",
        file.to_string_lossy().replace('\\', "/")
    );
    let writer = connect(&url).await.unwrap();
    let row = writer
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT sqlite_version() AS version".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    let version: String = row.try_get("", "version").unwrap();
    let components: Vec<u32> = version
        .split('.')
        .map(|part| part.parse().unwrap())
        .collect();
    assert!(
        components.as_slice() >= [3, 54, 1].as_slice(),
        "RUSTSEC-2026-0459: unsafe SQLite runtime {version}"
    );

    execute(&writer, "PRAGMA journal_mode=WAL").await;
    execute(&writer, "PRAGMA wal_autocheckpoint=0").await;
    execute(
        &writer,
        "CREATE TABLE records (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
    )
    .await;
    execute(
        &writer,
        "CREATE TABLE migration_history (id TEXT PRIMARY KEY, checksum TEXT NOT NULL)",
    )
    .await;
    execute(&writer, "INSERT INTO records VALUES (1, 'existing data')").await;
    execute(
        &writer,
        "INSERT INTO migration_history VALUES ('app:0001_initial', 'unchanged checksum')",
    )
    .await;

    let transaction = writer.begin().await.unwrap();
    transaction
        .execute(Statement::from_string(
            DbBackend::Sqlite,
            "INSERT INTO records VALUES (2, 'rolled back')".to_owned(),
        ))
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    assert!(directory.join("existing.sqlite3-wal").is_file());

    let reader = connect(&url).await.unwrap();
    let record = reader
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT id, name FROM records".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.try_get::<i64>("", "id").unwrap(), 1);
    assert_eq!(
        record.try_get::<String>("", "name").unwrap(),
        "existing data"
    );
    reader.close().await.unwrap();
    writer.close().await.unwrap();

    let reopened = connect(&url).await.unwrap();
    let record = reopened
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM records".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.try_get::<i64>("", "count").unwrap(), 1);
    let history = reopened
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT checksum FROM migration_history WHERE id='app:0001_initial'".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        history.try_get::<String>("", "checksum").unwrap(),
        "unchanged checksum"
    );
    reopened.close().await.unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
