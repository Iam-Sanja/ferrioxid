//! Driver round-trips for the values Fexu binds itself.
//!
//! Fexu decimal columns stay 16-byte blobs on both backends. `NUMERIC` is a
//! separate PostgreSQL value and is not used as the storage of those columns.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use fexu_core::{encode_decimal, ColumnKind, DatabaseKind};
use fexu_db::driver::{
    ConnectionTrait, DatabaseBackend, DbErr, Statement, TransactionTrait, Value,
};
use fexu_db::{connect, render_create_table, DisposablePostgres, PhysicalColumn};
use rust_decimal::Decimal;
use serde_json::{json, Value as Json};
use uuid::Uuid;

fn column(name: &'static str, kind: ColumnKind) -> PhysicalColumn<'static> {
    PhysicalColumn {
        name,
        kind,
        nullable: true,
        primary_key: false,
        default_bool: None,
    }
}

#[test]
fn rendered_decimal_and_json_ddl_stays_on_the_existing_types() {
    let columns = [
        column("amount", ColumnKind::Decimal),
        column("doc", ColumnKind::Json),
    ];
    let sqlite = render_create_table(DatabaseKind::Sqlite, "prices", &columns, false);
    let postgres = render_create_table(DatabaseKind::Postgres, "prices", &columns, false);
    assert!(sqlite.to_ascii_lowercase().contains("blob"));
    assert!(postgres.to_ascii_lowercase().contains("bytea"));
    assert!(!sqlite.to_ascii_lowercase().contains("numeric"));
    assert!(!postgres.to_ascii_lowercase().contains("numeric"));
    assert!(sqlite.to_ascii_lowercase().contains("text"));
    assert!(postgres.to_ascii_lowercase().contains("jsonb"));
}

#[tokio::test]
async fn sqlite_roundtrip_uses_blobs_transactions_and_foreign_keys() {
    let db = connect("sqlite::memory:").await.unwrap();
    let version: String = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT sqlite_version() AS version",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "version")
        .unwrap();
    let parts: Vec<u32> = version
        .split('.')
        .map(|part| part.parse().unwrap())
        .collect();
    assert!(parts.as_slice() >= [3, 54, 1].as_slice(), "{version}");

    execute(&db, "CREATE TABLE parent (id INTEGER PRIMARY KEY)").await;
    execute(
        &db,
        "CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parent(id))",
    )
    .await;
    let rejected = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO child (id, parent_id) VALUES (?, ?)",
            [Value::Int(Some(1)), Value::Int(Some(9))],
        ))
        .await
        .unwrap_err();
    assert_eq!(
        rejected.constraint(),
        Some(fexu_db::driver::ConstraintKind::ForeignKey)
    );

    let amount = encode_decimal(Decimal::new(1999, 2), 10, 2).unwrap();
    let inserted = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO parent (id) VALUES (?)",
            [Value::BigInt(Some(7))],
        ))
        .await
        .unwrap();
    assert_eq!(inserted.rows_affected(), 1);
    let decimal = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "CREATE TABLE money (amount BLOB)",
            [],
        ))
        .await
        .unwrap();
    assert_eq!(decimal.rows_affected(), 0);
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO money (amount) VALUES (?)",
        [Value::Bytes(Some(Box::new(amount.to_vec())))],
    ))
    .await
    .unwrap();
    let stored: Vec<u8> = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT amount FROM money",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "amount")
        .unwrap();
    assert_eq!(stored, amount);
    let numeric = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO money (amount) VALUES (?)",
            [Value::Decimal(Some(Box::new(Decimal::new(1999, 2))))],
        ))
        .await
        .unwrap_err();
    assert!(numeric.to_string().contains("16-byte blobs"));

    let transaction = db.begin().await.unwrap();
    transaction
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO parent (id) VALUES (?)",
            [Value::Int(Some(8))],
        ))
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    let count: i64 = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM parent",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap();
    assert_eq!(count, 1);
    db.close().await.unwrap();
}

#[tokio::test]
#[ignore = "creates and drops one fexu_test_* PostgreSQL database"]
async fn postgres_native_types_and_decimal_blob() {
    let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let database = DisposablePostgres::create(&admin)
        .await
        .expect("isolated database");
    let outcome = postgres_roundtrip(database.url()).await;
    let dropped = database.drop_database().await;
    outcome.unwrap();
    dropped.unwrap();
}

async fn postgres_roundtrip(url: &str) -> Result<(), DbErr> {
    let db = connect(url).await?;
    db.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        "CREATE TABLE types (
            id INTEGER PRIMARY KEY,
            big BIGINT,
            label TEXT,
            flag BOOLEAN,
            payload BYTEA,
            doc JSON,
            docb JSONB,
            uid UUID,
            day DATE,
            clock TIME,
            stamp TIMESTAMP,
            stamptz TIMESTAMPTZ,
            amount NUMERIC,
            fexu_decimal BYTEA,
            missing TEXT
        )",
    ))
    .await?;
    let uid = Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef);
    let day = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
    let clock = NaiveTime::from_hms_opt(21, 9, 1).unwrap();
    let stamp = day.and_hms_opt(21, 9, 2).unwrap();
    let stamptz: DateTime<Utc> = stamp.and_utc();
    let amount = Decimal::new(1999, 2);
    let fexu_decimal = encode_decimal(amount, 10, 2).unwrap();
    let doc = json!({"n": 1});
    let docb = json!({"ok": true});
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO types (
            id, big, label, flag, payload, doc, docb, uid, day, clock, stamp, stamptz, amount, fexu_decimal, missing
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
        [
            Value::Int(Some(1)),
            Value::BigInt(Some(9_000_000_000)),
            Value::String(Some(Box::new("alpha".to_owned()))),
            Value::Bool(Some(true)),
            Value::Bytes(Some(Box::new(vec![1, 2, 3]))),
            Value::Json(Some(Box::new(doc.clone()))),
            Value::Json(Some(Box::new(docb.clone()))),
            Value::Uuid(Some(Box::new(uid))),
            Value::ChronoDate(Some(Box::new(day))),
            Value::ChronoTime(Some(Box::new(clock))),
            Value::ChronoDateTime(Some(Box::new(stamp))),
            Value::ChronoDateTimeUtc(Some(Box::new(stamptz))),
            Value::Decimal(Some(Box::new(amount))),
            Value::Bytes(Some(Box::new(fexu_decimal.to_vec()))),
            Value::String(None),
        ],
    ))
    .await?;
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT id, big, label, flag, payload, doc, docb, uid, day, clock, stamp, stamptz, amount, fexu_decimal, missing FROM types",
        ))
        .await?
        .unwrap();
    assert_eq!(row.try_get::<i32>("", "id")?, 1);
    assert_eq!(row.try_get::<i64>("", "big")?, 9_000_000_000);
    assert_eq!(row.try_get::<String>("", "label")?, "alpha");
    assert!(row.try_get::<bool>("", "flag")?);
    assert_eq!(row.try_get::<Vec<u8>>("", "payload")?, vec![1, 2, 3]);
    assert_eq!(row.try_get::<Json>("", "doc")?, doc);
    assert_eq!(row.try_get::<Json>("", "docb")?, docb);
    assert_eq!(row.try_get::<Uuid>("", "uid")?, uid);
    assert_eq!(row.try_get::<NaiveDate>("", "day")?, day);
    assert_eq!(row.try_get::<NaiveTime>("", "clock")?, clock);
    assert_eq!(row.try_get::<chrono::NaiveDateTime>("", "stamp")?, stamp);
    assert_eq!(row.try_get::<DateTime<Utc>>("", "stamptz")?, stamptz);
    assert_eq!(row.try_get::<Decimal>("", "amount")?, amount);
    assert_eq!(row.try_get::<Vec<u8>>("", "fexu_decimal")?, fexu_decimal);
    assert_eq!(row.try_get::<Option<String>>("", "missing")?, None);

    let types = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT pg_typeof(amount)::text AS amount_type, pg_typeof(fexu_decimal)::text AS decimal_type FROM types",
        ))
        .await?
        .unwrap();
    assert_eq!(types.try_get::<String>("", "amount_type")?, "numeric");
    assert_eq!(types.try_get::<String>("", "decimal_type")?, "bytea");

    let transaction = db.begin().await?;
    transaction
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO types (id) VALUES ($1)",
            [Value::Int(Some(2))],
        ))
        .await?;
    transaction.rollback().await?;
    let count: i64 = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT COUNT(*)::bigint AS n FROM types",
        ))
        .await?
        .unwrap()
        .try_get("", "n")?;
    assert_eq!(count, 1);
    db.close().await?;
    Ok(())
}

async fn execute(db: &impl ConnectionTrait, sql: &str) {
    db.execute(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap();
}
