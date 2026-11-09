//! `seed-shop` can run twice and a rejected file leaves the previous rows in place.

use ferrioxid::migrations::discover_project;
use ferrioxid::sea_orm::ConnectionTrait;
use std::path::{Path, PathBuf};

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn database() -> (TempFile, String) {
    let path = std::env::temp_dir().join(format!(
        "ferrioxid-seed-{}-{}.sqlite3",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.display().to_string().replace('\\', "/")
    );
    (TempFile(path), url)
}

async fn product_count(url: &str) -> usize {
    let db = ferrioxid::db::connect(url).await.unwrap();
    db.query_all(ferrioxid::sea_orm::Statement::from_string(
        ferrioxid::sea_orm::DatabaseBackend::Sqlite,
        "SELECT id FROM products",
    ))
    .await
    .unwrap()
    .len()
}

#[tokio::test]
async fn seed_shop_is_idempotent_and_a_bad_file_rolls_back() {
    let project = discover_project(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("myshop project");
    assert_eq!(
        myshop::management::execute_with(&project, &["missing-command".into()]).await,
        2
    );
    let (file, url) = database();
    let first = myshop::management::execute_with(
        &project,
        &["seed-shop".into(), "--database-url".into(), url.clone()],
    )
    .await;
    assert_eq!(first, 0);
    let second = myshop::management::execute_with(
        &project,
        &["seed-shop".into(), "--database-url".into(), url.clone()],
    )
    .await;
    assert_eq!(second, 0);
    assert_eq!(product_count(&url).await, 2);

    let bad_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/bad-seed.json");
    let _bad = TempFile(bad_path.clone());
    std::fs::write(
        &bad_path,
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
                "name": "Should roll back",
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
    let failed = myshop::management::execute_with(
        &project,
        &[
            "seed-shop".into(),
            "--insert".into(),
            "--file".into(),
            "fixtures/bad-seed.json".into(),
            "--database-url".into(),
            url.clone(),
        ],
    )
    .await;
    assert_eq!(failed, 1);
    drop(ferrioxid::db::connect(&url).await.unwrap());
    assert_eq!(product_count(&url).await, 2);
    let reopened = ferrioxid::db::connect(&url).await.unwrap();
    let names = reopened
        .query_all(ferrioxid::sea_orm::Statement::from_string(
            ferrioxid::sea_orm::DatabaseBackend::Sqlite,
            "SELECT name FROM categories ORDER BY name",
        ))
        .await
        .unwrap();
    assert_eq!(names.len(), 1);
    drop(reopened);
    drop(file);
}
