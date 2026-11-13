use crate::{AuthConfig, AuthError, AuthService, Environment};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

fn memory() -> &'static str {
    "sqlite::memory:"
}

async fn service(config: AuthConfig) -> AuthService {
    let db = ferrioxid_db::connect(memory()).await.expect("sqlite");
    crate::apply_migrations(&db).await.expect("auth schema");
    AuthService::new(db, config)
}

#[tokio::test]
async fn sessions_permissions_and_repeatable_migrations() {
    let auth = service(AuthConfig::development()).await;
    let password = "correct-horse-battery";
    let user = auth
        .create_user(" Admin@Example.com ", password, true, false)
        .await
        .expect("user");
    assert_eq!(user.email, "admin@example.com");
    assert!(user.is_staff);
    assert!(!user.is_superuser);
    let duplicate = auth
        .create_user("admin@example.com", password, false, false)
        .await;
    assert!(matches!(duplicate, Err(AuthError::Validation(_))));

    assert!(matches!(
        auth.login("missing@example.com", password, "peer").await,
        Err(AuthError::InvalidCredentials)
    ));
    assert!(matches!(
        auth.login("admin@example.com", "wrong-password-1", "peer")
            .await,
        Err(AuthError::InvalidCredentials)
    ));

    let issued = auth
        .login("admin@example.com", password, "peer")
        .await
        .expect("login");
    assert!(issued.cookie.starts_with("ferrioxid_session="));
    assert!(!issued.cookie.contains("Secure"));
    assert!(issued.cookie.contains("HttpOnly"));
    assert!(issued.cookie.contains("SameSite=Lax"));
    assert!(!issued.cookie.contains("Domain="));
    let public = serde_json::to_string(&issued.user).unwrap();
    assert!(!public.contains("password"));
    assert!(!public.contains(&issued.token));
    assert_ne!(issued.token, issued.csrf_token);

    let row = auth
        .database()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT token_hash, csrf_token_hash FROM _ferrioxid_sessions",
        ))
        .await
        .unwrap()
        .unwrap();
    let token_hash: String = row.try_get("", "token_hash").unwrap();
    let csrf_hash: String = row.try_get("", "csrf_token_hash").unwrap();
    assert_eq!(token_hash.len(), 64);
    assert_ne!(token_hash, issued.token);
    assert_ne!(csrf_hash, issued.csrf_token);

    let current = auth
        .authenticate_token(&issued.token)
        .await
        .expect("session");
    assert_eq!(current.id, user.id);
    assert!(matches!(
        auth.authenticate_token("not-a-session").await,
        Err(AuthError::Unauthenticated)
    ));
    assert!(matches!(
        auth.authorize(&current, None, "read").await,
        Err(AuthError::Forbidden)
    ));
    auth.authorize(&current, Some("authenticated"), "read")
        .await
        .unwrap();
    assert!(matches!(
        auth.authorize(&current, Some("notes"), "read").await,
        Err(AuthError::Forbidden)
    ));
    auth.grant_permission("editor", "notes.read").await.unwrap();
    auth.assign_role("admin@example.com", "editor")
        .await
        .unwrap();
    auth.authorize(&current, Some("notes"), "read")
        .await
        .unwrap();
    assert!(matches!(
        auth.authorize(&current, Some("notes"), "create").await,
        Err(AuthError::Forbidden)
    ));
    auth.grant_permission("editor", "notes.create")
        .await
        .unwrap();
    auth.grant_permission("editor", "notes.update")
        .await
        .unwrap();
    auth.grant_permission("editor", "notes.delete")
        .await
        .unwrap();
    auth.authorize(&current, Some("notes"), "create")
        .await
        .unwrap();
    auth.authorize(&current, Some("notes"), "update")
        .await
        .unwrap();
    auth.authorize(&current, Some("notes"), "delete")
        .await
        .unwrap();
    auth.revoke_permission("editor", "notes.delete")
        .await
        .unwrap();
    assert!(matches!(
        auth.authorize(&current, Some("notes"), "delete").await,
        Err(AuthError::Forbidden)
    ));

    auth.create_user("root@example.com", "superuser-password", true, true)
        .await
        .unwrap();
    let root = auth
        .login("root@example.com", "superuser-password", "root")
        .await
        .unwrap();
    let root_user = auth.authenticate_token(&root.token).await.unwrap();
    auth.authorize(&root_user, Some("notes"), "delete")
        .await
        .unwrap();

    let rotated = auth.rotate_session(&issued.token).await.unwrap();
    assert_ne!(rotated.token, issued.token);
    assert!(matches!(
        auth.authenticate_token(&issued.token).await,
        Err(AuthError::Unauthenticated)
    ));
    auth.authenticate_token(&rotated.token).await.unwrap();
    let expiries = expiries_for(auth.database(), &user.id.to_string()).await;
    assert!(expiries.len() >= 2);
    assert!(expiries.iter().all(|value| *value == expiries[0]));

    let rotated_user = auth.authenticate_token(&rotated.token).await.unwrap();
    auth.change_password(&rotated_user, password, "updated-password-1")
        .await
        .unwrap();
    assert!(matches!(
        auth.authenticate_token(&rotated.token).await,
        Err(AuthError::Unauthenticated)
    ));
    assert!(matches!(
        auth.login("admin@example.com", password, "peer").await,
        Err(AuthError::InvalidCredentials)
    ));
    let again = auth
        .login("admin@example.com", "updated-password-1", "peer")
        .await
        .unwrap();
    auth.set_user_active("admin@example.com", false)
        .await
        .unwrap();
    assert!(matches!(
        auth.authenticate_token(&again.token).await,
        Err(AuthError::Unauthenticated)
    ));
    assert!(matches!(
        auth.login("admin@example.com", "updated-password-1", "peer")
            .await,
        Err(AuthError::InvalidCredentials)
    ));

    let applied = crate::apply_migrations(auth.database()).await.unwrap();
    assert!(applied.is_empty());
    let history = auth
        .database()
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT id FROM _ferrioxid_auth_migrations ORDER BY id",
        ))
        .await
        .unwrap();
    let ids = history
        .iter()
        .map(|row| row.try_get::<String>("", "id").unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "0001_auth_schema".to_owned(),
            "0002_admin_journal".to_owned(),
            "0003_audit_operation".to_owned(),
            "0004_password_reset".to_owned(),
            "0005_anonymous_sessions".to_owned(),
            "0006_account_security".to_owned()
        ]
    );
    assert_eq!(auth.list_users().await.unwrap().len(), 2);
}

#[tokio::test]
async fn expired_idle_and_secure_cookie() {
    let mut expired = AuthConfig::development();
    expired.session_ttl_secs = 0;
    let auth = service(expired).await;
    auth.create_user("short@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    let issued = auth
        .login("short@example.com", "correct-horse-battery", "peer")
        .await
        .unwrap();
    assert!(matches!(
        auth.authenticate_token(&issued.token).await,
        Err(AuthError::Unauthenticated)
    ));
    assert!(matches!(
        auth.rotate_session(&issued.token).await,
        Err(AuthError::Unauthenticated)
    ));

    let mut idle = AuthConfig::development();
    idle.idle_timeout_secs = -1;
    let auth = service(idle).await;
    auth.create_user("idle@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    let issued = auth
        .login("idle@example.com", "correct-horse-battery", "peer")
        .await
        .unwrap();
    assert!(matches!(
        auth.authenticate_token(&issued.token).await,
        Err(AuthError::Unauthenticated)
    ));

    let mut secure = AuthConfig::development();
    secure.environment = Environment::Production;
    secure.cookie_secure = true;
    let auth = service(secure).await;
    auth.create_user("secure@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    let issued = auth
        .login("secure@example.com", "correct-horse-battery", "peer")
        .await
        .unwrap();
    assert!(issued.cookie.contains("Secure"));
    assert!(issued.cookie.contains("HttpOnly"));
    let cleared = auth.logout(Some(&issued.token)).await.unwrap();
    assert!(cleared.contains("Max-Age=0"));
    assert!(cleared.contains("Secure"));
    assert!(matches!(
        auth.authenticate_token(&issued.token).await,
        Err(AuthError::Unauthenticated)
    ));
    assert!(auth.logout(None).await.unwrap().contains("Max-Age=0"));
}

#[tokio::test]
async fn login_rate_limit_and_proxy_header() {
    let mut config = AuthConfig::development();
    config.login_max_attempts = 2;
    let auth = service(config).await;
    auth.create_user("limit@example.com", "correct-horse-battery", false, false)
        .await
        .unwrap();
    assert!(matches!(
        auth.login("limit@example.com", "wrong-password-1", "10.0.0.1")
            .await,
        Err(AuthError::InvalidCredentials)
    ));
    assert!(matches!(
        auth.login("limit@example.com", "wrong-password-1", "10.0.0.1")
            .await,
        Err(AuthError::InvalidCredentials)
    ));
    assert!(matches!(
        auth.login("limit@example.com", "correct-horse-battery", "10.0.0.1")
            .await,
        Err(AuthError::RateLimited)
    ));
    auth.login("limit@example.com", "correct-horse-battery", "10.0.0.2")
        .await
        .unwrap();

    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-forwarded-for", "203.0.113.5, 10.0.0.1".parse().unwrap());
    assert_eq!(auth.client_key(&headers, "127.0.0.1"), "127.0.0.1");
    let mut trusting = AuthConfig::development();
    trusting.trust_proxy = true;
    let unlisted = AuthService::new(auth.database().clone(), trusting.clone());
    assert_eq!(unlisted.client_key(&headers, "127.0.0.1"), "127.0.0.1");
    trusting.trusted_proxies = vec!["127.0.0.1".to_owned()];
    let trusting = AuthService::new(auth.database().clone(), trusting);
    assert_eq!(trusting.client_key(&headers, "127.0.0.1"), "203.0.113.5");
    assert_eq!(trusting.client_key(&headers, "10.1.1.1"), "10.1.1.1");
    headers.insert("x-forwarded-host", "https://evil.example".parse().unwrap());
    headers.insert("origin", "https://evil.example".parse().unwrap());
    assert!(trusting.presented_origin_allowed(&headers).is_err());
}

async fn expiries_for(db: &sea_orm::DatabaseConnection, user_id: &str) -> Vec<i64> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT expires_at FROM _ferrioxid_sessions WHERE user_id = ?",
            [user_id.into()],
        ))
        .await
        .unwrap();
    rows.iter()
        .map(|row| row.try_get::<i64>("", "expires_at").unwrap())
        .collect()
}
