//! Kontaktformular mit anonymer Sitzung. Die Prüfung läuft über die Handler, nicht über eine zweite CSRF-Implementierung.

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use ferrioxid::auth::{AppState, AuthConfig, AuthService};
use ferrioxid::urls::UrlSet;
use http_body_util::BodyExt;
use std::sync::Arc;

fn state(auth: AuthService) -> AppState {
    AppState {
        db: auth.database().clone(),
        auth: Some(auth),
        mail: None,
    }
}

fn routes() -> Arc<ferrioxid::urls::RouteMap> {
    let set = UrlSet::new()
        .route(
            "shop.thanks",
            "/contact/thanks",
            axum::routing::get(myshop::contact::thanks),
        )
        .expect("thanks route");
    Arc::new(set.names().clone())
}

fn cookie_headers(set_cookie: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let pair = set_cookie.split(';').next().unwrap();
    headers.insert(
        axum::http::header::COOKIE,
        HeaderValue::from_str(pair).unwrap(),
    );
    headers
}

fn token_in(html: &str) -> String {
    let marker = "name=\"csrf_token\" value=\"";
    let start = html.find(marker).expect("csrf field") + marker.len();
    html[start..].split('"').next().unwrap().to_owned()
}

#[tokio::test]
async fn contact_rejects_missing_forged_and_rotated_tokens() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    ferrioxid::auth::apply_migrations(&db).await.unwrap();
    let auth = AuthService::new(db, AuthConfig::development());
    auth.create_user("ada@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    let app = state(auth.clone());
    let names = routes();

    let response = myshop::contact::show(State(app.clone()), HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("ferrioxid_anonymous="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("Path=/"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(!cookie.contains("Domain="));
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    let csrf = token_in(&html);
    let raw = cookie.split(';').next().unwrap().split_once('=').unwrap().1;
    assert!(!html.contains(raw));

    let missing = myshop::contact::submit(
        State(app.clone()),
        Extension(names.clone()),
        cookie_headers(&cookie),
        Bytes::from_static(b"name=Ada+Lovelace&email=ada@shop.example&message=Hallo"),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::FORBIDDEN);
    let denied = missing.into_body().collect().await.unwrap().to_bytes();
    let denied = String::from_utf8(denied.to_vec()).unwrap();
    assert!(!denied.contains(&csrf));

    let forged_cookie = cookie_headers("ferrioxid_anonymous=forged-session-token");
    let forged = myshop::contact::submit(
        State(app.clone()),
        Extension(names.clone()),
        forged_cookie,
        Bytes::from(format!(
            "name=Ada+Lovelace&email=ada@shop.example&message=Hallo&csrf_token={csrf}"
        )),
    )
    .await;
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);

    let accepted = myshop::contact::submit(
        State(app.clone()),
        Extension(names.clone()),
        cookie_headers(&cookie),
        Bytes::from(format!(
            "name=Ada+Lovelace&email=ada@shop.example&message=Hallo&csrf_token={csrf}"
        )),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::SEE_OTHER);

    let logged_in = auth
        .login_replacing(
            "ada@example.com",
            "correct-horse-battery",
            "peer",
            Some(raw),
        )
        .await
        .unwrap();
    assert_ne!(logged_in.token, raw);
    let replay = myshop::contact::submit(
        State(app.clone()),
        Extension(names.clone()),
        cookie_headers(&cookie),
        Bytes::from(format!(
            "name=Ada+Lovelace&email=ada@shop.example&message=Hallo&csrf_token={csrf}"
        )),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::FORBIDDEN);
    let mut user_headers = HeaderMap::new();
    user_headers.insert(
        axum::http::header::COOKIE,
        HeaderValue::from_str(&format!("ferrioxid_session={}", logged_in.token)).unwrap(),
    );
    let rotated = myshop::contact::submit(
        State(app),
        Extension(names),
        user_headers,
        Bytes::from(format!(
            "name=Ada+Lovelace&email=ada@shop.example&message=Hallo&csrf_token={}",
            logged_in.csrf_token
        )),
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn contact_post_without_auth_service_is_rejected() {
    let db = ferrioxid::db::connect("sqlite::memory:").await.unwrap();
    let bare = AppState {
        db,
        auth: None,
        mail: None,
    };
    let body =
        Bytes::from("name=Ada+Lovelace&email=ada@shop.example&message=Hallo&csrf_token=not-issued");
    let response = myshop::contact::submit(
        State(bare.clone()),
        Extension(routes()),
        HeaderMap::new(),
        body.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(response
        .headers()
        .get(axum::http::header::LOCATION)
        .is_none());
    let html = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("form.csrf"), "{html}");
    assert!(!html.contains("not-issued"));

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        HeaderValue::from_static("ferrioxid_session=unchecked-session-token"),
    );
    let cookie_post =
        myshop::contact::submit(State(bare), Extension(routes()), headers, body).await;
    assert_eq!(cookie_post.status(), StatusCode::FORBIDDEN);
    let html = String::from_utf8(
        cookie_post
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(!html.contains("unchecked-session-token"));
}
