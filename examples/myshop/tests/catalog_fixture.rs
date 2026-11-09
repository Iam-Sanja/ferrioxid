//! Reproducible category and product rows, including a failed import.

use ferrioxid::core::FerrioxidModel;
use ferrioxid::migrations::{
    apply_directory, dump_fixture, load_fixture, parse_fixture, LoadRequest, RegisteredModel,
    SchemaSnapshot,
};
use myshop::models::category::Category;
use myshop::models::product::Product;
use std::path::{Path, PathBuf};

struct TempDatabase {
    path: PathBuf,
}

impl TempDatabase {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ferrioxid-catalog-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        Self { path }
    }

    fn url(&self) -> String {
        format!(
            "sqlite://{}?mode=rwc",
            self.path.display().to_string().replace('\\', "/")
        )
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn catalog_schema() -> SchemaSnapshot {
    SchemaSnapshot::from_registered(&[
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
    .expect("catalog schema")
}

async fn load_catalog(db: &ferrioxid::sea_orm::DatabaseConnection, request: &LoadRequest) {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog.json"),
    )
    .expect("catalog fixture");
    let file = parse_fixture(&text).expect("catalog json");
    load_fixture(db, &catalog_schema(), &file, request)
        .await
        .expect("catalog load");
}

#[tokio::test]
async fn catalog_fixture_roundtrips_and_a_conflict_rolls_back() {
    let file = TempDatabase::new();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    {
        let db = ferrioxid::db::connect(&file.url()).await.unwrap();
        apply_directory(&db, &directory, false).await.unwrap();
        load_catalog(&db, &LoadRequest::default()).await;
        let duplicate = load_fixture(
            &db,
            &catalog_schema(),
            &parse_fixture(
                &std::fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog.json"),
                )
                .unwrap(),
            )
            .unwrap(),
            &LoadRequest::default(),
        )
        .await;
        assert!(duplicate.unwrap_err().0.contains("does not overwrite"));

        let mut upsert = LoadRequest::default();
        upsert.upsert = true;
        load_catalog(&db, &upsert).await;

        let conflict = parse_fixture(
            r#"{
              "ferrioxid": 1,
              "records": [
                {
                  "model": "categories",
                  "pk": "44444444-4444-4444-8444-444444444444",
                  "fields": { "name": "Temporary" }
                },
                {
                  "model": "products",
                  "pk": "11111111-1111-4111-8111-111111111111",
                  "fields": {
                    "name": "Should not replace",
                    "price_cents": 1,
                    "price": "0.01",
                    "status": "archived",
                    "active": false,
                    "category_id": "44444444-4444-4444-8444-444444444444"
                  }
                }
              ]
            }"#,
        )
        .unwrap();
        let error = load_fixture(&db, &catalog_schema(), &conflict, &LoadRequest::default())
            .await
            .unwrap_err();
        assert!(error.0.contains("does not overwrite"));
        let dumped = dump_fixture(&db, &catalog_schema(), None).await.unwrap();
        assert_eq!(
            dumped
                .records
                .iter()
                .map(|record| record.model.as_str())
                .collect::<Vec<_>>(),
            vec!["categories", "products", "products"]
        );
        assert_eq!(
            dumped.records[1]
                .fields
                .get("price")
                .and_then(|value| value.as_str()),
            Some("9.50")
        );
        assert!(dumped
            .records
            .iter()
            .all(|record| record.pk != "44444444-4444-4444-8444-444444444444"));
    }

    let db = ferrioxid::db::connect(&file.url()).await.unwrap();
    let dumped = dump_fixture(&db, &catalog_schema(), None).await.unwrap();
    assert_eq!(dumped.records.len(), 3);
    assert_eq!(
        dumped.records[1]
            .fields
            .get("name")
            .and_then(|value| value.as_str()),
        Some("Power cable")
    );
}
