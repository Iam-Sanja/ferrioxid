//! OpenAPI document, in-process client, and isolated database helper.

use axum::http::StatusCode;
use ferrioxid::db::QueryCounter;
use ferrioxid::prelude::*;
use ferrioxid::testing::{assert_query_budget, csrf_token_from_html, TestClient, TestDatabase};
use ferrioxid::{FerrioxidChoice, OpenApiOperation, OpenApiOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FerrioxidChoice)]
enum OfferStatus {
    #[choice(value = "draft", de = "Entwurf", en = "Draft")]
    Draft,
    #[choice(value = "listed", de = "Gelistet", en = "Listed")]
    Listed,
}

#[derive(
    Debug, ferrioxid::FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize,
)]
#[ferrioxid(
    table = "openapi_offers",
    api = "/api/v1/offers",
    allow_anonymous = true,
    admin = true
)]
struct Offer {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 2, max_length = 40)]
    #[admin(filterable, sortable, label = "Name")]
    name: String,
    #[field(min = 0, max = 1_000)]
    stock: i64,
    #[field(precision = 10, scale = 2, min = 0)]
    price: Decimal,
    #[field(choice)]
    status: OfferStatus,
    #[field(foreign_key = "categories.id", on_delete = "restrict")]
    category_id: Option<Uuid>,
    #[field(readonly)]
    sku: String,
}

#[tokio::test]
async fn document_describes_models_and_hides_unregistered_routes() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    let app = ferrioxid::App::new(db).register::<Offer>();
    app.bootstrap_schema().await.unwrap();
    let router = app.into_router();
    let mut client = TestClient::new(router);
    let document = client
        .get("/openapi.json")
        .await
        .unwrap()
        .assert_status(StatusCode::OK)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(document["openapi"], "3.1.0");
    assert!(document["paths"].get("/admin").is_none());
    assert!(document["paths"].get("/admin/api").is_none());
    assert!(document["paths"].get("/missing").is_none());
    let collection = &document["paths"]["/api/v1/offers"];
    assert!(collection.get("get").is_some());
    assert!(collection.get("post").is_some());
    assert!(collection.get("put").is_none());
    let detail = &document["paths"]["/api/v1/offers/{id}"];
    assert!(detail.get("patch").is_some());
    assert!(detail.get("delete").is_some());
    assert!(detail.get("put").is_none());
    let page = &document["components"]["schemas"]["OfferPage"];
    assert_eq!(page["properties"]["limit"]["const"], 100);
    let response = &document["components"]["schemas"]["Offer"]["properties"];
    assert_eq!(response["name"]["minLength"], 2);
    assert_eq!(response["name"]["maxLength"], 40);
    assert_eq!(response["name"]["x-ferrioxid-filterable"], true);
    assert_eq!(response["stock"]["minimum"], 0);
    assert_eq!(response["stock"]["maximum"], 1000);
    assert_eq!(response["price"]["x-ferrioxid-decimal"]["precision"], 10);
    assert_eq!(response["price"]["x-ferrioxid-decimal"]["scale"], 2);
    assert_eq!(response["price"]["x-ferrioxid-minimum"], 0);
    assert_eq!(response["status"]["enum"][0], "draft");
    assert_eq!(response["status"]["enum"][1], "listed");
    assert_eq!(
        response["category_id"]["x-ferrioxid-relation"]["table"],
        "categories"
    );
    let create = &document["components"]["schemas"]["OfferCreate"];
    assert!(create["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "sku"));
    let update = &document["components"]["schemas"]["OfferUpdate"]["properties"];
    assert!(update.get("sku").is_none());
    assert!(update.get("id").is_none());
    let csrf = collection["post"]["description"].as_str().unwrap();
    assert!(csrf.contains("x-csrf-token"));
    assert!(document["components"]["securitySchemes"]["csrfHeader"]["name"] == "x-csrf-token");
    assert!(document["paths"].get("/healthz").is_some());
    assert!(document["paths"].get("/auth/login").is_some());

    let page = client
        .get("/docs")
        .await
        .unwrap()
        .assert_status(StatusCode::OK)
        .unwrap();
    let html = page.text().unwrap();
    assert!(html.contains("/openapi.json"));
    assert!(html.contains("Try a request"));

    let created = client
        .post_json(
            "/api/v1/offers",
            &serde_json::json!({
                "name": "Tasse",
                "stock": 2,
                "price": "4.50",
                "status": "listed",
                "sku": "TASSE"
            }),
        )
        .await
        .unwrap()
        .assert_status(StatusCode::CREATED)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(created["price"], "4.50");
    client.set_cookie("ferrioxid_anonymous", "not-a-session");
    let rejected = client
        .post_json(
            "/api/v1/offers",
            &serde_json::json!({
                "name": "Zweite",
                "stock": 1,
                "price": "1.00",
                "status": "draft",
                "sku": "ZWEI"
            }),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
    let problem = rejected.json().unwrap();
    assert_eq!(problem["status"], 403);
    assert!(problem.get("code").is_some());

    let counter = QueryCounter::install();
    QueryCounter::reset();
    client.clear_cookies();
    client
        .get("/api/v1/offers")
        .await
        .unwrap()
        .assert_status(StatusCode::OK)
        .unwrap();
    let listed = QueryCounter::get();
    assert_query_budget(listed, 2).unwrap();
    assert!(listed >= 1);
    for _ in 0..4 {
        client.get("/api/v1/offers").await.unwrap();
    }
    assert!(QueryCounter::get() > listed);
    drop(counter);
}

#[derive(
    Debug, ferrioxid::FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize,
)]
#[ferrioxid(table = "openapi_notes", api = "/api/v1/notes", allow_anonymous = true)]
struct Note {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(json)]
    body: String,
}

#[tokio::test]
async fn openapi_describes_canonical_json_documents() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    let app = ferrioxid::App::new(db).register::<Note>();
    app.bootstrap_schema().await.unwrap();
    let router = app.into_router();
    let mut client = TestClient::new(router);
    let document = client
        .get("/openapi.json")
        .await
        .unwrap()
        .assert_status(StatusCode::OK)
        .unwrap()
        .json()
        .unwrap();
    let body = &document["components"]["schemas"]["Note"]["properties"]["body"];
    assert_eq!(body["type"], "string");
    assert_eq!(body["contentMediaType"], "application/json");
    assert!(body["description"]
        .as_str()
        .unwrap()
        .contains("Canonical JSON text"));
    let created = client
        .post_json(
            "/api/v1/notes",
            &serde_json::json!({ "body": " { \"b\": 1, \"a\": null } " }),
        )
        .await
        .unwrap()
        .assert_status(StatusCode::CREATED)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(created["body"], "{\"a\":null,\"b\":1}");
    let rejected = client
        .post_json("/api/v1/notes", &serde_json::json!({ "body": "{" }))
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn unregistered_openapi_paths_are_rejected() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    let app = ferrioxid::App::new(db).document(OpenApiOperation {
        method: "GET".to_owned(),
        path: "/not-registered".to_owned(),
        operation_id: "get_missing".to_owned(),
        summary: "Missing".to_owned(),
        tag: "custom".to_owned(),
        request_schema: None,
        response_status: 200,
        response_schema: None,
        session: false,
        csrf: false,
    });
    let error = app.try_into_router().unwrap_err();
    assert_eq!(error.code, "openapi.path");
    assert!(error.detail.contains("not a registered route"));
}

#[test]
fn postgres_helper_refuses_a_foreign_database_name() {
    let refused = TestDatabase::postgres("postgres://ferrioxid:secret@127.0.0.1:5432/ferrioxid");
    assert!(refused.is_err());
    let accepted =
        TestDatabase::postgres("postgres://ferrioxid:secret@127.0.0.1:5432/ferrioxid_test");
    assert!(accepted.is_ok());
}

#[tokio::test]
async fn sqlite_helper_cleans_only_its_own_directory() {
    let mut database = TestDatabase::sqlite().await.unwrap();
    let url = database.url().to_owned();
    database.record_table("openapi_offers").unwrap();
    assert!(database.record_table("products;drop").is_err());
    database.cleanup().await.unwrap();
    let path = url
        .trim_start_matches("sqlite://")
        .trim_end_matches("?mode=rwc");
    assert!(!std::path::Path::new(path).exists());
}

#[test]
fn csrf_helper_reads_the_form_field() {
    let html = r#"<input type="hidden" name="csrf_token" value="abc-123">"#;
    assert_eq!(csrf_token_from_html(html).unwrap(), "abc-123");
}

#[test]
fn openapi_options_can_move_the_document() {
    let options = OpenApiOptions {
        enabled: true,
        json_path: "/shop/openapi.json".to_owned(),
        ui_path: None,
        title: "TestShop".to_owned(),
    };
    assert_eq!(options.json_path, "/shop/openapi.json");
    assert!(options.ui_path.is_none());
}
