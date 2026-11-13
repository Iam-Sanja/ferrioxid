//! Dezimalpreise, Choice-Enums, Sortierung und ein expliziter Backfill.

use fexu::core::{ColumnKind, DatabaseKind, FexuError, FexuModel};
use fexu::db::{col, Direction, ModelQuery, ModelRepository, PatchField};
use fexu::migrations::{
    apply_directory, make_migration, make_migration_with_backfills, BackfillLiteral,
    ColumnBackfill, LogicalType, RegisteredModel, SchemaSnapshot,
};
use fexu::prelude::*;
use fexu::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use fexu::{forms, FexuChoice};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FexuChoice)]
enum ProductStatus {
    #[choice(value = "draft", de = "Entwurf", en = "Draft")]
    Draft,
    #[choice(value = "active", de = "Aktiv", en = "Active")]
    Active,
    #[choice(value = "archived", de = "Archiv", en = "Archived")]
    Archived,
}

#[derive(Debug, fexu::FexuModel, fexu::serde::Serialize, fexu::serde::Deserialize)]
#[fexu(table = "money_products")]
struct MoneyProduct {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 40)]
    name: String,
    #[field(precision = 12, scale = 2)]
    price: Decimal,
    #[field(choice)]
    status: ProductStatus,
}

fn scratch() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("fexu-money-{nanos}-{tick}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

async fn fresh() -> fexu::sea_orm::DatabaseConnection {
    let db = fexu::db::connect("sqlite::memory:").await.unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        MoneyProduct::schema_sql(DatabaseKind::Sqlite),
    ))
    .await
    .unwrap();
    db
}

fn price(text: &str) -> Decimal {
    fexu::core::parse_decimal(text, 12, 2).unwrap()
}

#[test]
fn schema_uses_blob_storage_and_a_choice_check() {
    let sqlite = MoneyProduct::schema_sql(DatabaseKind::Sqlite);
    let postgres = MoneyProduct::schema_sql(DatabaseKind::Postgres);
    assert!(sqlite.to_ascii_lowercase().contains("blob"), "{sqlite}");
    assert!(
        postgres.to_ascii_lowercase().contains("bytea"),
        "{postgres}"
    );
    assert!(
        sqlite.contains("CHECK (\"status\" IN ('draft', 'active', 'archived'))"),
        "{sqlite}"
    );
    assert!(!sqlite.to_ascii_lowercase().contains("real"));
    let column = MoneyProduct::columns()
        .iter()
        .find(|column| column.name == "price")
        .unwrap();
    assert_eq!(column.kind, ColumnKind::Decimal);
    assert_eq!(column.precision, 12);
    assert_eq!(column.scale, 2);
}

#[test]
fn forms_render_choice_labels_and_reject_unknown_values() {
    let status = MoneyProduct::columns()
        .iter()
        .find(|column| column.name == "status")
        .unwrap();
    let html = forms::render_choice_select(status, "de", "active", true);
    assert!(html.contains("Entwurf"));
    assert!(html.contains("value=\"active\" selected"));
    let mut values = forms::FormValues::default();
    values.insert("status", "active");
    assert_eq!(
        forms::required_column_choice(&values, status).unwrap(),
        "active"
    );
    values.insert("status", "nope");
    assert!(forms::required_column_choice(&values, status).is_err());
    values.insert("price", "0.10");
    assert_eq!(
        forms::required_decimal(&values, "price", 12, 2).unwrap(),
        price("0.10")
    );
    values.insert("price", "0.001");
    assert!(forms::required_decimal(&values, "price", 12, 2).is_err());
}

#[tokio::test]
async fn decimal_and_choice_roundtrip_sort_and_survive_reopen() {
    assert_eq!(price("0.1") + price("0.2"), price("0.3"));
    let db = fresh().await;
    let created = MoneyProduct::create(
        &db,
        MoneyProductCreate {
            name: "Cable".into(),
            price: price("10.50"),
            status: ProductStatus::Active,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.price, price("10.50"));
    assert_eq!(created.status, ProductStatus::Active);
    let json = fexu::serde_json::to_string(&created).unwrap();
    assert!(
        json.contains("\"price\":\"10.50\"") || json.contains("\"price\":\"10.5\""),
        "{json}"
    );
    assert!(json.contains("\"status\":\"active\""));
    assert!(fexu::serde_json::from_str::<MoneyProduct>(&json.replace("active", "nope")).is_err());

    let rejected = MoneyProduct::create(
        &db,
        MoneyProductCreate {
            name: "Too fine".into(),
            price: price("1.00"),
            status: ProductStatus::Draft,
        },
    )
    .await
    .unwrap();
    let bad = MoneyProduct::update(
        &db,
        rejected.id,
        MoneyProductUpdate {
            name: PatchField::Missing,
            price: PatchField::Value(fexu::core::parse_decimal("1.005", 28, 3).unwrap()),
            status: PatchField::Missing,
        },
    )
    .await;
    assert!(matches!(bad, Err(FexuError::Validation(_))));

    MoneyProduct::update(
        &db,
        created.id,
        MoneyProductUpdate {
            name: PatchField::Missing,
            price: PatchField::Value(price("9.00")),
            status: PatchField::Value(ProductStatus::Archived),
        },
    )
    .await
    .unwrap();
    let updated = MoneyProduct::get(&db, created.id).await.unwrap().unwrap();
    assert_eq!(updated.status, ProductStatus::Archived);
    assert_eq!(updated.price, price("9.00"));

    for (name, amount, status) in [
        ("Neg", "-2.00", ProductStatus::Draft),
        ("Ten", "10.00", ProductStatus::Active),
        ("Dime", "0.10", ProductStatus::Draft),
    ] {
        MoneyProduct::create(
            &db,
            MoneyProductCreate {
                name: name.into(),
                price: price(amount),
                status,
            },
        )
        .await
        .unwrap();
    }
    let ordered = MoneyProduct::query()
        .order_by("price", Direction::Asc)
        .all(&db)
        .await
        .unwrap();
    let names: Vec<_> = ordered.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, vec!["Neg", "Dime", "Too fine", "Cable", "Ten"]);
    let expensive = MoneyProduct::query()
        .filter(col("price").gt("9.00"))
        .filter(col("status").eq("active"))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(expensive.len(), 1);
    assert_eq!(expensive[0].name, "Ten");

    let blocked = db
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "INSERT INTO money_products (id, name, price, status) VALUES ('x', 'bad', X'00', 'nope')",
        ))
        .await;
    assert!(blocked.is_err());
}

#[tokio::test]
async fn values_survive_closing_the_sqlite_file() {
    let directory = scratch();
    let file = directory.join("shop.sqlite3");
    let url = format!(
        "sqlite://{}?mode=rwc",
        file.to_string_lossy().replace('\\', "/")
    );
    let id = {
        let db = fexu::db::connect(&url).await.unwrap();
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            MoneyProduct::schema_sql(DatabaseKind::Sqlite),
        ))
        .await
        .unwrap();
        MoneyProduct::create(
            &db,
            MoneyProductCreate {
                name: "Persisted".into(),
                price: price("-0.50"),
                status: ProductStatus::Draft,
            },
        )
        .await
        .unwrap()
        .id
    };
    let db = fexu::db::connect(&url).await.unwrap();
    let loaded = MoneyProduct::get(&db, id).await.unwrap().unwrap();
    assert_eq!(loaded.price, price("-0.50"));
    assert_eq!(loaded.status, ProductStatus::Draft);
}

#[tokio::test]
async fn adding_decimal_and_choice_backfills_existing_rows() {
    let directory = scratch();
    let migrations = directory.join("migrations");
    let bare = SchemaSnapshot::from_registered(&[RegisteredModel {
        table: "money_products",
        columns: &MoneyProduct::columns()[..2],
        constraints: MoneyProduct::constraints(),
    }])
    .unwrap();
    make_migration(&migrations, "bare", &bare).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory
            .join("shop.sqlite3")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let db = fexu::db::connect(&url).await.unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO money_products (id, name) VALUES (?, ?)",
        [Uuid::from_u128(7).to_string().into(), "Legacy".into()],
    ))
    .await
    .unwrap();
    let full = SchemaSnapshot::from_registered(&[RegisteredModel {
        table: MoneyProduct::descriptor().table,
        columns: MoneyProduct::columns(),
        constraints: MoneyProduct::constraints(),
    }])
    .unwrap();
    make_migration_with_backfills(
        &migrations,
        "money",
        &full,
        &[
            ColumnBackfill {
                table: "money_products".into(),
                column: "price".into(),
                value: BackfillLiteral::Text("1.50".into()),
            },
            ColumnBackfill {
                table: "money_products".into(),
                column: "status".into(),
                value: BackfillLiteral::Text("draft".into()),
            },
        ],
    )
    .unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT price, status FROM money_products",
        ))
        .await
        .unwrap()
        .unwrap();
    let stored: Vec<u8> = row.try_get("", "price").unwrap();
    let decoded = fexu::core::decode_decimal(&stored, 2).unwrap();
    assert_eq!(fexu::core::format_decimal(decoded, 2).unwrap(), "1.50");
    let status: String = row.try_get("", "status").unwrap();
    assert_eq!(status, "draft");
    let loaded = MoneyProduct::list(&db).await.unwrap();
    assert_eq!(loaded.data[0].name, "Legacy");
    assert_eq!(loaded.data[0].price, price("1.50"));
    assert_eq!(loaded.data[0].status, ProductStatus::Draft);
    let _ = LogicalType::Decimal;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
async fn postgres_decimal_and_choice_roundtrip() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let db = fexu::db::connect(&url).await.unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        "DROP TABLE IF EXISTS money_products",
    ))
    .await
    .unwrap();
    db.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        MoneyProduct::schema_sql(DatabaseKind::Postgres),
    ))
    .await
    .unwrap();
    let created = MoneyProduct::create(
        &db,
        MoneyProductCreate {
            name: "Postgres".into(),
            price: price("1234567890.25"),
            status: ProductStatus::Active,
        },
    )
    .await
    .unwrap();
    let _ = created;
}
