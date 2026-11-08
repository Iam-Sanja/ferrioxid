//! HTTP-Prüfung für Login, Sitzungen, CSRF und Modellrechte. Nur SQLite.

use ferrioxid::auth::{AuthConfig, AuthService, AuthUser, ObjectPolicy};
use ferrioxid::core::{QueryScope, ScopeValue};
use ferrioxid::db::ModelRepository;
use ferrioxid::prelude::*;
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "auth_demo_products",
    api = "/api/v1/auth_demo_products",
    allow_anonymous = true
)]
struct DemoProduct {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1)]
    name: String,
}

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "auth_internal_notes",
    api = "/api/v1/auth_internal_notes",
    permissions = "notes"
)]
struct ProtectedNote {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required, min_length = 1)]
    title: String,
}

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "auth_secrets",
    api = "/api/v1/auth_secrets",
    permissions = "secrets",
    object_policy = true
)]
struct SecretNote {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required)]
    owner: String,
    #[field(required, min_length = 1)]
    title: String,
}

#[derive(FerrioxidModel, ferrioxid::serde::Serialize, ferrioxid::serde::Deserialize)]
#[ferrioxid(
    table = "auth_unbound_secrets",
    api = "/api/v1/auth_unbound_secrets",
    permissions = "secrets",
    object_policy = true
)]
struct UnboundSecret {
    #[field(primary_key, auto)]
    id: Uuid,
    #[field(required)]
    title: String,
}

struct OwnerPolicy;

impl ObjectPolicy<SecretNote> for OwnerPolicy {
    fn can_read(user: &AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }

    fn can_update(user: &AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }

    fn can_delete(user: &AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }

    fn read_scope(user: &AuthUser) -> QueryScope {
        QueryScope::Equals {
            column: "owner",
            value: ScopeValue::Text(user.id.to_string()),
        }
    }

    fn write_scope(user: &AuthUser) -> QueryScope {
        Self::read_scope(user)
    }

    fn can_create(user: &AuthUser, input: &SecretNoteCreate) -> bool {
        input.owner == user.id.to_string()
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
    let header_end = response
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .expect("headers");
    let headers = String::from_utf8_lossy(&response[..header_end]).to_string();
    let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
    let payload = decode_body(&headers, &response[header_end + 4..]);
    let json = if payload.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&payload).unwrap_or(serde_json::Value::Null)
    };
    Reply {
        status,
        headers,
        json,
    }
}

fn complete(response: &[u8]) -> bool {
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

fn decode_body(header: &str, body: &[u8]) -> Vec<u8> {
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

fn session_cookie(headers: &str) -> String {
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

fn auth_headers(cookie: &str, csrf: &str, origin: Option<&str>) -> String {
    let mut lines = format!("Cookie: {cookie}\r\nx-csrf-token: {csrf}\r\n");
    if let Some(origin) = origin {
        lines.push_str(&format!("Origin: {origin}\r\n"));
    }
    lines
}

#[tokio::test]
async fn sqlite_auth_http_permissions_and_csrf() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    ferrioxid::auth::apply_migrations(&db).await.unwrap();
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let editor = auth
        .create_user("editor@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    auth.create_user("root@example.com", "superuser-password", true, true)
        .await
        .unwrap();
    let app = ferrioxid::App::new(db.clone())
        .with_auth(auth.clone())
        .register::<DemoProduct>()
        .register::<ProtectedNote>()
        .register_policy::<SecretNote, OwnerPolicy>()
        .register::<UnboundSecret>();
    app.bootstrap_schema().await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, app.into_router()).await });

    let call = |method: &'static str, path: String, body: Option<String>, extra: String| {
        tokio::task::spawn_blocking(move || http(port, method, &path, body.as_deref(), &extra))
    };

    let denied = call(
        "GET",
        "/api/v1/auth_internal_notes".into(),
        None,
        String::new(),
    )
    .await
    .unwrap();
    assert_eq!(denied.status, 401);
    assert_eq!(denied.json["detail"], "Authentication required");

    let demo = call(
        "POST",
        "/api/v1/auth_demo_products".into(),
        Some(r#"{"name":"Public"}"#.into()),
        String::new(),
    )
    .await
    .unwrap();
    assert_eq!(demo.status, 201, "{}", demo.json);

    let unknown = call(
        "POST",
        "/auth/login".into(),
        Some(r#"{"email":"missing@example.com","password":"correct-horse-battery"}"#.into()),
        String::new(),
    )
    .await
    .unwrap();
    let wrong = call(
        "POST",
        "/auth/login".into(),
        Some(r#"{"email":"editor@example.com","password":"wrong-password-1"}"#.into()),
        String::new(),
    )
    .await
    .unwrap();
    assert_eq!(unknown.status, 401);
    assert_eq!(wrong.status, 401);
    assert_eq!(unknown.json["detail"], wrong.json["detail"]);

    let login = call(
        "POST",
        "/auth/login".into(),
        Some(r#"{"email":"editor@example.com","password":"correct-horse-battery"}"#.into()),
        "Origin: http://127.0.0.1:3000\r\n".into(),
    )
    .await
    .unwrap();
    assert_eq!(login.status, 200, "{}", login.json);
    let raw = login.json.to_string();
    assert!(!raw.contains("password_hash"));
    assert!(!raw.contains("argon2"));
    let cookie = session_cookie(&login.headers);
    assert!(cookie.starts_with("ferrum_session="));
    assert!(!raw.contains(cookie.split('=').nth(1).unwrap_or("missing-token")));
    let csrf = login.json["csrf_token"].as_str().unwrap().to_owned();
    let me = call(
        "GET",
        "/auth/me".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(me.status, 200, "{}", me.json);
    assert_eq!(me.json["email"], "editor@example.com");
    assert!(me.json.get("password_hash").is_none());
    assert!(me.json.get("is_staff").is_some());

    let no_right = call(
        "GET",
        "/api/v1/auth_internal_notes".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(no_right.status, 403);

    auth.grant_permission("editor", "notes.read").await.unwrap();
    auth.assign_role(&editor.email, "editor").await.unwrap();
    let listed = call(
        "GET",
        "/api/v1/auth_internal_notes".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(listed.status, 200, "{}", listed.json);

    let missing_csrf = call(
        "POST",
        "/api/v1/auth_internal_notes".into(),
        Some(r#"{"title":"Draft"}"#.into()),
        format!("Cookie: {cookie}\r\nOrigin: http://127.0.0.1:3000\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(missing_csrf.status, 403);
    assert!(missing_csrf.json["detail"]
        .as_str()
        .unwrap()
        .contains("csrf"));

    auth.grant_permission("editor", "notes.create")
        .await
        .unwrap();
    let bad_token = call(
        "POST",
        "/api/v1/auth_internal_notes".into(),
        Some(r#"{"title":"Draft"}"#.into()),
        auth_headers(&cookie, "not-the-token", Some("http://127.0.0.1:3000")),
    )
    .await
    .unwrap();
    assert_eq!(bad_token.status, 403);
    let bad_origin = call(
        "POST",
        "/api/v1/auth_internal_notes".into(),
        Some(r#"{"title":"Draft"}"#.into()),
        auth_headers(&cookie, &csrf, Some("http://evil.example")),
    )
    .await
    .unwrap();
    assert_eq!(bad_origin.status, 403);
    assert!(bad_origin.json["detail"]
        .as_str()
        .unwrap()
        .contains("origin"));
    let created = call(
        "POST",
        "/api/v1/auth_internal_notes".into(),
        Some(r#"{"title":"Draft"}"#.into()),
        auth_headers(&cookie, &csrf, Some("http://127.0.0.1:3000")),
    )
    .await
    .unwrap();
    assert_eq!(created.status, 201, "{}", created.json);
    let note_id = created.json["id"].as_str().unwrap().to_owned();

    let update_denied = call(
        "PATCH",
        format!("/api/v1/auth_internal_notes/{note_id}"),
        Some(r#"{"title":"Final"}"#.into()),
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(update_denied.status, 403);
    auth.grant_permission("editor", "notes.update")
        .await
        .unwrap();
    let updated = call(
        "PATCH",
        format!("/api/v1/auth_internal_notes/{note_id}"),
        Some(r#"{"title":"Final"}"#.into()),
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(updated.status, 200, "{}", updated.json);
    assert_eq!(updated.json["title"], "Final");

    let delete_denied = call(
        "DELETE",
        format!("/api/v1/auth_internal_notes/{note_id}"),
        None,
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(delete_denied.status, 403);
    auth.grant_permission("editor", "notes.delete")
        .await
        .unwrap();
    let deleted = call(
        "DELETE",
        format!("/api/v1/auth_internal_notes/{note_id}"),
        None,
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(deleted.status, 204);

    auth.revoke_permission("editor", "notes.read")
        .await
        .unwrap();
    let revoked = call(
        "GET",
        "/api/v1/auth_internal_notes".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(revoked.status, 403);

    let root_login = call(
        "POST",
        "/auth/login".into(),
        Some(r#"{"email":"root@example.com","password":"superuser-password"}"#.into()),
        String::new(),
    )
    .await
    .unwrap();
    assert_eq!(root_login.status, 200, "{}", root_login.json);
    let root_cookie = session_cookie(&root_login.headers);
    let root_list = call(
        "GET",
        "/api/v1/auth_internal_notes".into(),
        None,
        format!("Cookie: {root_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(root_list.status, 200, "{}", root_list.json);

    SecretNote::create(
        &db,
        SecretNoteCreate {
            owner: editor.id.to_string(),
            title: "Mine".to_owned(),
        },
    )
    .await
    .unwrap();
    let foreign = SecretNote::create(
        &db,
        SecretNoteCreate {
            owner: "someone-else".to_owned(),
            title: "Theirs".to_owned(),
        },
    )
    .await
    .unwrap();
    auth.grant_permission("editor", "secrets.read")
        .await
        .unwrap();
    let own = SecretNote::list(&db).await.unwrap();
    let mine = own
        .data
        .iter()
        .find(|note| note.owner == editor.id.to_string())
        .unwrap();
    let allowed = call(
        "GET",
        format!("/api/v1/auth_secrets/{}", mine.id),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(allowed.status, 200, "{}", allowed.json);
    let hidden = call(
        "GET",
        format!("/api/v1/auth_secrets/{}", foreign.id),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(hidden.status, 404, "{}", hidden.json);
    let blocked_list = call(
        "GET",
        "/api/v1/auth_secrets".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(blocked_list.status, 200, "{}", blocked_list.json);
    let visible_rows = blocked_list.json["data"].as_array().unwrap();
    assert_eq!(visible_rows.len(), 1);
    assert_eq!(visible_rows[0]["title"], "Mine");
    auth.grant_permission("editor", "secrets.create")
        .await
        .unwrap();
    let own_create = call(
        "POST",
        "/api/v1/auth_secrets".into(),
        Some(format!(r#"{{"owner":"{}","title":"Neu"}}"#, editor.id)),
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(own_create.status, 201, "{}", own_create.json);
    let foreign_create = call(
        "POST",
        "/api/v1/auth_secrets".into(),
        Some(r#"{"owner":"someone-else","title":"Fremd"}"#.into()),
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(foreign_create.status, 403, "{}", foreign_create.json);
    let missing_owner = call(
        "POST",
        "/api/v1/auth_secrets".into(),
        Some(r#"{"title":"Ohne"}"#.into()),
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(missing_owner.status, 422, "{}", missing_owner.json);
    let root_csrf = root_login.json["csrf_token"].as_str().unwrap().to_owned();
    let super_create = call(
        "POST",
        "/api/v1/auth_secrets".into(),
        Some(r#"{"owner":"someone-else","title":"Super"}"#.into()),
        auth_headers(&root_cookie, &root_csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(super_create.status, 201, "{}", super_create.json);
    let super_list = call(
        "GET",
        "/api/v1/auth_secrets".into(),
        None,
        format!("Cookie: {root_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(super_list.status, 200, "{}", super_list.json);
    let unbound = call(
        "GET",
        "/api/v1/auth_unbound_secrets".into(),
        None,
        format!("Cookie: {root_cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(unbound.status, 403);
    assert!(unbound.json["detail"]
        .as_str()
        .unwrap()
        .contains("not bound"));

    let changed = call(
        "POST",
        "/auth/change-password".into(),
        Some(
            r#"{"current_password":"correct-horse-battery","new_password":"updated-password-1"}"#
                .into(),
        ),
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(changed.status, 200, "{}", changed.json);
    let stale = call(
        "GET",
        "/auth/me".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(stale.status, 401);

    let relogin = call(
        "POST",
        "/auth/login".into(),
        Some(r#"{"email":"editor@example.com","password":"updated-password-1"}"#.into()),
        String::new(),
    )
    .await
    .unwrap();
    assert_eq!(relogin.status, 200, "{}", relogin.json);
    let cookie = session_cookie(&relogin.headers);
    let csrf = relogin.json["csrf_token"].as_str().unwrap().to_owned();
    let logout_blocked = call(
        "POST",
        "/auth/logout".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(logout_blocked.status, 403);
    let still = call(
        "GET",
        "/auth/me".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(still.status, 200, "{}", still.json);
    let logged_out = call(
        "POST",
        "/auth/logout".into(),
        None,
        auth_headers(&cookie, &csrf, None),
    )
    .await
    .unwrap();
    assert_eq!(logged_out.status, 204);
    let after = call(
        "GET",
        "/auth/me".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(after.status, 401);
    let csrf_after_logout = call(
        "GET",
        "/auth/csrf".into(),
        None,
        format!("Cookie: {cookie}\r\n"),
    )
    .await
    .unwrap();
    assert_eq!(csrf_after_logout.status, 401);
    let again = call("POST", "/auth/logout".into(), None, String::new())
        .await
        .unwrap();
    assert_eq!(again.status, 204);
    let missing = call("GET", "/auth/me".into(), None, String::new())
        .await
        .unwrap();
    assert_eq!(missing.status, 401);
    let tampered = call(
        "GET",
        "/auth/me".into(),
        None,
        "Cookie: ferrum_session=tampered\r\n".into(),
    )
    .await
    .unwrap();
    assert_eq!(tampered.status, 401);

    server.abort();
}

#[tokio::test]
async fn expired_session_cookie_is_rejected() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    ferrioxid::auth::apply_migrations(&db).await.unwrap();
    let mut config = AuthConfig::development();
    config.session_ttl_secs = 0;
    let auth = AuthService::new(db.clone(), config);
    auth.create_user("short@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    let app = ferrioxid::App::new(db).with_auth(auth);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, app.into_router()).await });
    let login = tokio::task::spawn_blocking(move || {
        http(
            port,
            "POST",
            "/auth/login",
            Some(r#"{"email":"short@example.com","password":"correct-horse-battery"}"#),
            "",
        )
    })
    .await
    .unwrap();
    assert_eq!(login.status, 200, "{}", login.json);
    let cookie = session_cookie(&login.headers);
    let me = tokio::task::spawn_blocking(move || {
        http(
            port,
            "GET",
            "/auth/me",
            None,
            &format!("Cookie: {cookie}\r\n"),
        )
    })
    .await
    .unwrap();
    assert_eq!(me.status, 401, "{}", me.json);
    server.abort();
}
