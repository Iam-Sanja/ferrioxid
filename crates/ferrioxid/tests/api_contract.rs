//! Generated list, read serializer, and OpenAPI 3.1 contract.

use axum::http::{Method, StatusCode};
use ferrioxid::prelude::*;
use ferrioxid::testing::TestClient;
use serde_json::Value;

#[derive(
    Debug, ferrioxid::FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize,
)]
#[ferrioxid(
    table = "api_contract_items",
    api = "/api/v1/contract_items",
    allow_anonymous = true,
    admin = true
)]
struct ContractItem {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 40)]
    #[admin(filterable, sortable, label = "Name")]
    name: String,
    #[field(write_only, required, min_length = 8, max_length = 80)]
    #[admin(hidden)]
    secret: String,
}

async fn client_for(db: ferrioxid::sea_orm::DatabaseConnection) -> TestClient {
    let app = ferrioxid::App::new(db).register::<ContractItem>();
    app.bootstrap_schema().await.unwrap();
    TestClient::new(app.into_router())
}

async fn exercise(mut client: TestClient) {
    for name in ["Ada", "Bea", "Cara"] {
        let created = client
            .post_json(
                "/api/v1/contract_items",
                &serde_json::json!({"name": name, "secret": "supersecret"}),
            )
            .await
            .unwrap()
            .assert_status(StatusCode::CREATED)
            .unwrap()
            .json()
            .unwrap();
        assert!(created.get("secret").is_none(), "{created}");
        assert!(created["id"].is_string());
    }

    let mut cursor: Option<String> = None;
    let mut seen = Vec::new();
    for _ in 0..4 {
        let path = match &cursor {
            Some(token) => format!(
                "/api/v1/contract_items?limit=1&sort=name&cursor={}",
                encode_query(token)
            ),
            None => "/api/v1/contract_items?limit=1&sort=name".to_owned(),
        };
        let page = client
            .get(&path)
            .await
            .unwrap()
            .assert_status(StatusCode::OK)
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(page["limit"], 1);
        let rows = page["data"].as_array().unwrap();
        if rows.is_empty() {
            assert_eq!(page["has_more"], false);
            assert!(page["next"].is_null());
            break;
        }
        seen.push(rows[0]["name"].as_str().unwrap().to_owned());
        assert!(rows[0].get("secret").is_none());
        if page["has_more"] == true {
            cursor = Some(page["next"].as_str().unwrap().to_owned());
        } else {
            assert!(page["next"].is_null());
            break;
        }
    }
    assert_eq!(seen, ["Ada", "Bea", "Cara"]);

    let filtered = client
        .get("/api/v1/contract_items?filter=name:eq:Bea")
        .await
        .unwrap()
        .assert_status(StatusCode::OK)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(filtered["data"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["data"][0]["name"], "Bea");
    assert_eq!(filtered["has_more"], false);

    let hidden = client
        .get("/api/v1/contract_items?filter=secret:eq:supersecret")
        .await
        .unwrap()
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(hidden["code"], "list.filter");
    assert_eq!(hidden["status"], 422);

    let unsorted = client
        .get("/api/v1/contract_items?sort=secret")
        .await
        .unwrap()
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(unsorted["code"], "list.sort");

    let bad_cursor = client
        .get("/api/v1/contract_items?cursor=nope")
        .await
        .unwrap()
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(bad_cursor["code"], "list.cursor");

    let broken = client
        .request(Method::POST, "/api/v1/contract_items")
        .header("content-type", "application/json")
        .unwrap()
        .bytes(b"{".to_vec())
        .send()
        .await
        .unwrap()
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(broken["type"], "about:blank");
    assert_eq!(broken["code"], "validation.invalid_type");
    assert!(broken.get("title").is_some());

    let missing = client
        .get("/api/v1/contract_items/not-a-uuid")
        .await
        .unwrap()
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(missing["code"], "validation.uuid");

    let ready = client
        .get("/readyz")
        .await
        .unwrap()
        .assert_status(StatusCode::OK)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(ready["status"], "ready");

    let document = client
        .get("/openapi.json")
        .await
        .unwrap()
        .assert_status(StatusCode::OK)
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(document["openapi"], "3.1.0");
    assert_eq!(document["info"]["version"], "1");
    let page_schema = &document["components"]["schemas"]["ContractItemPage"];
    let sample = client
        .get("/api/v1/contract_items?limit=1&sort=name")
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_matches_object(page_schema, &sample);
    let item_schema = &document["components"]["schemas"]["ContractItem"];
    assert!(item_schema["properties"].get("secret").is_none());
    assert_matches_object(item_schema, &sample["data"][0]);
    let create = &document["components"]["schemas"]["ContractItemCreate"];
    assert_eq!(create["properties"]["secret"]["writeOnly"], true);
    let parameters = document["paths"]["/api/v1/contract_items"]["get"]["parameters"]
        .as_array()
        .unwrap();
    assert!(parameters.iter().any(|item| item["name"] == "cursor"));
    assert!(parameters.iter().any(|item| item["name"] == "filter"));
    assert!(document["paths"].get("/readyz").is_some());
    assert!(document["paths"].get("/admin").is_none());
}

#[tokio::test]
async fn cursor_filters_and_openapi_describe_the_same_list() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    exercise(client_for(db).await).await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL and creates only ferrioxid_test_* databases"]
async fn postgres_cursor_filters_and_openapi_describe_the_same_list() {
    let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let disposable = ferrioxid::db::DisposablePostgres::create(&admin)
        .await
        .unwrap();
    let url = disposable.url().to_owned();
    let db = ferrioxid::db::connect(&url).await.unwrap();
    exercise(client_for(db).await).await;
    disposable.drop_database().await.unwrap();
}

fn assert_matches_object(schema: &Value, body: &Value) {
    let properties = schema["properties"].as_object().unwrap();
    let object = body.as_object().expect("json object");
    for key in object.keys() {
        assert!(
            properties.contains_key(key),
            "{key} is not in the OpenAPI schema"
        );
    }
    for required in schema["required"].as_array().unwrap() {
        let name = required.as_str().unwrap();
        assert!(object.contains_key(name), "missing {name}");
    }
}

fn encode_query(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
