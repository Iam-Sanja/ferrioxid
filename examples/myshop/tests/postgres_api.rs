//! HTTP coverage for SQLite, plus an ignored PostgreSQL run.
//! SQLite: cargo test -p myshop --test postgres_api sqlite_http_roundtrip
//! PostgreSQL: cargo test -p myshop --test postgres_api -- --ignored

use ferrioxid::db::ModelRepository;
use ferrioxid::prelude::*;
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize, Debug)]
#[ferrioxid(
    table = "ferrioxid_v01_integration_products",
    api = "/api/v1/products",
    allow_anonymous = true
)]
struct TestProduct {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 3, max_length = 150)]
    name: String,
    #[field(min = 0)]
    price_cents: i64,
    #[field(default = true)]
    active: bool,
}

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "ferrioxid_v01_required_only",
    api = "/api/v1/required_only",
    allow_anonymous = true
)]
struct RequiredOnly {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required)]
    name: String,
}

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "ferrioxid_v01_private_products",
    api = "/api/v1/private_products"
)]
struct PrivateProduct {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required)]
    name: String,
}

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "ferrioxid_v01_notes",
    api = "/api/v1/notes",
    allow_anonymous = true
)]
struct NoteItem {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1)]
    title: String,
    note: Option<String>,
}

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(table = "ferrioxid_v01_locked", api = "/api/v1/locked")]
struct LockedItem {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(readonly, required, min_length = 1)]
    code: String,
    #[field(required)]
    name: String,
}

fn http_request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (u16, serde_json::Value) {
    let (status, payload) = http_exchange(port, method, path, body);
    if payload.is_empty() {
        return (status, serde_json::Value::Null);
    }
    (
        status,
        serde_json::from_slice(&payload).expect("JSON response"),
    )
}

fn http_message_complete(response: &[u8]) -> bool {
    let Some(header_end) = response.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
        return false;
    };
    let header = String::from_utf8_lossy(&response[..header_end]).to_ascii_lowercase();
    let body = &response[header_end + 4..];
    if header.contains("transfer-encoding: chunked") {
        return body.windows(5).any(|bytes| bytes == b"0\r\n\r\n");
    }
    let length = header
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok());
    match length {
        Some(length) => body.len() >= length,
        None => true,
    }
}

fn http_exchange(port: u16, method: &str, path: &str, body: Option<&str>) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to test API");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
    let data = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{data}",
        data.len(),
    );
    stream
        .write_all(request.as_bytes())
        .expect("write HTTP request");
    // Do not half-close the socket. On Windows that drops the response.
    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                response.extend_from_slice(&buffer[..count]);
                if http_message_complete(&response) {
                    break;
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                if http_message_complete(&response) {
                    break;
                }
                panic!("timed out reading HTTP response: {error}");
            }
            Err(error) => panic!("read HTTP response: {error}"),
        }
    }
    let header_end = response
        .windows(4)
        .position(|x| x == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("HTTP headers in {:?}", String::from_utf8_lossy(&response)));
    let header = std::str::from_utf8(&response[..header_end]).expect("valid HTTP headers");
    let status: u16 = header.split_whitespace().nth(1).unwrap().parse().unwrap();
    let body = &response[header_end + 4..];
    let payload = if header
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut decoded = Vec::new();
        let mut rest = body;
        loop {
            let end = rest.windows(2).position(|x| x == b"\r\n").unwrap();
            let length = usize::from_str_radix(
                std::str::from_utf8(&rest[..end])
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap(),
                16,
            )
            .unwrap();
            if length == 0 {
                break;
            }
            let start = end + 2;
            decoded.extend_from_slice(&rest[start..start + length]);
            rest = &rest[start + length + 2..];
        }
        decoded
    } else {
        body.to_vec()
    };
    (status, payload)
}

#[tokio::test]
async fn sqlite_http_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    run_http_roundtrip("sqlite::memory:").await
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a running local PostgreSQL database"]
async fn postgres_http_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run the ignored PostgreSQL integration test");
    run_http_roundtrip(&url).await
}

async fn run_http_roundtrip(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let db = ferrioxid::db::connect(url).await?;
    if url.starts_with("sqlite:") {
        use ferrioxid::sea_orm::ConnectionTrait;
        let row = db
            .query_one(ferrioxid::sea_orm::Statement::from_string(
                db.get_database_backend(),
                "PRAGMA foreign_keys",
            ))
            .await?
            .expect("pragma row");
        let enabled: i32 = row.try_get("", "foreign_keys")?;
        assert_eq!(enabled, 1);
    }
    // Dedicated test table. This test must run against a disposable development database.
    use ferrioxid::sea_orm::ConnectionTrait;
    db.execute(ferrioxid::sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrioxid_v01_integration_products\"",
    ))
    .await?;
    db.execute(ferrioxid::sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrioxid_v01_required_only\"",
    ))
    .await?;
    db.execute(ferrioxid::sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrioxid_v01_private_products\"",
    ))
    .await?;
    db.execute(ferrioxid::sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrioxid_v01_notes\"",
    ))
    .await?;
    let app = ferrioxid::App::new(db.clone())
        .register::<TestProduct>()
        .register::<RequiredOnly>()
        .register::<PrivateProduct>()
        .register::<NoteItem>();
    app.bootstrap_schema().await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let server = tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let path = "/api/v1/products";

    let (status, created) = tokio::task::spawn_blocking(move || {
        http_request(
            port,
            "POST",
            path,
            Some(r#"{"name":"Laptop","price_cents":1250}"#),
        )
    })
    .await?;
    assert_eq!(status, 201, "{created}");
    assert!(Uuid::parse_str(created["id"].as_str().unwrap()).is_ok());
    assert_eq!(created["name"], "Laptop");
    assert_eq!(created["price_cents"], 1250);
    assert_eq!(created["active"], true);

    let detail_path = format!("{path}/{}", created["id"].as_str().unwrap());
    let (status, detail) =
        tokio::task::spawn_blocking(move || http_request(port, "GET", &detail_path, None)).await?;
    assert_eq!(status, 200, "{detail}");
    assert_eq!(detail["id"], created["id"]);
    assert_eq!(detail["name"], "Laptop");
    assert_eq!(detail["active"], true);

    let missing = format!("{path}/{}", Uuid::new_v4());
    let (status, missing_body) =
        tokio::task::spawn_blocking(move || http_request(port, "GET", &missing, None)).await?;
    assert_eq!(status, 404, "{missing_body}");
    assert_eq!(missing_body["status"], 404);

    let (status, forbidden) = tokio::task::spawn_blocking(move || {
        http_request(port, "GET", "/api/v1/private_products", None)
    })
    .await?;
    assert_eq!(status, 403, "{forbidden}");
    assert_eq!(forbidden["status"], 403);
    let (status, forbidden_detail) = tokio::task::spawn_blocking(move || {
        http_request(
            port,
            "GET",
            "/api/v1/private_products/00000000-0000-0000-0000-000000000000",
            None,
        )
    })
    .await?;
    assert_eq!(status, 403, "{forbidden_detail}");
    assert_eq!(forbidden_detail["status"], 403);

    let (status, list) =
        tokio::task::spawn_blocking(move || http_request(port, "GET", path, None)).await?;
    assert_eq!(status, 200);
    assert_eq!(list["limit"], 100);
    assert_eq!(list["truncated"], false);
    assert_eq!(list["data"].as_array().unwrap().len(), 1);
    assert_eq!(list["data"][0]["id"], created["id"]);

    let (status, invalid) = tokio::task::spawn_blocking(move || {
        http_request(port, "POST", path, Some(r#"{"name":"","price_cents":-1}"#))
    })
    .await?;
    assert_eq!(status, 422, "{invalid}");
    let (status, blank) = tokio::task::spawn_blocking(move || {
        http_request(
            port,
            "POST",
            path,
            Some(r#"{"name":"   ","price_cents":1}"#),
        )
    })
    .await?;
    assert_eq!(status, 422, "{blank}");

    // This model has #[field(required)] WITHOUT min_length.
    let (status, blank) = tokio::task::spawn_blocking(move || {
        http_request(
            port,
            "POST",
            "/api/v1/required_only",
            Some(r#"{"name":"   "}"#),
        )
    })
    .await?;
    assert_eq!(status, 422, "{blank}");

    let product_id = created["id"].as_str().unwrap().to_owned();
    let patch_path = format!("{path}/{product_id}");
    let (status, patched) = tokio::task::spawn_blocking({
        let patch_path = patch_path.clone();
        move || http_request(port, "PATCH", &patch_path, Some(r#"{"name":"Notebook"}"#))
    })
    .await?;
    assert_eq!(status, 200, "{patched}");
    assert_eq!(patched["name"], "Notebook");
    assert_eq!(patched["price_cents"], 1250);
    assert_eq!(patched["active"], true);

    let (status, unchanged) = tokio::task::spawn_blocking({
        let patch_path = patch_path.clone();
        move || http_request(port, "PATCH", &patch_path, Some("{}"))
    })
    .await?;
    assert_eq!(status, 200, "{unchanged}");
    assert_eq!(unchanged["name"], "Notebook");
    assert_eq!(unchanged["price_cents"], 1250);
    assert_eq!(unchanged["active"], true);

    let (status, blank_name) = tokio::task::spawn_blocking({
        let patch_path = patch_path.clone();
        move || http_request(port, "PATCH", &patch_path, Some(r#"{"name":""}"#))
    })
    .await?;
    assert_eq!(status, 422, "{blank_name}");
    let (status, negative) = tokio::task::spawn_blocking({
        let patch_path = patch_path.clone();
        move || http_request(port, "PATCH", &patch_path, Some(r#"{"price_cents":-1}"#))
    })
    .await?;
    assert_eq!(status, 422, "{negative}");
    let unknown = format!("{path}/{}", Uuid::new_v4());
    let (status, missing_patch) = tokio::task::spawn_blocking(move || {
        http_request(port, "PATCH", &unknown, Some(r#"{"name":"Ghost"}"#))
    })
    .await?;
    assert_eq!(status, 404, "{missing_patch}");
    assert_eq!(missing_patch["status"], 404);

    let (status, disposable) = tokio::task::spawn_blocking(move || {
        http_request(
            port,
            "POST",
            path,
            Some(r#"{"name":"Disposable","price_cents":10}"#),
        )
    })
    .await?;
    assert_eq!(status, 201, "{disposable}");
    let disposable_id = disposable["id"].as_str().unwrap().to_owned();
    let delete_path = format!("{path}/{disposable_id}");
    let (status, delete_body) = tokio::task::spawn_blocking({
        let delete_path = delete_path.clone();
        move || http_exchange(port, "DELETE", &delete_path, None)
    })
    .await?;
    assert_eq!(status, 204);
    assert!(delete_body.is_empty(), "{delete_body:?}");
    let (status, gone) = tokio::task::spawn_blocking({
        let delete_path = delete_path.clone();
        move || http_request(port, "GET", &delete_path, None)
    })
    .await?;
    assert_eq!(status, 404, "{gone}");
    let (status, delete_again) = tokio::task::spawn_blocking({
        let delete_path = delete_path.clone();
        move || http_request(port, "DELETE", &delete_path, None)
    })
    .await?;
    assert_eq!(status, 404, "{delete_again}");
    assert_eq!(delete_again["status"], 404);

    let private_id = "00000000-0000-0000-0000-000000000000";
    let (status, forbidden_patch) = tokio::task::spawn_blocking(move || {
        http_request(
            port,
            "PATCH",
            &format!("/api/v1/private_products/{private_id}"),
            Some(r#"{"name":"nope"}"#),
        )
    })
    .await?;
    assert_eq!(status, 403, "{forbidden_patch}");
    assert_eq!(forbidden_patch["title"], "Forbidden");
    assert_eq!(forbidden_patch["status"], 403);
    assert_eq!(forbidden_patch["detail"], "Access denied");
    let (status, forbidden_delete) = tokio::task::spawn_blocking(move || {
        http_request(
            port,
            "DELETE",
            &format!("/api/v1/private_products/{private_id}"),
            None,
        )
    })
    .await?;
    assert_eq!(status, 403, "{forbidden_delete}");
    assert_eq!(forbidden_delete["title"], "Forbidden");
    assert_eq!(forbidden_delete["status"], 403);
    assert_eq!(forbidden_delete["detail"], "Access denied");

    let (status, note) = tokio::task::spawn_blocking(move || {
        http_request(port, "POST", "/api/v1/notes", Some(r#"{"title":"Desk"}"#))
    })
    .await?;
    assert_eq!(status, 201, "{note}");
    assert!(note["note"].is_null());
    let note_id = note["id"].as_str().unwrap().to_owned();
    let note_path = format!("/api/v1/notes/{note_id}");
    let (status, titled) = tokio::task::spawn_blocking({
        let note_path = note_path.clone();
        move || http_request(port, "PATCH", &note_path, Some(r#"{"title":"Table"}"#))
    })
    .await?;
    assert_eq!(status, 200, "{titled}");
    assert_eq!(titled["title"], "Table");
    assert!(titled["note"].is_null());
    let (status, noted) = tokio::task::spawn_blocking({
        let note_path = note_path.clone();
        move || http_request(port, "PATCH", &note_path, Some(r#"{"note":"hello"}"#))
    })
    .await?;
    assert_eq!(status, 200, "{noted}");
    assert_eq!(noted["title"], "Table");
    assert_eq!(noted["note"], "hello");
    let (status, cleared) = tokio::task::spawn_blocking({
        let note_path = note_path.clone();
        move || http_request(port, "PATCH", &note_path, Some(r#"{"note":null}"#))
    })
    .await?;
    assert_eq!(status, 200, "{cleared}");
    assert_eq!(cleared["title"], "Table");
    assert!(cleared["note"].is_null());
    let (status, null_title) = tokio::task::spawn_blocking({
        let note_path = note_path.clone();
        move || http_request(port, "PATCH", &note_path, Some(r#"{"title":null}"#))
    })
    .await?;
    assert_eq!(status, 422, "{null_title}");

    // Verify typed repository boundary and generated model field access.
    let typed = <TestProduct as ModelRepository>::list(&db).await?;
    assert_eq!(typed.data[0].name, "Notebook");
    assert_eq!(typed.data[0].price_cents, 1250);
    assert!(typed.data[0].active);
    assert_eq!(
        typed.data[0].id.to_string(),
        created["id"].as_str().unwrap()
    );

    // Populate 100 additional rows without the HTTP overhead.
    for i in 0..100 {
        <TestProduct as ModelRepository>::create(
            &db,
            TestProductCreate {
                name: format!("Product {i}"),
                price_cents: i,
                active: None,
            },
        )
        .await?;
    }
    let (status, list) =
        tokio::task::spawn_blocking(move || http_request(port, "GET", path, None)).await?;
    assert_eq!(status, 200);
    assert_eq!(list["limit"], 100);
    assert_eq!(list["truncated"], true);
    assert_eq!(list["data"].as_array().unwrap().len(), 100);

    server.abort();
    db.execute(ferrioxid::sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrioxid_v01_required_only\"",
    ))
    .await?;
    db.execute(ferrioxid::sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrioxid_v01_integration_products\"",
    ))
    .await?;
    db.execute(ferrioxid::sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrioxid_v01_private_products\"",
    ))
    .await?;
    db.execute(ferrioxid::sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS \"ferrioxid_v01_notes\"",
    ))
    .await?;
    Ok(())
}

fn column_definition(sql: &str, column: &str) -> String {
    let lower = sql.to_ascii_lowercase();
    let marker = format!("\"{column}\"");
    let start = lower
        .find(&marker)
        .unwrap_or_else(|| panic!("missing {column} in {sql}"));
    let after = &lower[start..];
    let end = after.find([',', ')']).unwrap_or(after.len());
    after[..end].to_owned()
}

#[test]
fn boolean_default_is_emitted_into_postgres_schema() {
    let sql = <TestProduct as ferrioxid::core::FerrioxidModel>::schema_sql(
        ferrioxid::core::DatabaseKind::Postgres,
    );
    let active = column_definition(&sql, "active");
    assert!(active.contains("not null"), "{active}");
    assert!(active.contains("true"), "{active}");
    let sqlite = <TestProduct as ferrioxid::core::FerrioxidModel>::schema_sql(
        ferrioxid::core::DatabaseKind::Sqlite,
    );
    let sqlite_active = column_definition(&sqlite, "active");
    assert!(sqlite_active.contains("integer"), "{sqlite_active}");
    assert!(sqlite_active.contains("default 1"), "{sqlite_active}");
}

#[test]
fn patch_dto_distinguishes_missing_null_value_and_rejects_readonly_fields() {
    use ferrioxid::db::PatchField;

    let missing: TestProductUpdate = serde_json::from_str("{}").unwrap();
    assert_eq!(missing.name, PatchField::Missing);
    assert_eq!(missing.price_cents, PatchField::Missing);
    assert_eq!(missing.active, PatchField::Missing);

    let nulls: TestProductUpdate =
        serde_json::from_str(r#"{"name":null,"price_cents":null,"active":null}"#).unwrap();
    assert_eq!(nulls.name, PatchField::Null);
    assert_eq!(nulls.price_cents, PatchField::Null);
    assert_eq!(nulls.active, PatchField::Null);

    let values: TestProductUpdate =
        serde_json::from_str(r#"{"name":"Desk","price_cents":5,"active":false}"#).unwrap();
    assert_eq!(values.name, PatchField::Value("Desk".to_owned()));
    assert_eq!(values.price_cents, PatchField::Value(5));
    assert_eq!(values.active, PatchField::Value(false));
    assert!(serde_json::from_str::<TestProductUpdate>(
        r#"{"id":"00000000-0000-0000-0000-000000000000"}"#
    )
    .is_err());

    let note: NoteItemUpdate = serde_json::from_str(r#"{"note":null}"#).unwrap();
    assert_eq!(note.note, PatchField::Null);
    assert_eq!(note.title, PatchField::Missing);
    let valued: NoteItemUpdate = serde_json::from_str(r#"{"note":"hello"}"#).unwrap();
    assert_eq!(valued.note, PatchField::Value("hello".to_owned()));
    let sql = <NoteItem as ferrioxid::core::FerrioxidModel>::schema_sql(
        ferrioxid::core::DatabaseKind::Postgres,
    );
    let note = column_definition(&sql, "note");
    assert!(note.contains("text"), "{note}");
    assert!(!note.contains("not null"), "{note}");

    let locked: LockedItemUpdate = serde_json::from_str(r#"{"name":"ok"}"#).unwrap();
    assert_eq!(locked.name, PatchField::Value("ok".to_owned()));
    assert!(serde_json::from_str::<LockedItemUpdate>(r#"{"code":"abc"}"#).is_err());
    let stored = LockedItem {
        id: Uuid::new_v4(),
        code: "abc".to_owned(),
        name: "ok".to_owned(),
    };
    assert_eq!(stored.code, "abc");
    assert_eq!(stored.name, "ok");
}
