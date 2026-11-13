use fexu_rusqlite_poc::{SqlValue, SqliteSession};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn version_at_least(version: &str, expected: [u64; 3]) -> bool {
    let mut parts = version.split('.').map(|part| {
        part.chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0)
    });
    let got = [
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    ];
    got >= expected
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fexu-rusqlite-poc-{nanos}-{tick}"));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn bundled_sqlite_is_at_least_3_54_1() {
    let session = SqliteSession::open_memory().await.unwrap();
    let version = session.sqlite_version().await.unwrap();
    assert!(
        version_at_least(&version, [3, 54, 1]),
        "sqlite_version={version}"
    );
    assert_eq!(
        session.connection_thread_name().await.unwrap(),
        "fexu-sqlite"
    );
    session.close().await.unwrap();
}

#[tokio::test]
async fn parameterized_select_insert_update_and_delete() {
    let session = SqliteSession::open_url("sqlite::memory:").await.unwrap();
    session
        .execute("CREATE TABLE item (id INTEGER PRIMARY KEY, name TEXT)", &[])
        .await
        .unwrap();
    let inserted = session
        .execute(
            "INSERT INTO item (id, name) VALUES (?1, ?2)",
            &[SqlValue::Integer(1), SqlValue::Text("Laptop".to_owned())],
        )
        .await
        .unwrap();
    assert_eq!(inserted, 1);
    session
        .execute(
            "UPDATE item SET name = ?1 WHERE id = ?2",
            &[SqlValue::Text("Desk".to_owned()), SqlValue::Integer(1)],
        )
        .await
        .unwrap();
    let rows = session
        .query(
            "SELECT name FROM item WHERE id = ?1",
            &[SqlValue::Integer(1)],
        )
        .await
        .unwrap();
    assert_eq!(rows, vec![vec![SqlValue::Text("Desk".to_owned())]]);
    session
        .execute("DELETE FROM item WHERE id = ?1", &[SqlValue::Integer(1)])
        .await
        .unwrap();
    let rows = session.query("SELECT name FROM item", &[]).await.unwrap();
    assert!(rows.is_empty());
    session.close().await.unwrap();
}

#[tokio::test]
async fn commit_persists_and_rollback_discards() {
    let dir = TempDir::new();
    let path = dir.path().join("app.sqlite3");
    let session = SqliteSession::open_path(&path).await.unwrap();
    session
        .execute(
            "CREATE TABLE item (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            &[],
        )
        .await
        .unwrap();
    let journal = session.query("PRAGMA journal_mode", &[]).await.unwrap();
    assert_eq!(journal, vec![vec![SqlValue::Text("wal".to_owned())]]);

    let transaction = session.begin().await.unwrap();
    transaction
        .execute(
            "INSERT INTO item (id, name) VALUES (?1, ?2)",
            &[SqlValue::Integer(1), SqlValue::Text("Laptop".to_owned())],
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let transaction = session.begin().await.unwrap();
    transaction
        .execute(
            "INSERT INTO item (id, name) VALUES (?1, ?2)",
            &[SqlValue::Integer(2), SqlValue::Text("Temporary".to_owned())],
        )
        .await
        .unwrap();
    transaction.rollback().await.unwrap();

    session.close().await.unwrap();
    let reopened = SqliteSession::open_path(&path).await.unwrap();
    let rows = reopened
        .query("SELECT id, name FROM item ORDER BY id", &[])
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![vec![
            SqlValue::Integer(1),
            SqlValue::Text("Laptop".to_owned())
        ]]
    );
    let journal = reopened.query("PRAGMA journal_mode", &[]).await.unwrap();
    assert_eq!(journal, vec![vec![SqlValue::Text("wal".to_owned())]]);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn dropped_transaction_rolls_back() {
    let session = SqliteSession::open_memory().await.unwrap();
    session
        .execute("CREATE TABLE item (id INTEGER PRIMARY KEY)", &[])
        .await
        .unwrap();
    let transaction = session.begin().await.unwrap();
    transaction
        .execute("INSERT INTO item (id) VALUES (1)", &[])
        .await
        .unwrap();
    drop(transaction);
    let rows = session
        .query("SELECT COUNT(*) FROM item", &[])
        .await
        .unwrap();
    assert_eq!(rows, vec![vec![SqlValue::Integer(0)]]);
    session.close().await.unwrap();
}

#[tokio::test]
async fn transaction_holds_the_connection_until_commit() {
    let session = SqliteSession::open_memory().await.unwrap();
    session
        .execute(
            "CREATE TABLE item (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            &[],
        )
        .await
        .unwrap();
    let transaction = session.begin().await.unwrap();
    transaction
        .execute("INSERT INTO item (id, name) VALUES (1, 'Laptop')", &[])
        .await
        .unwrap();
    let waiting = session.clone();
    let pending =
        tokio::spawn(async move { waiting.query("SELECT COUNT(*) FROM item", &[]).await });
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(!pending.is_finished());
    transaction.commit().await.unwrap();
    let rows = pending.await.unwrap().unwrap();
    assert_eq!(rows, vec![vec![SqlValue::Integer(1)]]);
    session.close().await.unwrap();
}

#[tokio::test]
async fn second_connection_waits_for_the_writer() {
    let dir = TempDir::new();
    let path = dir.path().join("locks.sqlite3");
    let writer = SqliteSession::open_path(&path).await.unwrap();
    writer
        .execute("CREATE TABLE item (id INTEGER PRIMARY KEY)", &[])
        .await
        .unwrap();
    let reader = SqliteSession::open_path(&path).await.unwrap();
    let transaction = writer.begin().await.unwrap();
    transaction
        .execute("INSERT INTO item (id) VALUES (1)", &[])
        .await
        .unwrap();
    let pending = tokio::spawn(async move {
        let result = reader
            .execute("INSERT INTO item (id) VALUES (2)", &[])
            .await;
        reader.close().await.unwrap();
        result
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(!pending.is_finished());
    transaction.commit().await.unwrap();
    assert_eq!(pending.await.unwrap().unwrap(), 1);
    writer.close().await.unwrap();
}

#[tokio::test]
async fn pin_suspends_foreign_keys_only_for_its_transaction() {
    let dir = TempDir::new();
    let path = dir.path().join("fk.sqlite3");
    let session = SqliteSession::open_path(&path).await.unwrap();
    session
        .execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)", &[])
        .await
        .unwrap();
    session
        .execute(
            "CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parent(id))",
            &[],
        )
        .await
        .unwrap();
    session
        .execute("INSERT INTO parent (id) VALUES (1)", &[])
        .await
        .unwrap();
    assert!(session
        .execute("INSERT INTO child (id, parent_id) VALUES (1, 99)", &[])
        .await
        .is_err());

    let mut pin = session.pin().await.unwrap();
    pin.suspend_foreign_keys().await.unwrap();
    pin.begin().await.unwrap();
    pin.execute("INSERT INTO child (id, parent_id) VALUES (1, 99)", &[])
        .await
        .unwrap();
    pin.rollback().await.unwrap();
    pin.restore_foreign_keys().await.unwrap();
    drop(pin);

    assert!(session
        .execute("INSERT INTO child (id, parent_id) VALUES (2, 99)", &[])
        .await
        .is_err());
    let rows = session.query("SELECT id FROM parent", &[]).await.unwrap();
    assert_eq!(rows, vec![vec![SqlValue::Integer(1)]]);
    session.close().await.unwrap();
}

#[tokio::test]
async fn dropping_a_pin_with_foreign_keys_off_reopens_a_safe_file() {
    let dir = TempDir::new();
    let path = dir.path().join("discard.sqlite3");
    let session = SqliteSession::open_path(&path).await.unwrap();
    session
        .execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)", &[])
        .await
        .unwrap();
    session
        .execute(
            "CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parent(id))",
            &[],
        )
        .await
        .unwrap();
    session
        .execute("INSERT INTO parent (id) VALUES (1)", &[])
        .await
        .unwrap();
    let mut pin = session.pin().await.unwrap();
    pin.suspend_foreign_keys().await.unwrap();
    drop(pin);
    let flag = session.query("PRAGMA foreign_keys", &[]).await.unwrap();
    assert!(session.discard_error().is_none());
    assert_eq!(flag, vec![vec![SqlValue::Integer(1)]]);
    assert!(session
        .execute("INSERT INTO child (id, parent_id) VALUES (1, 99)", &[])
        .await
        .is_err());
    let rows = session.query("SELECT id FROM parent", &[]).await.unwrap();
    assert_eq!(rows, vec![vec![SqlValue::Integer(1)]]);
    session.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_work_does_not_block_the_runtime_worker() {
    let session = SqliteSession::open_memory().await.unwrap();
    let ran = std::sync::Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(40)).await;
        flag.store(true, Ordering::SeqCst);
    });
    let started = Instant::now();
    session
        .wait_on_connection(Duration::from_millis(200))
        .await
        .unwrap();
    assert!(
        ran.load(Ordering::SeqCst),
        "timer task did not run while the SQLite thread was busy"
    );
    assert!(started.elapsed() >= Duration::from_millis(200));
    session.close().await.unwrap();
}
