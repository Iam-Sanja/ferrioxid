use fexu_rusqlite_poc::{connect_postgres, SqliteSession};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::time::{SystemTime, UNIX_EPOCH};

const ADMIN_URL: &str = "postgresql://postgres@127.0.0.1:5432/postgres";

fn disposable_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mixed = nanos ^ ((std::process::id() as u128) << 64);
    format!("fexu_test_{mixed:032x}")
}

fn is_disposable(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("fexu_test_") else {
        return false;
    };
    rest.len() == 32 && rest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

struct Cleanup(String);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let name = self.0.clone();
        if !is_disposable(&name) {
            return;
        }
        let _ = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("cleanup runtime");
            runtime.block_on(drop_database(&name));
        })
        .join();
    }
}

async fn drop_database(name: &str) {
    if !is_disposable(name) {
        return;
    }
    let Ok(admin) = connect_postgres(ADMIN_URL).await else {
        return;
    };
    let terminate = format!(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{name}' AND pid <> pg_backend_pid()"
    );
    let _ = admin
        .execute(Statement::from_string(DatabaseBackend::Postgres, terminate))
        .await;
    let drop_sql = format!("DROP DATABASE IF EXISTS \"{name}\"");
    let _ = admin
        .execute(Statement::from_string(DatabaseBackend::Postgres, drop_sql))
        .await;
    let _ = admin.close().await;
}

#[tokio::test]
async fn postgres_roundtrip_stays_on_sqlx_and_sqlite_stays_on_rusqlite() {
    let name = disposable_name();
    assert!(is_disposable(&name));
    let admin = connect_postgres(ADMIN_URL)
        .await
        .expect("PostgreSQL admin connection on database postgres");
    admin
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("CREATE DATABASE \"{name}\""),
        ))
        .await
        .expect("create isolated fexu_test database");
    let _cleanup = Cleanup(name.clone());
    admin.close().await.unwrap();

    let database = connect_postgres(&format!("postgresql://postgres@127.0.0.1:5432/{name}"))
        .await
        .unwrap();
    let row = database
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT 1::int4 AS ok".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    let ok: i32 = row.try_get("", "ok").unwrap();
    assert_eq!(ok, 1);
    database
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "CREATE TABLE poc_items (id integer PRIMARY KEY, name text NOT NULL)".to_owned(),
        ))
        .await
        .unwrap();
    database
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "INSERT INTO poc_items (id, name) VALUES (1, 'Laptop')".to_owned(),
        ))
        .await
        .unwrap();
    let row = database
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT name FROM poc_items WHERE id = 1".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    let name_value: String = row.try_get("", "name").unwrap();
    assert_eq!(name_value, "Laptop");
    database.close().await.unwrap();

    let sqlite = SqliteSession::open_memory().await.unwrap();
    let version = sqlite.sqlite_version().await.unwrap();
    let parsed = version
        .split('.')
        .take(3)
        .map(|part| {
            part.chars()
                .take_while(|character| character.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    assert!(
        parsed.as_slice() >= [3, 54, 1].as_slice(),
        "sqlite_version={version}"
    );
    sqlite.close().await.unwrap();
}
