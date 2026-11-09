//! `seed-shop` can run twice and a rejected file leaves the previous rows in place.
//!
//! Bad fixtures are unique files under the system temp directory. The test does
//! not create or delete `examples/myshop/fixtures/bad-seed.json`.

use ferrioxid::migrations::discover_project;
use ferrioxid::sea_orm::ConnectionTrait;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn unique_path(prefix: &str, extension: &str) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}-{sequence}.{extension}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn database() -> (TempFile, String) {
    let path = unique_path("ferrioxid-seed", "sqlite3");
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.display().to_string().replace('\\', "/")
    );
    (TempFile(path), url)
}

fn temp_fixture(body: &str) -> TempFile {
    let path = unique_path("ferrioxid-fixture", "json");
    std::fs::write(&path, body).expect("temp fixture");
    TempFile(path)
}

fn shared_source_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/bad-seed.json")
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
    let shared = shared_source_fixture();
    let shared_before = shared.exists();
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

    let bad = temp_fixture(
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
    );
    let failed = myshop::management::execute_with(
        &project,
        &[
            "seed-shop".into(),
            "--insert".into(),
            "--file".into(),
            bad.0.display().to_string(),
            "--database-url".into(),
            url.clone(),
        ],
    )
    .await;
    assert_eq!(failed, 1);
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
    drop(bad);
    assert_eq!(shared.exists(), shared_before);
}

#[tokio::test]
async fn seed_shop_rejects_escaped_paths_without_opening_the_database() {
    let project = discover_project(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("myshop project");
    let shared = shared_source_fixture();
    let shared_before = shared.exists();
    let (database_file, url) = database();
    let outside = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Cargo.toml")
        .canonicalize()
        .expect("workspace Cargo.toml");
    for path in ["../secret.json", outside.to_str().unwrap()] {
        let code = myshop::management::execute_with(
            &project,
            &[
                "seed-shop".into(),
                "--file".into(),
                path.to_owned(),
                "--database-url".into(),
                url.clone(),
            ],
        )
        .await;
        assert_eq!(code, 2, "{path}");
    }
    assert!(!database_file.0.exists());
    assert_eq!(shared.exists(), shared_before);

    let first = temp_fixture("{\"ferrioxid\":1,\"records\":[]}");
    let second = temp_fixture("{\"ferrioxid\":1,\"records\":[]}");
    assert_ne!(first.0, second.0);
    assert!(first.0.starts_with(std::env::temp_dir()));
    assert!(second.0.starts_with(std::env::temp_dir()));
}
