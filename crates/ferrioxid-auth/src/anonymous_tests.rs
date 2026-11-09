use crate::{AuthConfig, AuthError, AuthService, BrowserActor};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};

async fn service(config: AuthConfig) -> AuthService {
    let db = ferrioxid_db::connect("sqlite::memory:")
        .await
        .expect("sqlite");
    crate::apply_migrations(&db).await.expect("auth schema");
    AuthService::new(db, config)
}

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.append(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value).expect("header"),
        );
    }
    map
}

fn token_of(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .to_owned()
}

fn anonymous_cookie(token: &str) -> String {
    format!("ferrioxid_anonymous={token}")
}

fn session_cookie(token: &str) -> String {
    format!("ferrioxid_session={token}")
}

async fn core_roundtrip(auth: &AuthService) {
    let issued = auth
        .ensure_anonymous_session(&HeaderMap::new())
        .await
        .unwrap();
    let cookie = issued.cookie.expect("new anonymous cookie");
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("Path=/"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(!cookie.contains("Domain="));
    assert!(!cookie.contains("Secure"));
    let token = token_of(&cookie);
    let again = auth
        .ensure_anonymous_session(&headers(&[("cookie", &anonymous_cookie(&token))]))
        .await
        .unwrap();
    assert_eq!(again.session_id, issued.session_id);
    assert_eq!(again.csrf_token, issued.csrf_token);
    assert!(again.cookie.is_none());

    let visitor = headers(&[("cookie", &anonymous_cookie(&token))]);
    assert!(matches!(
        auth.authenticate_browser_form(&visitor, &Method::POST, "")
            .await,
        Err(AuthError::CsrfRejected)
    ));
    assert!(matches!(
        auth.authenticate_browser_form(&visitor, &Method::POST, "not-the-token")
            .await,
        Err(AuthError::CsrfRejected)
    ));
    assert!(matches!(
        auth.authenticate_browser_form(&HeaderMap::new(), &Method::POST, &issued.csrf_token)
            .await,
        Err(AuthError::Unauthenticated)
    ));
    let forged = headers(&[("cookie", &anonymous_cookie("forged-session-token"))]);
    assert!(matches!(
        auth.authenticate_browser_form(&forged, &Method::POST, &issued.csrf_token)
            .await,
        Err(AuthError::Unauthenticated)
    ));

    let other = auth
        .ensure_anonymous_session(&HeaderMap::new())
        .await
        .unwrap();
    assert_ne!(other.session_id, issued.session_id);
    assert!(matches!(
        auth.authenticate_browser_form(&visitor, &Method::POST, &other.csrf_token)
            .await,
        Err(AuthError::CsrfRejected)
    ));
    assert!(matches!(
        auth.authenticate_browser_form(&visitor, &Method::PUT, &issued.csrf_token)
            .await,
        Ok(BrowserActor::Anonymous { session_id }) if session_id == issued.session_id
    ));
    assert!(matches!(
        auth.authenticate_browser_form(&visitor, &Method::PATCH, &issued.csrf_token)
            .await,
        Ok(BrowserActor::Anonymous { .. })
    ));
    assert!(matches!(
        auth.authenticate_browser_form(&visitor, &Method::DELETE, &issued.csrf_token)
            .await,
        Ok(BrowserActor::Anonymous { .. })
    ));

    auth.write_anonymous_state(&token, r#"{"cart":"cable"}"#)
        .await
        .unwrap();
    assert_eq!(
        auth.read_anonymous_state(&token).await.unwrap(),
        r#"{"cart":"cable"}"#
    );
    let other_token = token_of(other.cookie.as_deref().unwrap());
    assert_eq!(auth.read_anonymous_state(&other_token).await.unwrap(), "");
    assert!(auth
        .read_anonymous_state("forged-session-token")
        .await
        .is_err());
    let secret = format!(r#"{{"note":"{}"}}"#, "X".repeat(2000));
    let rejected = auth
        .write_anonymous_state(&token, &secret)
        .await
        .unwrap_err();
    assert!(!rejected.to_string().contains("XXXX"));
    assert!(auth
        .write_anonymous_state(&token, r#"["nope"]"#)
        .await
        .is_err());

    let (left, right) = tokio::join!(
        auth.authenticate_browser_form(&visitor, &Method::POST, &issued.csrf_token),
        auth.authenticate_browser_form(&visitor, &Method::POST, &issued.csrf_token),
    );
    assert!(left.is_ok());
    assert!(right.is_ok());

    let bearer = headers(&[("authorization", "Bearer public-api-token")]);
    assert!(matches!(
        auth.guard_cookie_write(&bearer, &Method::POST, "").await,
        Ok(BrowserActor::Stateless)
    ));
    assert!(matches!(
        auth.guard_cookie_write(&HeaderMap::new(), &Method::DELETE, "")
            .await,
        Ok(BrowserActor::Stateless)
    ));
    let bearer_and_cookie = headers(&[
        ("authorization", "Bearer public-api-token"),
        ("cookie", &anonymous_cookie(&token)),
    ]);
    assert!(matches!(
        auth.guard_cookie_write(&bearer_and_cookie, &Method::POST, "")
            .await,
        Err(AuthError::CsrfRejected)
    ));
    assert!(matches!(
        auth.guard_cookie_write(&bearer_and_cookie, &Method::POST, &issued.csrf_token)
            .await,
        Ok(BrowserActor::Anonymous { .. })
    ));

    assert!(matches!(
        auth.authenticate_token(&token).await,
        Err(AuthError::Unauthenticated)
    ));
    assert!(matches!(
        auth.authenticate_csrf(&visitor, &Method::POST, &issued.csrf_token)
            .await,
        Err(AuthError::Unauthenticated)
    ));

    let email = format!("visitor-{}@example.com", uuid::Uuid::new_v4().simple());
    auth.create_user(&email, "correct-horse-battery", true, false)
        .await
        .unwrap();
    let logged_in = auth
        .login_replacing(&email, "correct-horse-battery", "peer", Some(&token))
        .await
        .unwrap();
    assert_ne!(logged_in.token, token);
    assert!(logged_in
        .clear_cookies
        .iter()
        .any(|cookie| cookie.starts_with("ferrioxid_anonymous=") && cookie.contains("Max-Age=0")));
    assert!(matches!(
        auth.authenticate_browser_form(&visitor, &Method::POST, &issued.csrf_token)
            .await,
        Err(AuthError::Unauthenticated)
    ));
    assert!(auth.read_anonymous_state(&token).await.is_err());
    let user_headers = headers(&[("cookie", &session_cookie(&logged_in.token))]);
    assert!(matches!(
        auth.authenticate_browser_form(&user_headers, &Method::POST, &issued.csrf_token)
            .await,
        Err(AuthError::CsrfRejected)
    ));
    assert!(matches!(
        auth.authenticate_csrf(&user_headers, &Method::POST, &logged_in.csrf_token)
            .await,
        Ok(user) if user.is_staff && user.session_id != issued.session_id
    ));
    let both = headers(&[(
        "cookie",
        &format!(
            "{}; {}",
            session_cookie(&logged_in.token),
            anonymous_cookie(&token)
        ),
    )]);
    assert!(matches!(
        auth.authenticate_browser_form(&both, &Method::POST, &issued.csrf_token)
            .await,
        Err(AuthError::CsrfRejected)
    ));

    auth.revoke_anonymous_token(&other_token).await.unwrap();
    let cleared = auth.clear_anonymous_cookie().unwrap();
    assert!(cleared.contains("HttpOnly"));
    assert!(cleared.contains("Max-Age=0"));
    assert!(matches!(
        auth.authenticate_anonymous_csrf(
            &headers(&[("cookie", &anonymous_cookie(&other_token))]),
            &Method::POST,
            &other.csrf_token,
        )
        .await,
        Err(AuthError::Unauthenticated)
    ));
    let logged_out = auth.logout(Some(&logged_in.token)).await.unwrap();
    assert!(logged_out.contains("Max-Age=0"));
    assert!(matches!(
        auth.authenticate_token(&logged_in.token).await,
        Err(AuthError::Unauthenticated)
    ));
}

#[tokio::test]
async fn anonymous_sessions_cover_csrf_rotation_and_cookie_boundaries() {
    let auth = service(AuthConfig::development()).await;
    core_roundtrip(&auth).await;

    let mut strict = AuthConfig::development();
    strict.same_site = crate::CookieSameSite::Strict;
    let strict_auth = service(strict).await;
    let strict_cookie = strict_auth
        .ensure_anonymous_session(&HeaderMap::new())
        .await
        .unwrap()
        .cookie
        .unwrap();
    assert!(strict_cookie.contains("SameSite=Strict"));

    let mut expired = AuthConfig::development();
    expired.session_ttl_secs = 0;
    let expired_auth = service(expired).await;
    let dead = expired_auth
        .ensure_anonymous_session(&HeaderMap::new())
        .await
        .unwrap();
    let dead_token = token_of(dead.cookie.as_deref().unwrap());
    assert!(matches!(
        expired_auth
            .authenticate_browser_form(
                &headers(&[("cookie", &anonymous_cookie(&dead_token))]),
                &Method::POST,
                &dead.csrf_token,
            )
            .await,
        Err(AuthError::Unauthenticated)
    ));

    let mut idle = AuthConfig::development();
    idle.idle_timeout_secs = -1;
    let idle_auth = service(idle).await;
    let idle_session = idle_auth
        .ensure_anonymous_session(&HeaderMap::new())
        .await
        .unwrap();
    let idle_token = token_of(idle_session.cookie.as_deref().unwrap());
    assert!(matches!(
        idle_auth
            .authenticate_browser_form(
                &headers(&[("cookie", &anonymous_cookie(&idle_token))]),
                &Method::GET,
                "",
            )
            .await,
        Err(AuthError::Unauthenticated)
    ));

    let mut secure = AuthConfig::development();
    secure.cookie_secure = true;
    secure.allowed_origins = vec!["https://shop.example".to_owned()];
    let secure_auth = service(secure).await;
    let secure_session = secure_auth
        .ensure_anonymous_session(&HeaderMap::new())
        .await
        .unwrap();
    let secure_cookie = secure_session.cookie.as_deref().unwrap();
    assert!(secure_cookie.contains("Secure"));
    assert!(secure_cookie.contains("HttpOnly"));
    let secure_token = token_of(secure_cookie);
    let valid = headers(&[
        ("cookie", &anonymous_cookie(&secure_token)),
        ("origin", "https://shop.example"),
    ]);
    assert!(matches!(
        secure_auth
            .authenticate_browser_form(&valid, &Method::POST, &secure_session.csrf_token)
            .await,
        Ok(BrowserActor::Anonymous { .. })
    ));
    let referer = headers(&[
        ("cookie", &anonymous_cookie(&secure_token)),
        ("referer", "https://shop.example/contact"),
    ]);
    assert!(secure_auth
        .authenticate_browser_form(&referer, &Method::POST, &secure_session.csrf_token)
        .await
        .is_ok());
    let missing = headers(&[("cookie", &anonymous_cookie(&secure_token))]);
    assert!(matches!(
        secure_auth
            .authenticate_browser_form(&missing, &Method::POST, &secure_session.csrf_token)
            .await,
        Err(AuthError::OriginRejected)
    ));
    let cross = headers(&[
        ("cookie", &anonymous_cookie(&secure_token)),
        ("origin", "https://evil.example"),
    ]);
    assert!(matches!(
        secure_auth
            .authenticate_browser_form(&cross, &Method::POST, &secure_session.csrf_token)
            .await,
        Err(AuthError::OriginRejected)
    ));
    let plain = headers(&[
        ("cookie", &anonymous_cookie(&secure_token)),
        ("origin", "http://shop.example"),
    ]);
    assert!(matches!(
        secure_auth
            .authenticate_browser_form(&plain, &Method::POST, &secure_session.csrf_token)
            .await,
        Err(AuthError::OriginRejected)
    ));
    let nulled = headers(&[
        ("cookie", &anonymous_cookie(&secure_token)),
        ("origin", "null"),
        ("sec-fetch-site", "same-origin"),
    ]);
    assert!(secure_auth
        .authenticate_browser_form(&nulled, &Method::POST, &secure_session.csrf_token)
        .await
        .is_ok());

    let first = auth
        .ensure_anonymous_session(&HeaderMap::new())
        .await
        .unwrap();
    let second = auth
        .ensure_anonymous_session(&HeaderMap::new())
        .await
        .unwrap();
    assert_ne!(first.session_id, second.session_id);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
async fn postgres_anonymous_session_roundtrip() {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run the ignored PostgreSQL anonymous session test");
    let db = ferrioxid_db::connect(&url).await.expect("postgres");
    crate::apply_migrations(&db).await.expect("auth schema");
    let auth = AuthService::new(db, AuthConfig::development());
    core_roundtrip(&auth).await;
}
