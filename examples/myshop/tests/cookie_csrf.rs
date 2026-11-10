//! Der App-Router prüft Cookie-Schreibzugriffe, auch wenn der Handler selbst keinen Guard aufruft.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ferrioxid::auth::{AppState, AuthConfig, AuthService};
use ferrioxid::App;
use http_body_util::BodyExt;
use myshop::models::product::Product;
use tower::ServiceExt;

fn probe_routes() -> axum::Router<AppState> {
    axum::Router::new().route(
        "/probe",
        axum::routing::post(probe)
            .put(probe)
            .patch(probe)
            .delete(probe),
    )
}

async fn probe(body: axum::body::Bytes) -> String {
    String::from_utf8_lossy(&body).into_owned()
}

#[allow(clippy::too_many_arguments)]
async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    authorization: Option<&str>,
    content_type: Option<&str>,
    body: &str,
) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    if let Some(authorization) = authorization {
        builder = builder.header("authorization", authorization);
    }
    if let Some(content_type) = content_type {
        builder = builder.header("content-type", content_type);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// Dieselbe öffentliche In-Memory-Fixture wie die übrigen Auth-Tests. Kein ausgeliefertes Geheimnis.
fn memory_fixture_password() -> String {
    String::from_utf8(vec![
        99, 111, 114, 114, 101, 99, 116, 45, 104, 111, 114, 115, 101, 45, 98, 97, 116, 116, 101,
        114, 121,
    ])
    .expect("fixture password")
}

fn pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn cookie_writes_require_csrf_before_the_handler() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    ferrioxid::auth::apply_migrations(&db).await.unwrap();
    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    ferrioxid::migrations::apply_directory(&db, &migrations, false)
        .await
        .unwrap();
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let password = memory_fixture_password();
    auth.create_user("ada@example.com", &password, false, false)
        .await
        .unwrap();
    let issued = auth
        .login("ada@example.com", &password, "127.0.0.1")
        .await
        .unwrap();
    let session = format!("ferrioxid_session={}", issued.token);
    let anonymous = auth
        .ensure_anonymous_session(&axum::http::HeaderMap::new())
        .await
        .unwrap();
    let anonymous_cookie = pair(anonymous.cookie.as_deref().unwrap());
    let app = App::new(db.clone())
        .with_auth(auth.clone())
        .merge(probe_routes())
        .register::<Product>()
        .into_router();

    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        let (status, body) = send(
            &app,
            method,
            "/probe",
            Some(&session),
            None,
            None,
            Some("application/json"),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {body}");
        assert!(body.contains("csrf token is missing or invalid"), "{body}");
        assert!(!body.contains(&issued.token), "{body}");
        assert!(!body.contains(&issued.csrf_token), "{body}");

        let (status, body) = send(
            &app,
            method,
            "/probe",
            Some(&session),
            Some(&issued.csrf_token),
            None,
            Some("application/json"),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{method} {body}");

        let (status, body) = send(
            &app,
            method,
            "/probe",
            Some(&anonymous_cookie),
            None,
            None,
            Some("application/json"),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} anonymous {body}");
        assert!(!body.contains(&anonymous.csrf_token), "{body}");

        let (status, _) = send(
            &app,
            method,
            "/probe",
            Some(&anonymous_cookie),
            Some(&anonymous.csrf_token),
            None,
            Some("application/json"),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{method} anonymous");

        let (status, _) = send(
            &app,
            method,
            "/probe",
            None,
            None,
            Some("Bearer opaque-client"),
            Some("application/json"),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{method} bearer");
    }

    let (status, body) = send(
        &app,
        "POST",
        "/probe",
        Some(&session),
        None,
        Some("Bearer opaque-client"),
        Some("application/json"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let (status, _) = send(
        &app,
        "POST",
        "/probe",
        Some("ferrioxid_locale=de"),
        None,
        None,
        Some("application/json"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &app,
        "POST",
        "/probe",
        Some("ferrioxid_login_csrf=abc"),
        None,
        None,
        Some("application/json"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "POST",
        "/probe",
        Some(&session),
        None,
        None,
        Some("application/x-www-form-urlencoded"),
        "name=Ada",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(
        body.contains("Die Sicherheitsprüfung ist fehlgeschlagen."),
        "{body}"
    );
    assert!(!body.contains(&issued.token), "{body}");

    let form = format!("csrf_token={}&marker=kept", issued.csrf_token);
    let (status, body) = send(
        &app,
        "POST",
        "/probe",
        Some(&session),
        None,
        None,
        Some("application/x-www-form-urlencoded"),
        &form,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("marker=kept"), "{body}");

    let multipart = format!(
        "--bound\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\n{}\r\n--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\n\r\nhello-file\r\n--bound--\r\n",
        issued.csrf_token
    );
    let (status, body) = send(
        &app,
        "POST",
        "/probe",
        Some(&session),
        None,
        None,
        Some("multipart/form-data; boundary=bound"),
        &multipart,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("hello-file"), "{body}");

    let product =
        r#"{"name":"Widget","price_cents":100,"price":"1.00","status":"draft","active":true}"#;
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/products",
        None,
        None,
        None,
        Some("application/json"),
        product,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = send(
        &app,
        "POST",
        "/api/v1/products",
        Some(&session),
        None,
        None,
        Some("application/json"),
        product,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.contains("csrf"));
    assert!(!body.contains(&issued.token));
    let created = send(
        &app,
        "POST",
        "/api/v1/products",
        Some(&session),
        Some(&issued.csrf_token),
        None,
        Some("application/json"),
        product,
    )
    .await;
    assert_eq!(created.0, StatusCode::CREATED, "{}", created.1);
    let id = serde_json::from_str::<serde_json::Value>(&created.1).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/v1/products/{id}"),
        Some(&session),
        None,
        None,
        Some("application/json"),
        r#"{"name":"Widget 2"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/v1/products/{id}"),
        Some(&session),
        None,
        None,
        None,
        "",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/v1/products/{id}"),
        Some(&session),
        None,
        None,
        Some("application/json"),
        product,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let mut renamed = AuthConfig::development();
    renamed.cookie_name = "shop_session".to_owned();
    let renamed = App::new(db.clone())
        .with_auth(AuthService::new(db.clone(), renamed))
        .merge(probe_routes())
        .into_router();
    let (status, _) = send(
        &renamed,
        "POST",
        "/probe",
        Some(&session),
        None,
        None,
        Some("application/json"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send(
        &renamed,
        "DELETE",
        "/probe",
        Some(&format!("shop_session={}", issued.token)),
        None,
        None,
        Some("application/json"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let bare = App::new(db).merge(probe_routes()).into_router();
    let (status, body) = send(
        &bare,
        "POST",
        "/probe",
        Some("ferrioxid_session=unchecked"),
        None,
        None,
        Some("application/json"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(!body.contains("unchecked"));
    let (status, _) = send(
        &bare,
        "PUT",
        "/probe",
        None,
        None,
        Some("Bearer opaque-client"),
        Some("application/json"),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

fn shop_routes() -> axum::Router<AppState> {
    axum::Router::new().route("/shop", axum::routing::get(shop_page))
}

async fn shop_page(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let Some(auth) = state.auth.as_ref() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let session = auth
        .ensure_anonymous_session(&headers)
        .await
        .expect("anonymous session");
    let mut response = axum::response::Html("shop").into_response();
    if let Some(cookie) = session.cookie {
        response.headers_mut().append(
            axum::http::header::SET_COOKIE,
            axum::http::HeaderValue::from_str(&cookie).expect("cookie"),
        );
    }
    response
}

struct Exchange {
    status: StatusCode,
    cookies: Vec<String>,
    body: String,
}

async fn exchange(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    content_type: Option<&str>,
    body: &str,
) -> Exchange {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(content_type) = content_type {
        builder = builder.header("content-type", content_type);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let cookies = response
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok().map(str::to_owned))
        .collect();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Exchange {
        status,
        cookies,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap_or("").trim().to_owned()
}

fn cookie_named<'a>(cookies: &'a [String], name: &str) -> Option<&'a str> {
    cookies.iter().find_map(|cookie| {
        let pair = cookie_pair(cookie);
        let (key, _) = pair.split_once('=')?;
        (key == name).then_some(cookie.as_str())
    })
}

fn form_token(body: &str) -> String {
    let marker = "name=\"csrf_token\" value=\"";
    let start = body.find(marker).expect("login csrf field") + marker.len();
    body[start..].split('"').next().unwrap().to_owned()
}

fn encode_form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", encode_component(key), encode_component(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[tokio::test]
async fn admin_login_accepts_login_csrf_while_an_anonymous_session_exists() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    ferrioxid::auth::apply_migrations(&db).await.unwrap();
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let password = memory_fixture_password();
    auth.create_user("ada@example.com", &password, true, false)
        .await
        .unwrap();
    let app = App::new(db)
        .with_auth(auth.clone())
        .with_admin(ferrioxid::admin::AdminRegistry::new())
        .merge(shop_routes())
        .into_router();

    let shop = exchange(&app, "GET", "/shop", None, None, "").await;
    assert_eq!(shop.status, StatusCode::OK);
    let anonymous = cookie_named(&shop.cookies, "ferrioxid_anonymous").expect("anonymous cookie");
    let anonymous_pair = cookie_pair(anonymous);
    let anonymous_token = anonymous_pair.split_once('=').unwrap().1.to_owned();

    let login_page = exchange(&app, "GET", "/admin/login", Some(&anonymous_pair), None, "").await;
    assert_eq!(login_page.status, StatusCode::OK, "{}", login_page.body);
    let login_cookie =
        cookie_named(&login_page.cookies, "ferrioxid_login_csrf").expect("login csrf");
    let login_pair = cookie_pair(login_cookie);
    let login_token = form_token(&login_page.body);
    assert_eq!(login_pair.split_once('=').unwrap().1, login_token);

    let forged = encode_form(&[
        ("csrf_token", &anonymous_token),
        ("email", "ada@example.com"),
        ("password", &password),
        ("next", "/admin"),
    ]);
    let rejected = exchange(
        &app,
        "POST",
        "/admin/login",
        Some(&format!("{anonymous_pair}; {login_pair}")),
        Some("application/x-www-form-urlencoded"),
        &forged,
    )
    .await;
    assert_eq!(rejected.status, StatusCode::FORBIDDEN, "{}", rejected.body);
    assert!(auth.read_anonymous_state(&anonymous_token).await.is_ok());

    let missing = encode_form(&[
        ("email", "ada@example.com"),
        ("password", &password),
        ("next", "/admin"),
    ]);
    let missing = exchange(
        &app,
        "POST",
        "/admin/login",
        Some(&format!("{anonymous_pair}; {login_pair}")),
        Some("application/x-www-form-urlencoded"),
        &missing,
    )
    .await;
    assert_eq!(missing.status, StatusCode::FORBIDDEN, "{}", missing.body);

    let prior = auth
        .login("ada@example.com", &password, "peer")
        .await
        .unwrap();
    let both = format!(
        "ferrioxid_session={}; {anonymous_pair}; {login_pair}",
        prior.token
    );
    let body = encode_form(&[
        ("csrf_token", &login_token),
        ("email", "ada@example.com"),
        ("password", &password),
        ("next", "/admin"),
    ]);
    let logged_in = exchange(
        &app,
        "POST",
        "/admin/login",
        Some(&both),
        Some("application/x-www-form-urlencoded"),
        &body,
    )
    .await;
    assert_eq!(
        logged_in.status,
        StatusCode::SEE_OTHER,
        "{}",
        logged_in.body
    );
    let session = cookie_named(&logged_in.cookies, "ferrioxid_session").expect("session");
    assert!(!cookie_pair(session).ends_with('='));
    let cleared = cookie_named(&logged_in.cookies, "ferrioxid_anonymous").expect("clear anonymous");
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
    assert!(auth.read_anonymous_state(&anonymous_token).await.is_err());

    let session_pair = cookie_pair(session);
    let csrf = exchange(&app, "GET", "/auth/csrf", Some(&session_pair), None, "").await;
    assert_eq!(csrf.status, StatusCode::OK, "{}", csrf.body);
    let csrf_token = csrf
        .body
        .split("\"csrf_token\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    let logged_out = exchange(
        &app,
        "POST",
        "/admin/logout",
        Some(&session_pair),
        Some("application/x-www-form-urlencoded"),
        &encode_form(&[("csrf_token", &csrf_token)]),
    )
    .await;
    assert_eq!(
        logged_out.status,
        StatusCode::SEE_OTHER,
        "{}",
        logged_out.body
    );
    let me = exchange(&app, "GET", "/auth/me", Some(&session_pair), None, "").await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED, "{}", me.body);

    let stateless = exchange(
        &app,
        "POST",
        "/auth/login",
        None,
        Some("application/json"),
        &format!("{{\"email\":\"ada@example.com\",\"password\":\"{password}\"}}"),
    )
    .await;
    assert_eq!(stateless.status, StatusCode::OK, "{}", stateless.body);
}
