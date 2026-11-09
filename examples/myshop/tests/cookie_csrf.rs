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
    auth.create_user("ada@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    let issued = auth
        .login("ada@example.com", "correct-horse-battery", "127.0.0.1")
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
