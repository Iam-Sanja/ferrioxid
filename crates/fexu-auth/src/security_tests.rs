//! Sicherheitsprüfungen für den zweiten Faktor, Sitzungen und Kontoschluss.

use crate::{AuthConfig, AuthError, AuthService, RemoteProtocol};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use tower::ServiceExt;
use uuid::Uuid;

async fn service() -> AuthService {
    let db = fexu_db::connect("sqlite::memory:").await.expect("sqlite");
    crate::apply_migrations(&db).await.expect("schema");
    AuthService::new(db, AuthConfig::development())
}

fn password() -> &'static str {
    static VALUE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VALUE.get_or_init(|| {
        String::from_utf8(vec![
            99, 111, 114, 114, 101, 99, 116, 45, 104, 111, 114, 115, 101, 45, 98, 97, 116, 116,
            101, 114, 121,
        ])
        .expect("fixture password")
    })
}

fn wrong_password() -> &'static str {
    static VALUE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VALUE.get_or_init(|| {
        String::from_utf8(vec![
            119, 114, 111, 110, 103, 45, 112, 97, 115, 115, 119, 111, 114, 100, 45, 49,
        ])
        .expect("fixture password")
    })
}

fn email(label: &str) -> String {
    format!("{label}-{}@example.com", Uuid::new_v4().simple())
}

async fn codes(auth: &AuthService, user_id: Uuid) -> (String, String) {
    let row = crate::account_store::find_totp(auth.database(), user_id)
        .await
        .expect("totp row")
        .expect("enrolled");
    let secret = crate::totp::open_stored(
        &auth.config().mfa_key,
        &crate::totp::legacy_mfa_key(&auth.config().csrf_secret),
        user_id,
        &row.nonce,
        &row.ciphertext,
    )
    .expect("secret");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    (
        crate::totp::totp_code(&secret, now, 6).expect("current"),
        crate::totp::totp_code(&secret, now.saturating_add(30), 6).expect("next"),
    )
}

async fn session_count(auth: &AuthService, user_id: Uuid) -> usize {
    crate::account_store::list_open_sessions(auth.database(), user_id)
        .await
        .expect("sessions")
        .len()
}

#[tokio::test]
async fn password_login_stays_available_without_totp() {
    password_login_stays_available_without_totp_on(&service().await).await;
}

async fn password_login_stays_available_without_totp_on(auth: &AuthService) {
    let address = email("plain");
    auth.create_user(&address, password(), false, false)
        .await
        .expect("user");
    let issued = auth
        .login(&address, password(), "peer")
        .await
        .expect("session");
    assert!(issued.cookie.contains("HttpOnly"));
    assert!(!RemoteProtocol::Oidc.is_implemented());
    assert!(!RemoteProtocol::Passkey.is_implemented());
}

#[tokio::test]
async fn confirmed_totp_blocks_session_until_the_second_factor() {
    confirmed_totp_blocks_session_until_the_second_factor_on(&service().await).await;
}

async fn confirmed_totp_blocks_session_until_the_second_factor_on(auth: &AuthService) {
    let address = email("mfa");
    let created = auth
        .create_user(&address, password(), false, false)
        .await
        .expect("user");
    let first = auth
        .login(&address, password(), "peer")
        .await
        .expect("first");
    let user = auth.authenticate_token(&first.token).await.expect("actor");
    let enrollment = auth.begin_totp(&user, password()).await.expect("enroll");
    assert!(!format!("{enrollment:?}").contains(&enrollment.secret_base32));
    let (current, next) = codes(auth, created.id).await;
    let recovery = auth.confirm_totp(&user, &current).await.expect("confirm");
    assert_eq!(recovery.codes.len(), 8);
    assert!(!format!("{recovery:?}").contains(&recovery.codes[0]));

    let before = session_count(auth, created.id).await;
    let pending = auth.login(&address, password(), "peer").await;
    let Err(AuthError::MfaPending(pending)) = pending else {
        panic!("mfa required");
    };
    assert!(!format!("{pending:?}").contains(&pending.challenge));
    assert_eq!(session_count(auth, created.id).await, before);
    assert!(auth.authenticate_token(&pending.challenge).await.is_err());

    let rejected = auth
        .complete_pending_login(&pending.challenge, "000000", "peer", None)
        .await;
    assert!(matches!(rejected, Err(AuthError::InvalidCredentials)));
    assert_eq!(session_count(auth, created.id).await, before);

    let issued = auth
        .complete_pending_login(&pending.challenge, &recovery.codes[0], "peer", None)
        .await
        .expect("recovery");
    assert_ne!(issued.token, pending.challenge);
    let again = auth.login(&address, password(), "peer").await;
    let Err(AuthError::MfaPending(second)) = again else {
        panic!("second challenge");
    };
    assert!(auth
        .complete_pending_login(&second.challenge, &recovery.codes[0], "peer", None)
        .await
        .is_err());
    let totp_session = auth
        .complete_pending_login(&second.challenge, &next, "peer", None)
        .await
        .expect("next step");
    let replay = auth.login(&address, password(), "other").await;
    let Err(AuthError::MfaPending(third)) = replay else {
        panic!("third");
    };
    assert!(auth
        .complete_pending_login(&third.challenge, &next, "other", None)
        .await
        .is_err());
    let _ = totp_session;
    assert!(auth
        .complete_pending_login(&pending.challenge, &recovery.codes[1], "peer", None)
        .await
        .is_err());
}

#[tokio::test]
async fn session_revoke_reauth_and_close_respect_the_actor() {
    session_revoke_reauth_and_close_respect_the_actor_on(&service().await).await;
}

async fn session_revoke_reauth_and_close_respect_the_actor_on(auth: &AuthService) {
    let owner_email = email("owner");
    let staff_email = email("staff");
    let root_email = email("root");
    auth.create_user(&owner_email, password(), false, false)
        .await
        .expect("owner");
    auth.create_user(&staff_email, password(), true, false)
        .await
        .expect("staff");
    auth.create_user(&root_email, password(), true, true)
        .await
        .expect("root");
    let owner_issued = auth
        .login(&owner_email, password(), "owner")
        .await
        .expect("owner session");
    let owner = auth
        .authenticate_token(&owner_issued.token)
        .await
        .expect("owner actor");
    let second = auth
        .login(&owner_email, password(), "owner-2")
        .await
        .expect("second");
    let owner_again = auth
        .authenticate_token(&second.token)
        .await
        .expect("second actor");
    assert_ne!(owner.session_id, owner_again.session_id);
    let grant = auth
        .reauthenticate(&owner, password(), None)
        .await
        .expect("proof");
    assert!(!format!("{grant:?}").contains(&grant.proof));
    auth.require_reauth(&owner, &grant.proof).expect("bound");
    assert!(auth.require_reauth(&owner_again, &grant.proof).is_err());
    let now = crate::service::unix_now().expect("clock");
    let expired =
        crate::totp::reauth_proof(&auth.config().mfa_key, owner.id, owner.session_id, now);
    let too_long = crate::totp::reauth_proof(
        &auth.config().mfa_key,
        owner.id,
        owner.session_id,
        now.saturating_add(301),
    );
    assert!(auth.require_reauth(&owner, &expired).is_err());
    assert!(auth.require_reauth(&owner, &too_long).is_err());
    assert!(auth.authenticate_token(&owner_issued.token).await.is_ok());

    let staff = auth
        .authenticate_token(
            &auth
                .login(&staff_email, password(), "staff")
                .await
                .expect("staff session")
                .token,
        )
        .await
        .expect("staff actor");
    assert!(matches!(
        auth.authorize(&staff, Some("notes"), "read").await,
        Err(AuthError::Forbidden)
    ));
    assert!(matches!(
        auth.revoke_session_for(&staff, owner.session_id).await,
        Err(AuthError::Forbidden)
    ));
    let root = auth
        .authenticate_token(
            &auth
                .login(&root_email, password(), "root")
                .await
                .expect("root session")
                .token,
        )
        .await
        .expect("root actor");
    auth.revoke_session_for(&root, owner.session_id)
        .await
        .expect("superuser revoke");
    assert!(auth.authenticate_token(&owner_issued.token).await.is_err());
    let extra = auth
        .login(&owner_email, password(), "owner-3")
        .await
        .expect("extra");
    let revoked = auth
        .revoke_other_sessions(&owner_again)
        .await
        .expect("others");
    assert!(revoked >= 1);
    assert!(auth.authenticate_token(&extra.token).await.is_err());
    assert!(auth.authenticate_token(&second.token).await.is_ok());

    auth.close_account(&owner_again, password())
        .await
        .expect("close");
    assert!(matches!(
        auth.account_phase(owner.id).await,
        Ok(crate::AccountPhase::Disabled)
    ));
    assert!(matches!(
        auth.login(&owner_email, password(), "closed").await,
        Err(AuthError::InvalidCredentials)
    ));
}

#[tokio::test]
async fn login_http_returns_the_challenge_without_a_cookie() {
    login_http_returns_the_challenge_without_a_cookie_on(&service().await).await;
}

async fn login_http_returns_the_challenge_without_a_cookie_on(auth: &AuthService) {
    let address = email("http");
    let created = auth
        .create_user(&address, password(), false, false)
        .await
        .expect("user");
    let issued = auth
        .login(&address, password(), "peer")
        .await
        .expect("login");
    let user = auth.authenticate_token(&issued.token).await.expect("actor");
    auth.begin_totp(&user, password()).await.expect("enroll");
    let stored = crate::account_store::find_totp(auth.database(), created.id)
        .await
        .expect("row")
        .expect("totp");
    assert!(stored.nonce.starts_with("v2."));
    assert!(!stored.ciphertext.contains("BEGIN"));
    let (current, _) = codes(auth, created.id).await;
    auth.confirm_totp(&user, &current).await.expect("confirm");
    let app = crate::mount(Router::new()).with_state(crate::AppState {
        db: auth.database().clone(),
        auth: Some(auth.clone()),
        mail: None,
    });
    let response = app
        .oneshot(
            Request::post("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    "{{\"email\":\"{address}\",\"password\":\"{}\"}}",
                    password()
                )))
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .is_none());
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let body = String::from_utf8(bytes.to_vec()).expect("utf8");
    assert!(body.contains("auth.mfa_required"));
    assert!(body.contains("challenge"));
    assert!(!body.contains("fexu_session="));
}

async fn failed_checks_do_not_issue_sessions(auth: &AuthService) {
    let address = email("guard");
    let created = auth
        .create_user(&address, password(), false, false)
        .await
        .expect("user");
    let before = session_count(auth, created.id).await;
    assert!(matches!(
        auth.login(&address, wrong_password(), "guard").await,
        Err(AuthError::InvalidCredentials)
    ));
    assert_eq!(session_count(auth, created.id).await, before);
    let issued = auth
        .login(&address, password(), "guard")
        .await
        .expect("login");
    let user = auth.authenticate_token(&issued.token).await.expect("actor");
    let open = session_count(auth, created.id).await;
    assert!(matches!(
        auth.authorize(&user, Some("notes"), "delete").await,
        Err(AuthError::Forbidden)
    ));
    assert_eq!(session_count(auth, created.id).await, open);
    let role = format!("role{}", &created.id.simple().to_string()[..8]);
    auth.grant_permission(&role, "notes.read")
        .await
        .expect("grant");
    assert!(matches!(
        auth.authorize(&user, Some("notes"), "read").await,
        Err(AuthError::Forbidden)
    ));
    auth.assign_role(&address, &role).await.expect("assign");
    auth.authorize(&user, Some("notes"), "read")
        .await
        .expect("allowed");
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&format!("fexu_session={}", issued.token))
            .expect("cookie"),
    );
    assert!(matches!(
        auth.authenticate_csrf(&headers, &axum::http::Method::POST, "not-the-token")
            .await,
        Err(AuthError::CsrfRejected)
    ));
    assert!(auth.authenticate_token(&issued.token).await.is_ok());
    auth.authenticate_csrf(&headers, &axum::http::Method::POST, &issued.csrf_token)
        .await
        .expect("csrf");
}

#[tokio::test]
async fn failed_permission_and_csrf_checks_keep_the_session() {
    failed_checks_do_not_issue_sessions(&service().await).await;
}

#[tokio::test]
async fn new_mfa_uses_the_dedicated_key_and_keeps_recovery_codes_one_time() {
    let auth = service().await;
    let address = email("fresh");
    let created = auth
        .create_user(&address, password(), false, false)
        .await
        .expect("user");
    let user = auth
        .authenticate_token(
            &auth
                .login(&address, password(), "fresh")
                .await
                .expect("session")
                .token,
        )
        .await
        .expect("actor");
    let enrollment = auth.begin_totp(&user, password()).await.expect("enroll");
    assert!(!format!("{enrollment:?}").contains(&enrollment.secret_base32));
    let row = crate::account_store::find_totp(auth.database(), created.id)
        .await
        .expect("row")
        .expect("totp");
    assert!(row.nonce.starts_with("v2."));
    assert!(!row.ciphertext.contains(&enrollment.secret_base32));
    assert!(crate::totp::open(
        &crate::totp::legacy_mfa_key(&auth.config().csrf_secret),
        created.id,
        row.nonce.strip_prefix("v2.").unwrap(),
        &row.ciphertext,
    )
    .is_err());
    let (current, _) = codes(&auth, created.id).await;
    let recovery = auth.confirm_totp(&user, &current).await.expect("confirm");
    assert!(!format!("{recovery:?}").contains(&recovery.codes[0]));
    let hashes = crate::account_store::unused_recovery(auth.database(), created.id)
        .await
        .expect("hashes");
    assert_eq!(hashes.len(), recovery.codes.len());
    for (_, hash) in &hashes {
        assert!(hash.starts_with("v2."));
        assert!(crate::totp::recovery_format(hash) == Some(crate::totp::RecoveryFormat::Current));
        assert!(crate::totp::seal_format(hash).is_err());
        for code in &recovery.codes {
            assert!(!hash.contains(code));
        }
    }
    let pending = auth.login(&address, password(), "fresh-2").await;
    let crate::AuthError::MfaPending(challenge) = pending.expect_err("challenge") else {
        panic!("mfa challenge");
    };
    assert!(auth.authenticate_token(&challenge.challenge).await.is_err());
    let issued = auth
        .complete_pending_login(&challenge.challenge, &recovery.codes[0], "fresh-2", None)
        .await
        .expect("recovery login");
    assert!(auth.authenticate_token(&issued.token).await.is_ok());
    let again = auth.login(&address, password(), "fresh-3").await;
    let crate::AuthError::MfaPending(second) = again.expect_err("second challenge") else {
        panic!("second challenge");
    };
    assert!(auth
        .complete_pending_login(&second.challenge, &recovery.codes[0], "fresh-3", None)
        .await
        .is_err());
    auth.complete_pending_login(&second.challenge, &recovery.codes[1], "fresh-3", None)
        .await
        .expect("next code");
}

#[tokio::test]
async fn legacy_totp_migrates_and_recovery_hashes_stay_until_used() {
    let auth = service().await;
    let address = email("legacy");
    let created = auth
        .create_user(&address, password(), false, false)
        .await
        .expect("user");
    let user_id = created.id;
    let secret = crate::totp::random_secret();
    let legacy = crate::totp::legacy_mfa_key(&auth.config().csrf_secret);
    let (nonce, ciphertext) = crate::totp::seal(&legacy, user_id, &secret).expect("seal");
    let now = crate::service::unix_now().expect("clock");
    crate::account_store::replace_totp(auth.database(), user_id, &nonce, &ciphertext, now)
        .await
        .expect("store");
    crate::account_store::confirm_totp(auth.database(), user_id, now, 0)
        .await
        .expect("confirm");
    let first = "ABCDEFGHJK";
    let second = "ABCD2EFGHJ";
    let first_hash = crate::totp::recovery_hash(&legacy, user_id, first);
    let second_hash = crate::totp::recovery_hash(&legacy, user_id, second);
    crate::account_store::insert_recovery(
        auth.database(),
        Uuid::new_v4(),
        user_id,
        &first_hash,
        now,
    )
    .await
    .expect("first code");
    crate::account_store::insert_recovery(
        auth.database(),
        Uuid::new_v4(),
        user_id,
        &second_hash,
        now,
    )
    .await
    .expect("second code");
    let code = crate::totp::totp_code(&secret, now, 6).expect("code");
    let crate::AuthError::MfaPending(challenge) = auth
        .login(&address, password(), "legacy")
        .await
        .expect_err("challenge")
    else {
        panic!("challenge");
    };
    let issued = auth
        .complete_pending_login(&challenge.challenge, &code, "legacy", None)
        .await
        .expect("totp");
    let moved = crate::account_store::find_totp(auth.database(), user_id)
        .await
        .expect("row")
        .expect("totp");
    assert!(moved.nonce.starts_with("v2."));
    assert_ne!(moved.ciphertext, ciphertext);
    assert!(!moved.ciphertext.contains(&crate::totp::base32(&secret)));
    let hashes = crate::account_store::unused_recovery(auth.database(), user_id)
        .await
        .expect("hashes");
    let stored: Vec<&str> = hashes.iter().map(|(_, hash)| hash.as_str()).collect();
    assert!(stored.contains(&first_hash.as_str()));
    assert!(stored.contains(&second_hash.as_str()));
    let crate::AuthError::MfaPending(use_code) = auth
        .login(&address, password(), "legacy-code")
        .await
        .expect_err("code challenge")
    else {
        panic!("code challenge");
    };
    auth.complete_pending_login(&use_code.challenge, first, "legacy-code", None)
        .await
        .expect("first recovery");
    let crate::AuthError::MfaPending(replay) = auth
        .login(&address, password(), "legacy-replay")
        .await
        .expect_err("replay challenge")
    else {
        panic!("replay challenge");
    };
    assert!(auth
        .complete_pending_login(&replay.challenge, first, "legacy-replay", None)
        .await
        .is_err());
    let remaining = crate::account_store::unused_recovery(auth.database(), user_id)
        .await
        .expect("remaining");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].1, second_hash);

    let mut rotated = auth.config().clone();
    let mut rotated_csrf = [0u8; 32];
    OsRng.fill_bytes(&mut rotated_csrf);
    rotated.csrf_secret = rotated_csrf;
    let rotated = AuthService::new(auth.database().clone(), rotated);
    let crate::AuthError::MfaPending(after) = rotated
        .login(&address, password(), "rotated")
        .await
        .expect_err("rotated challenge")
    else {
        panic!("rotated challenge");
    };
    assert!(rotated
        .complete_pending_login(&after.challenge, second, "rotated", None)
        .await
        .is_err());
    let rotated_legacy = crate::totp::legacy_mfa_key(&rotated.config().csrf_secret);
    assert!(crate::totp::open_stored(
        &rotated.config().mfa_key,
        &rotated_legacy,
        user_id,
        &nonce,
        &ciphertext,
    )
    .is_err());
    let opened = crate::totp::open_stored(
        &rotated.config().mfa_key,
        &rotated_legacy,
        user_id,
        &moved.nonce,
        &moved.ciphertext,
    )
    .expect("current seal");
    assert_eq!(opened, secret);
    let actor = rotated
        .authenticate_token(&issued.token)
        .await
        .expect("actor");
    let used_step = now.div_euclid(30);
    let following = crate::totp::totp_code(&secret, (used_step + 1) * 30, 6).expect("following");
    let proof = rotated
        .reauthenticate(&actor, password(), Some(&following))
        .await
        .expect("reauth");
    rotated
        .require_reauth(&actor, &proof.proof)
        .expect("v2 proof");
    let legacy_proof = crate::totp::legacy_reauth_proof(
        &legacy,
        actor.id,
        actor.session_id,
        crate::service::unix_now()
            .expect("clock")
            .saturating_add(60),
    );
    assert!(rotated.require_reauth(&actor, &legacy_proof).is_err());
    assert_eq!(
        crate::account_store::unused_recovery(rotated.database(), user_id)
            .await
            .expect("unchanged")
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
async fn postgres_account_security_roundtrip() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    assert!(
        url.starts_with("postgres"),
        "the ignored PostgreSQL auth test refuses a non-postgres DATABASE_URL"
    );
    let db = fexu_db::connect(&url).await.expect("postgres");
    crate::apply_migrations(&db).await.expect("schema");
    let auth = AuthService::new(db, AuthConfig::development());
    password_login_stays_available_without_totp_on(&auth).await;
    confirmed_totp_blocks_session_until_the_second_factor_on(&auth).await;
    session_revoke_reauth_and_close_respect_the_actor_on(&auth).await;
    failed_checks_do_not_issue_sessions(&auth).await;
    login_http_returns_the_challenge_without_a_cookie_on(&auth).await;
}
