use axum::body::Body;
use axum::http::StatusCode;
use ferrum::core::{FerrumModel, HasTimestamps};
use ferrum::migrations::{plan_change, RegisteredModel, SchemaSnapshot};
use ferrum::prelude::*;
use http_body_util::BodyExt;
use tower::ServiceExt;

ferrum::model_fields! {
    pub struct AuditStamp {
        #[field(managed = "created")]
        pub created_at: ferrum::chrono::DateTime<ferrum::chrono::Utc>,
        #[field(managed = "updated")]
        pub updated_at: ferrum::chrono::DateTime<ferrum::chrono::Utc>,
    }
}

ferrum_fields_AuditStamp! {
    #[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
    #[ferrum(
        table = "products",
        api = "/api/v1/products",
        allow_anonymous = true,
        admin = true,
        admin_label = "Produkte"
    )]
    struct Product {
        #[field(primary_key, auto)]
        pub id: Uuid,
        #[field(required, min_length = 1, format = "slug")]
        pub name: String,
        #[field(format = "email")]
        pub contact: Option<String>,
    }
}

ferrum::with_timestamps! {
    #[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
    #[ferrum(
        table = "categories",
        api = "/api/v1/categories",
        allow_anonymous = true,
        admin = true,
        admin_label = "Kategorien"
    )]
    struct Category {
        #[field(primary_key, auto)]
        pub id: Uuid,
        #[field(required, min_length = 1)]
        pub title: String,
    }
}

#[tokio::test]
async fn two_models_share_flat_timestamps_and_reject_client_values() {
    let created = Product::columns()
        .iter()
        .map(|column| column.name)
        .collect::<Vec<_>>();
    assert!(created.contains(&"created_at"));
    assert!(created.contains(&"updated_at"));
    assert!(created.contains(&"name"));
    assert!(!created.contains(&"audit_stamp"));
    let category = Category::columns()
        .iter()
        .map(|column| column.name)
        .collect::<Vec<_>>();
    assert_eq!(
        category
            .iter()
            .filter(|name| **name == "created_at")
            .count(),
        1
    );
    assert_ne!(Product::descriptor().table, Category::descriptor().table);
    let created_field = Product::admin_meta()
        .unwrap()
        .fields
        .iter()
        .find(|field| field.name == "created_at")
        .unwrap();
    assert!(created_field.readonly);
    assert!(!created_field.hidden);
    let snapshot = SchemaSnapshot::from_registered(&[
        RegisteredModel {
            table: Product::descriptor().table,
            columns: Product::columns(),
            constraints: Product::constraints(),
        },
        RegisteredModel {
            table: Category::descriptor().table,
            columns: Category::columns(),
            constraints: Category::constraints(),
        },
    ])
    .unwrap();
    assert!(snapshot
        .tables
        .iter()
        .all(|table| table.name != "audit_stamp"));
    let plan = plan_change(&SchemaSnapshot::empty(), &snapshot).unwrap();
    let rendered = format!("{plan:?}");
    assert!(rendered.contains("products"));
    assert!(rendered.contains("categories"));
    assert!(rendered.contains("created_at"));

    let first = ferrum::chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let second = ferrum::chrono::DateTime::parse_from_rfc3339("2026-01-02T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    ferrum::core::set_test_clock(Some(first));
    let db = ferrum::db::connect("sqlite::memory:").await.unwrap();
    let app = ferrum::App::new(db)
        .register::<Product>()
        .register::<Category>();
    app.bootstrap_schema().await.unwrap();
    let rejected = ferrum::serde_json::from_str::<ProductCreate>(
        r#"{"name":"red-chair","created_at":"2020-01-01T00:00:00Z"}"#,
    );
    assert!(rejected.is_err());
    let router = app.into_router();
    let invalid = router
        .clone()
        .oneshot(
            axum::http::Request::post("/api/v1/products")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Red Chair"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let response = router
        .clone()
        .oneshot(
            axum::http::Request::post("/api/v1/products")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"red-chair","contact":"owner@example.com"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let created_row: Product = ferrum::serde_json::from_slice(&body).unwrap();
    assert_eq!(created_row.created_at(), first);
    assert_eq!(created_row.updated_at(), first);
    assert!(!body.windows(12).any(|window| window == b"\"timestamps\""));
    ferrum::core::set_test_clock(Some(second));
    let patched = router
        .oneshot(
            axum::http::Request::patch(format!("/api/v1/products/{}", created_row.id))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"blue-chair"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patched.status(), StatusCode::OK);
    let body = patched.into_body().collect().await.unwrap().to_bytes();
    let updated: Product = ferrum::serde_json::from_slice(&body).unwrap();
    assert_eq!(updated.created_at(), first);
    assert_eq!(updated.updated_at(), second);
    ferrum::core::set_test_clock(None);
}
