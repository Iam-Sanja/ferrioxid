use crate::password::{ephemeral_password, ephemeral_weak_password};
use crate::recovery::{complete_reset, exchange_token, request_reset, CompleteError};
use crate::{apply_migrations, mount, AuthConfig, AuthService};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use fexu_mail::{MailService, SmtpSecurity, SmtpSettings};
use http_body_util::BodyExt;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;

async fn setup(attempts: u32) -> (AuthService, MailService, fexu_mail::MemoryInbox) {
    let db = fexu_db::connect("sqlite::memory:").await.expect("sqlite");
    apply_migrations(&db).await.expect("schema");
    let mut config = AuthConfig::development();
    config.login_max_attempts = attempts;
    let auth = AuthService::new(db, config);
    let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
    (auth, mail, inbox)
}

fn token_from(body: &str) -> String {
    body.lines()
        .find(|line| line.contains("token="))
        .expect("reset url")
        .split("token=")
        .nth(1)
        .expect("token")
        .trim()
        .to_owned()
}

#[tokio::test]
async fn reset_keeps_accounts_private_and_sessions_consistent() {
    let (auth, mail, inbox) = setup(5).await;
    let current = ephemeral_password();
    let replacement = ephemeral_password();
    let mismatched = ephemeral_password();
    let weak = ephemeral_weak_password();
    auth.create_user("known@example.com", &current, false, false)
        .await
        .unwrap();
    auth.create_user("idle@example.com", &current, false, false)
        .await
        .unwrap();
    auth.set_user_active("idle@example.com", false)
        .await
        .unwrap();
    let issued = auth
        .login("known@example.com", &current, "peer")
        .await
        .unwrap();

    request_reset(&auth, Some(&mail), "known@example.com", "203.0.113.5", "de").await;
    request_reset(
        &auth,
        Some(&mail),
        "missing@example.com",
        "203.0.113.6",
        "de",
    )
    .await;
    request_reset(&auth, Some(&mail), "idle@example.com", "203.0.113.7", "de").await;
    let messages = inbox.messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].to, "known@example.com");
    assert!(!messages[0].text.contains(&current));
    let token = token_from(&messages[0].text);
    assert!(!messages[0].subject.contains('\n'));

    let app = mount(Router::new()).with_state(crate::AppState {
        db: auth.database().clone(),
        auth: Some(auth.clone()),
        mail: Some(mail.clone()),
    });
    let known = post_reset(app.clone(), "known@example.com").await;
    let unknown = post_reset(app, "missing@example.com").await;
    assert_eq!(known.0, unknown.0);
    assert_eq!(known.1, unknown.1);
    assert!(known.1.contains("Wenn ein Konto"));
    assert!(!known.1.contains("known@example.com"));

    assert!(
        exchange_token(&auth, "not-a-real-token-value-with-enough-length")
            .await
            .is_err()
    );
    let opened = exchange_token(&auth, &token).await.expect("exchange");
    let mismatch = complete_reset(
        &auth,
        Some(&mail),
        &opened.secret,
        &replacement,
        &mismatched,
        "de",
    )
    .await;
    assert_eq!(mismatch, Err(CompleteError::Mismatch));
    let weak = complete_reset(&auth, Some(&mail), &opened.secret, &weak, &weak, "de").await;
    assert_eq!(weak, Err(CompleteError::Weak));
    let reused = complete_reset(&auth, Some(&mail), &opened.secret, &current, &current, "de").await;
    assert_eq!(reused, Err(CompleteError::Reused));
    let first = complete_reset(
        &auth,
        Some(&mail),
        &opened.secret,
        &replacement,
        &replacement,
        "en",
    )
    .await;
    assert!(first.is_ok());
    let replay = complete_reset(
        &auth,
        Some(&mail),
        &opened.secret,
        &replacement,
        &replacement,
        "en",
    )
    .await;
    assert_eq!(replay, Err(CompleteError::Expired));
    assert!(exchange_token(&auth, &token).await.is_err());
    assert!(auth.authenticate_token(&issued.token).await.is_err());
    assert!(auth
        .login("known@example.com", &current, "peer")
        .await
        .is_err());
    auth.login("known@example.com", &replacement, "peer")
        .await
        .expect("new password");

    let audit = auth
        .database()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT summary FROM _fexu_admin_audit WHERE action = 'password_reset'",
        ))
        .await
        .unwrap()
        .unwrap();
    let summary = audit.try_get::<String>("", "summary").unwrap();
    assert_eq!(summary, "password reset completed");
    assert!(!summary.contains(&token));
    assert!(!summary.contains(&replacement));
    let changed = inbox.messages();
    let confirmation = changed
        .iter()
        .find(|message| message.subject == "Password changed")
        .expect("confirmation");
    assert!(!confirmation.text.contains(&token));
    assert!(!confirmation.text.contains(&replacement));
}

#[tokio::test]
async fn rate_limit_parallel_replay_and_failed_delivery_stay_closed() {
    let current = ephemeral_password();
    let replacement = ephemeral_password();
    let other = ephemeral_password();
    let (auth, mail, inbox) = setup(1).await;
    auth.create_user("known@example.com", &current, false, false)
        .await
        .unwrap();
    request_reset(&auth, Some(&mail), "known@example.com", "203.0.113.8", "de").await;
    request_reset(&auth, Some(&mail), "known@example.com", "203.0.113.8", "de").await;
    assert_eq!(inbox.messages().len(), 1);
    let token = token_from(&inbox.messages()[0].text);
    auth.database()
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE _fexu_password_resets SET expires_at = 1 WHERE token_hash = ?",
            [crate_digest(&token).into()],
        ))
        .await
        .unwrap();
    assert!(exchange_token(&auth, &token).await.is_err());

    let (auth, mail, inbox) = setup(5).await;
    auth.create_user("known@example.com", &current, false, false)
        .await
        .unwrap();
    request_reset(&auth, Some(&mail), "known@example.com", "203.0.113.9", "de").await;
    let token = token_from(&inbox.messages()[0].text);
    let opened = exchange_token(&auth, &token).await.unwrap();
    let left = complete_reset(
        &auth,
        Some(&mail),
        &opened.secret,
        &replacement,
        &replacement,
        "de",
    );
    let right = complete_reset(&auth, Some(&mail), &opened.secret, &other, &other, "de");
    let (left, right) = tokio::join!(left, right);
    let wins = [left.is_ok(), right.is_ok()]
        .into_iter()
        .filter(|ok| *ok)
        .count();
    assert_eq!(wins, 1);

    let (auth, _, _) = setup(5).await;
    auth.create_user("known@example.com", &current, false, false)
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    // A closed port can time out; this case needs a definitive SMTP rejection.
    let rejection = tokio::spawn(async move {
        let (mut connection, _) = listener.accept().await.unwrap();
        connection
            .write_all(b"554 Service unavailable\r\n")
            .await
            .unwrap();
    });
    let failing = MailService::smtp(
        SmtpSettings {
            host: "127.0.0.1".into(),
            port,
            security: SmtpSecurity::Starttls,
            username: "mailer".into(),
            password: "super-secret-value".into(),
            timeout: Duration::from_secs(10),
        },
        "noreply@example.com",
        None,
        "http://127.0.0.1:3000",
        1800,
    );
    request_reset(
        &auth,
        Some(&failing),
        "known@example.com",
        "203.0.113.10",
        "de",
    )
    .await;
    tokio::time::timeout(Duration::from_secs(10), rejection)
        .await
        .unwrap()
        .unwrap();
    let usable = auth
        .database()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS open_count FROM _fexu_password_resets WHERE used_at IS NULL",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "open_count")
        .unwrap();
    assert_eq!(usable, 0);
    let code = auth
        .database()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT error_code FROM _fexu_mail_outbox",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "error_code")
        .unwrap();
    assert_eq!(code, "mail.rejected");
    assert!(!code.contains("super-secret-value"));
}

fn crate_digest(value: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn post_reset(app: Router, email: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(
            Request::get("/auth/password-reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let html = body_string(response).await;
    let csrf = html
        .split("name=\"csrf_token\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let response = app
        .oneshot(
            Request::post("/auth/password-reset")
                .header("origin", "http://127.0.0.1:3000")
                .header("cookie", cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "csrf_token={csrf}&locale=de&email={email}"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, body_string(response).await)
}

async fn body_string(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}
