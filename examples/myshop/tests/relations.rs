//! Typisierte Belongs-To- und Has-Many-Beziehungen auf einer temporären SQLite-Datei.

use ferrioxid::core::FerrioxidModel;
use ferrioxid::db::ModelRepository;
use ferrioxid::migrations::{
    apply_directory, make_migration, RegisteredModel, SchemaSnapshot, FORMAT_VERSION,
};
use myshop::models::category::{Category, CategoryCreate};
use myshop::models::product::{Product, ProductCreate};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ferrioxid-relations-{nanos}-{tick}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[tokio::test]
async fn category_and_product_relations_are_typed() {
    let directory = temp_dir();
    let migrations = directory.join("migrations");
    let snapshot = SchemaSnapshot::from_registered(&[
        RegisteredModel {
            table: Category::descriptor().table,
            columns: Category::columns(),
            constraints: Category::constraints(),
        },
        RegisteredModel {
            table: Product::descriptor().table,
            columns: Product::columns(),
            constraints: Product::constraints(),
        },
    ])
    .unwrap();
    assert_eq!(snapshot.format_version, FORMAT_VERSION);
    make_migration(&migrations, "relations", &snapshot).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory
            .join("app.sqlite3")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let db = ferrioxid::db::connect(&url).await.unwrap();
    apply_directory(&db, &migrations, false).await.unwrap();

    let books = Category::create(
        &db,
        CategoryCreate {
            name: "Books".to_owned(),
        },
    )
    .await
    .unwrap();
    let duplicate = Category::create(
        &db,
        CategoryCreate {
            name: "Books".to_owned(),
        },
    )
    .await;
    assert!(matches!(
        duplicate,
        Err(ferrioxid::core::FerrioxidError::Validation(message)) if message.contains("unique")
    ));

    let laptop = Product::create(
        &db,
        ProductCreate {
            name: "Laptop".to_owned(),
            price_cents: 1000,
            active: None,
            category_id: Some(books.id),
        },
    )
    .await
    .unwrap();
    let orphan = Product::create(
        &db,
        ProductCreate {
            name: "Orphan".to_owned(),
            price_cents: 1,
            active: None,
            category_id: None,
        },
    )
    .await
    .unwrap();
    let invalid = Product::create(
        &db,
        ProductCreate {
            name: "Ghost".to_owned(),
            price_cents: 1,
            active: None,
            category_id: Some(ferrioxid::uuid::Uuid::nil()),
        },
    )
    .await;
    assert!(matches!(
        invalid,
        Err(ferrioxid::core::FerrioxidError::Validation(message)) if message.contains("foreign key")
    ));

    let loaded = laptop.category(&db).await.unwrap().unwrap();
    assert_eq!(loaded.id, books.id);
    assert_eq!(loaded.name, "Books");
    assert!(orphan.category(&db).await.unwrap().is_none());
    let products = books.products(&db).await.unwrap();
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].id, laptop.id);

    let empty = Category::create(
        &db,
        CategoryCreate {
            name: "Empty".to_owned(),
        },
    )
    .await
    .unwrap();
    assert!(empty.products(&db).await.unwrap().is_empty());

    let blocked = Category::delete(&db, books.id).await;
    assert!(matches!(
        blocked,
        Err(ferrioxid::core::FerrioxidError::Validation(message)) if message.contains("foreign key")
    ));
}

#[tokio::test]
async fn checked_in_migrations_add_the_category_foreign_key() {
    let directory = temp_dir();
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory
            .join("app.sqlite3")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let db = ferrioxid::db::connect(&url).await.unwrap();
    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let report = apply_directory(&db, &migrations, false).await.unwrap();
    assert_eq!(
        report.applied,
        vec![
            "0001_initial".to_owned(),
            "0002_categories_and_product_relations".to_owned(),
            "0003_internal_notes".to_owned(),
            "0004_events".to_owned(),
            "0005_orders".to_owned()
        ]
    );
    let category = Category::create(
        &db,
        CategoryCreate {
            name: "Tools".to_owned(),
        },
    )
    .await
    .unwrap();
    let product = Product::create(
        &db,
        ProductCreate {
            name: "Hammer".to_owned(),
            price_cents: 2500,
            active: Some(true),
            category_id: Some(category.id),
        },
    )
    .await
    .unwrap();
    assert_eq!(product.category(&db).await.unwrap().unwrap().name, "Tools");
    assert_eq!(category.products(&db).await.unwrap()[0].name, "Hammer");
}
