//! Admin-HTTP gegen SQLite: Staff-Zugang, CRUD, Whitelist und CSRF-Bootstrap.

use ferrum::admin::AdminRegistry;
use ferrum::auth::{AuthConfig, AuthService, AuthUser, ObjectPolicy};
use ferrum::core::FerrumModel;
use ferrum::db::ModelRepository;
use ferrum::prelude::*;
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "admin_categories",
    api = "/api/v1/admin_categories",
    admin = true,
    admin_label = "Kategorien",
    admin_permission = "categories"
)]
struct AdminCategory {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 100)]
    #[admin(list_display, searchable, sortable, label = "Name")]
    name: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "admin_products",
    api = "/api/v1/admin_products",
    admin = true,
    admin_label = "Produkte",
    admin_permission = "products"
)]
struct AdminProduct {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1, max_length = 150)]
    #[admin(list_display, searchable, sortable, label = "Name")]
    name: String,
    #[field(min = 0)]
    #[admin(list_display, sortable, label = "Preis")]
    price_cents: i64,
    #[field(foreign_key = "admin_categories.id", on_delete = "restrict")]
    #[admin(filterable, label = "Kategorie")]
    category_id: Option<Uuid>,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "admin_secrets",
    api = "/api/v1/admin_secrets",
    permissions = "secrets",
    object_policy = true,
    admin = true,
    admin_label = "Geheimnisse"
)]
struct AdminSecret {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required)]
    owner: String,
    #[field(required, min_length = 1)]
    #[admin(list_display, searchable, label = "Titel")]
    title: String,
}

#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(
    table = "admin_payroll",
    api = "/api/v1/admin_payroll",
    admin = true,
    admin_permission = "payroll"
)]
struct AdminPayroll {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1)]
    amount: String,
}

struct OwnerPolicy;

impl ObjectPolicy<AdminSecret> for OwnerPolicy {
    fn can_read(user: &AuthUser, object: &AdminSecret) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_update(user: &AuthUser, object: &AdminSecret) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_delete(user: &AuthUser, object: &AdminSecret) -> bool {
        object.owner == user.id.to_string()
    }
}

struct Reply {
    status: u16,
    headers: String,
    json: serde_json::Value,
}

fn http(port: u16, method: &str, path: &str, body: Option<&str>, extra: &str) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
    let data = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n{data}",
        data.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                response.extend_from_slice(&buffer[..count]);
                if complete(&response) {
                    break;
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                if complete(&response) {
                    break;
                }
                panic!("timeout: {error}");
            }
            Err(error) => panic!("read: {error}"),
        }
    }
    let text = String::from_utf8_lossy(&response);
    let (headers, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_ref(), ""));
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let payload = decode(headers, body.as_bytes());
    let json = serde_json::from_slice(&payload).unwrap_or(serde_json::Value::Null);
    Reply {
        status,
        headers: headers.to_owned(),
        json,
    }
}

fn complete(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let Some((headers, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    if headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return body.ends_with("0\r\n\r\n");
    }
    let length = headers.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .strip_prefix("content-length:")
            .map(|value| value.trim().parse::<usize>().unwrap_or(0))
    });
    length.is_some_and(|length| body.len() >= length)
}

fn decode(header: &str, body: &[u8]) -> Vec<u8> {
    if !header
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return body.to_vec();
    }
    let mut decoded = Vec::new();
    let mut rest = body;
    loop {
        let end = rest.windows(2).position(|bytes| bytes == b"\r\n").unwrap();
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
}

fn cookie(headers: &str) -> String {
    headers
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("set-cookie:"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| {
            value
                .trim()
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_owned()
        })
        .unwrap_or_default()
}

fn session(cookie: &str, csrf: &str) -> String {
    format!("Cookie: {cookie}\r\nx-csrf-token: {csrf}\r\n")
}

#[tokio::test]
async fn sqlite_admin_security_crud_and_csrf_bootstrap() {
    let meta = AdminProduct::admin_meta().expect("admin metadata");
    assert_eq!(meta.label, "Produkte");
    assert!(meta
        .fields
        .iter()
        .any(|field| field.name == "name" && field.searchable));
    assert!(meta
        .fields
        .iter()
        .any(|field| field.name == "id" && field.readonly && field.list_display));
    let registry = AdminRegistry::new()
        .add::<AdminCategory>()
        .add::<AdminProduct>()
        .add_policy::<AdminSecret, OwnerPolicy>()
        .add::<AdminPayroll>();
    assert!(registry.find("missing").is_none());

    let db = ferrum::db::connect("sqlite::memory:").await.unwrap();
    ferrum::auth::apply_migrations(&db).await.unwrap();
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let _staff = auth
        .create_user("staff@example.com", "correct-horse-battery", true, false)
        .await
        .unwrap();
    auth.create_user("plain@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    auth.create_user("root@example.com", "correct-horse-battery", true, true)
        .await
        .unwrap();
    let other_root = auth
        .create_user(
            "other-root@example.com",
            "correct-horse-battery",
            true,
            true,
        )
        .await
        .unwrap();
    auth.ensure_role("catalog").await.unwrap();
    for code in [
        "products.read",
        "products.create",
        "products.update",
        "products.delete",
        "categories.read",
        "secrets.read",
    ] {
        auth.ensure_permission(code).await.unwrap();
        auth.grant_permission("catalog", code).await.unwrap();
    }
    auth.assign_role("staff@example.com", "catalog")
        .await
        .unwrap();
    let app = ferrum::App::new(db.clone())
        .with_auth(auth.clone())
        .with_admin(registry)
        .register::<AdminCategory>()
        .register::<AdminProduct>()
        .register_policy::<AdminSecret, OwnerPolicy>()
        .register::<AdminPayroll>();
    app.bootstrap_schema().await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let call = |method: &'static str, path: String, body: Option<String>, extra: String| {
        tokio::task::spawn_blocking(move || http(port, method, &path, body.as_deref(), &extra))
    };

    let anonymous = call("GET", "/admin/api/models".into(), None, String::new())
        .await
        .unwrap();
    assert_eq!(anonymous.status, 401);

    let plain_login = call(
        "POST",
        "/auth/login".into(),
        Some(r#"{"email":"plain@example.com","password":"correct-horse-battery"}"#.into()),
        String::new(),
    )
    .await
    .unwrap();
    let plain_cookie = cookie(&plain_login.headers);
    let plain = call(
        "GET",
        "/admin/api/me".into(),
        None,
        format!("Cookie: {plain_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(plain.status, 403);

    let login = call(
        "POST",
        "/auth/login".into(),
        Some(r#"{"email":"staff@example.com","password":"correct-horse-battery"}"#.into()),
        String::new(),
    )
    .await
    .unwrap();
    assert_eq!(login.status, 200);
    let staff_cookie = cookie(&login.headers);
    let issued = login.json["csrf_token"].as_str().unwrap().to_owned();
    let refreshed = call(
        "GET",
        "/auth/csrf".into(),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(refreshed.status, 200);
    let csrf = refreshed.json["csrf_token"].as_str().unwrap().to_owned();
    assert_ne!(csrf, issued);
    let stale = call(
        "POST",
        "/admin/api/models/admin_categories/records".into(),
        Some(r#"{"name":"Alt"}"#.into()),
        session(&staff_cookie, &issued),
    )
    .await
    .unwrap();
    assert_eq!(stale.status, 403);
    let missing_csrf = call(
        "POST",
        "/admin/api/models/admin_categories/records".into(),
        Some(r#"{"name":"Ohne"}"#.into()),
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(missing_csrf.status, 403);

    let denied_create = call(
        "POST",
        "/admin/api/models/admin_categories/records".into(),
        Some(r#"{"name":"Buecher"}"#.into()),
        session(&staff_cookie, &csrf),
    )
    .await
    .unwrap();
    assert_eq!(denied_create.status, 403);
    let category = AdminCategory::create(
        &db,
        AdminCategoryCreate {
            name: "Buecher".into(),
        },
    )
    .await
    .unwrap();
    let category_id = category.id.to_string();
    let options = call(
        "GET",
        "/admin/api/models/admin_products/fields/category_id/options".into(),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(options.status, 200, "{}", options.json);
    assert_eq!(options.json["options"][0]["label"], "Buecher");

    for (name, price) in [("Alpha", 200), ("Beta", 100)] {
        let body =
            format!(r#"{{"name":"{name}","price_cents":{price},"category_id":"{category_id}"}}"#);
        let row = call(
            "POST",
            "/admin/api/models/admin_products/records".into(),
            Some(body),
            session(&staff_cookie, &csrf),
        )
        .await
        .unwrap();
        assert_eq!(row.status, 201, "{}", row.json);
    }
    let page = call(
        "GET",
        "/admin/api/models/admin_products/records?page=1&page_size=1&sort=price_cents&dir=desc"
            .into(),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(page.status, 200, "{}", page.json);
    assert_eq!(page.json["page_size"], 1);
    assert_eq!(page.json["has_more"], true);
    assert_eq!(page.json["data"][0]["name"], "Alpha");
    let found = call(
        "GET",
        "/admin/api/models/admin_products/records?q=Beta".into(),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(found.json["data"].as_array().unwrap().len(), 1);
    let bad_sort = call(
        "GET",
        "/admin/api/models/admin_products/records?sort=name;drop".into(),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(bad_sort.status, 422);
    let bad_filter = call(
        "GET",
        format!("/admin/api/models/admin_products/records?filter.password={category_id}"),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(bad_filter.status, 422);
    let product_id = page.json["data"][0]["id"].as_str().unwrap();
    let locked = call(
        "PATCH",
        format!("/admin/api/models/admin_products/records/{product_id}"),
        Some(format!(r#"{{"id":"{product_id}","name":"Neu"}}"#)),
        session(&staff_cookie, &csrf),
    )
    .await
    .unwrap();
    assert_eq!(locked.status, 422);
    let renamed = call(
        "PATCH",
        format!("/admin/api/models/admin_products/records/{product_id}"),
        Some(r#"{"name":"Alpha Neu"}"#.into()),
        session(&staff_cookie, &csrf),
    )
    .await
    .unwrap();
    assert_eq!(renamed.status, 200, "{}", renamed.json);
    let removed = call(
        "DELETE",
        format!("/admin/api/models/admin_products/records/{product_id}"),
        None,
        session(&staff_cookie, &csrf),
    )
    .await
    .unwrap();
    assert_eq!(removed.status, 204);

    let unknown = call(
        "GET",
        "/admin/api/models/not_registered/schema".into(),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(unknown.status, 404);
    let no_right = call(
        "GET",
        "/admin/api/models/admin_payroll/schema".into(),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(no_right.status, 403);
    let escalate = call(
        "POST",
        "/admin/api/directory/roles".into(),
        Some(r#"{"name":"owners"}"#.into()),
        session(&staff_cookie, &csrf),
    )
    .await
    .unwrap();
    assert_eq!(escalate.status, 403);

    let root_login = call(
        "POST",
        "/auth/login".into(),
        Some(r#"{"email":"root@example.com","password":"correct-horse-battery"}"#.into()),
        String::new(),
    )
    .await
    .unwrap();
    let root_cookie = cookie(&root_login.headers);
    let root_csrf = call(
        "GET",
        "/auth/csrf".into(),
        None,
        format!("Cookie: {root_cookie}\r\n"),
    )
    .await
    .unwrap()
    .json["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let blocked_list = call(
        "GET",
        "/admin/api/models/admin_secrets/records".into(),
        None,
        format!("Cookie: {root_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(blocked_list.status, 403);
    let secret = AdminSecret::create(
        &db,
        AdminSecretCreate {
            owner: "someone-else".into(),
            title: "Nur Besitzer".into(),
        },
    )
    .await
    .unwrap();
    let hidden = call(
        "GET",
        format!("/admin/api/models/admin_secrets/records/{}", secret.id),
        None,
        format!("Cookie: {staff_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(hidden.status, 404);
    let visible = call(
        "GET",
        format!("/admin/api/models/admin_secrets/records/{}", secret.id),
        None,
        format!("Cookie: {root_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(visible.status, 200, "{}", visible.json);
    let superuser_change = call(
        "POST",
        format!("/admin/api/directory/users/{}/staff", other_root.id),
        Some(r#"{"staff":false}"#.into()),
        session(&root_cookie, &root_csrf),
    )
    .await
    .unwrap();
    assert_eq!(superuser_change.status, 403);

    server.abort();
}

#[tokio::test]
async fn seed_file_database_when_requested() {
    let Ok(url) = std::env::var("FERRUM_E2E_DATABASE") else {
        return;
    };
    let db = ferrum::db::connect(&url).await.unwrap();
    ferrum::auth::apply_migrations(&db).await.unwrap();
    let auth = AuthService::new(db, AuthConfig::development());
    if auth.list_users().await.unwrap().is_empty() {
        auth.create_user("staff@example.com", "correct-horse-battery", true, true)
            .await
            .unwrap();
    }
}
