//! Passwort zurücksetzen. Der Roh-Token steht nur im Speicher und in der E-Mail.

use crate::cookie::{read_cookie, set_cookie};
use crate::error::AuthError;
use crate::password::{hash_password, validate_new_password, verify_password};
use crate::routes::{AppState, ClientIp};
use crate::service::{normalize_email, unix_now, AuthService};
use crate::store;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use ferrioxid_mail::{MailError, MailService, Notice};
use sea_orm::TransactionTrait;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

const RESET_COOKIE: &str = "ferrioxid_reset";
const CSRF_COOKIE: &str = "ferrioxid_reset_csrf";
const ACK_DE: &str = "Wenn ein Konto mit dieser Adresse existiert, wurde eine E-Mail mit weiteren Schritten angefordert.";
const ACK_EN: &str =
    "If an account with this address exists, an email with further steps was requested.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetAcknowledgement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedExchange {
    pub secret: String,
    pub cookie: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompleteError {
    Expired,
    Mismatch,
    Weak,
    Reused,
}

pub async fn request_reset(
    auth: &AuthService,
    mail: Option<&MailService>,
    email: &str,
    client: &str,
    locale: &str,
) -> ResetAcknowledgement {
    let email_key = email.trim().to_lowercase();
    let ip_limited = auth
        .note_attempt(
            &format!("reset-ip:{client}"),
            auth.config().login_max_attempts.saturating_mul(4),
        )
        .unwrap_or(true);
    let mail_limited = auth
        .note_attempt(
            &format!("reset-mail:{email_key}"),
            auth.config().login_max_attempts,
        )
        .unwrap_or(true);
    if ip_limited || mail_limited {
        tracing::info!(code = "reset.limited", "password reset request was limited");
        return ResetAcknowledgement;
    }
    let Ok(email) = normalize_email(email) else {
        return ResetAcknowledgement;
    };
    let user = match store::find_user_by_email(auth.database(), &email).await {
        Ok(user) => user,
        Err(error) => {
            tracing::warn!(code = "reset.unavailable", error = %error, "password reset lookup failed");
            return ResetAcknowledgement;
        }
    };
    let Some(user) = user.filter(|user| user.is_active) else {
        return ResetAcknowledgement;
    };
    let Some(mail) = mail.filter(|mail| mail.can_send()) else {
        tracing::info!(
            code = "mail.disabled",
            "password reset has no mail transport"
        );
        return ResetAcknowledgement;
    };
    if !trusted_origin(mail.public_origin()) {
        tracing::warn!(
            code = "mail.invalid",
            "password reset origin is not configured"
        );
        return ResetAcknowledgement;
    }
    if sweep(auth).await.is_err() {
        return ResetAcknowledgement;
    }
    let now = match unix_now() {
        Ok(now) => now,
        Err(_) => return ResetAcknowledgement,
    };
    let token = random_token();
    let reset_id = Uuid::new_v4();
    let outbox_id = Uuid::new_v4();
    let expires_at = now.saturating_add(mail.reset_ttl_secs());
    let issued = async {
        let transaction = auth.database().begin().await.map_err(AuthError::database)?;
        let active = store::active_password_resets(&transaction, user.id, now).await?;
        let overflow = active.len().saturating_sub(2);
        for (id, _) in active.into_iter().take(overflow) {
            store::consume_password_reset(&transaction, id, now).await?;
        }
        store::insert_password_reset(
            &transaction,
            reset_id,
            user.id,
            &digest(&token),
            now,
            expires_at,
        )
        .await?;
        store::insert_outbox(
            &transaction,
            outbox_id,
            "password_reset",
            &user.email,
            Some(reset_id),
            now,
        )
        .await?;
        transaction.commit().await.map_err(AuthError::database)?;
        Ok::<(), AuthError>(())
    }
    .await;
    if issued.is_err() {
        tracing::warn!(
            code = "reset.unavailable",
            "password reset token was not stored"
        );
        return ResetAcknowledgement;
    }
    let url = format!(
        "{}/auth/password-reset/confirm?token={token}",
        mail.public_origin().trim_end_matches('/')
    );
    let minutes = (mail.reset_ttl_secs() / 60).clamp(5, 1440) as u32;
    let sent = mail
        .notify(Notice::PasswordResetRequested {
            to: user.email.clone(),
            locale: if locale == "en" {
                "en".into()
            } else {
                "de".into()
            },
            reset_url: url,
            minutes,
        })
        .await;
    match sent {
        Ok(_) => {
            let _ = store::finish_outbox(auth.database(), outbox_id, Some(now), None, None).await;
            tracing::info!(
                backend = mail.backend_name(),
                "password reset mail accepted"
            );
        }
        Err(MailError::Timeout) => {
            let _ =
                store::finish_outbox(auth.database(), outbox_id, None, None, Some("mail.timeout"))
                    .await;
            tracing::warn!(code = "mail.timeout", "password reset delivery is unknown");
        }
        Err(error) => {
            let _ = store::consume_password_reset(auth.database(), reset_id, now).await;
            let _ = store::finish_outbox(
                auth.database(),
                outbox_id,
                None,
                Some(now),
                Some(error_code(error)),
            )
            .await;
            tracing::warn!(
                code = error_code(error),
                "password reset mail was not accepted"
            );
        }
    }
    ResetAcknowledgement
}

pub async fn exchange_token(
    auth: &AuthService,
    raw_token: &str,
) -> Result<IssuedExchange, CompleteError> {
    if !valid_token_shape(raw_token) {
        return Err(CompleteError::Expired);
    }
    let now = unix_now().map_err(|_| CompleteError::Expired)?;
    let Some(reset) = store::find_password_reset_by_hash(auth.database(), &digest(raw_token))
        .await
        .map_err(|_| CompleteError::Expired)?
    else {
        return Err(CompleteError::Expired);
    };
    if reset.used_at.is_some() || reset.expires_at <= now {
        return Err(CompleteError::Expired);
    }
    let secret = random_token();
    let expires_at = now.saturating_add(10 * 60).min(reset.expires_at);
    store::insert_reset_exchange(
        auth.database(),
        Uuid::new_v4(),
        reset.id,
        &digest(&secret),
        now,
        expires_at,
    )
    .await
    .map_err(|_| CompleteError::Expired)?;
    let max_age = (expires_at - now).max(1);
    Ok(IssuedExchange {
        cookie: reset_cookie(auth.config(), RESET_COOKIE, &secret, max_age),
        secret,
    })
}

pub async fn complete_reset(
    auth: &AuthService,
    mail: Option<&MailService>,
    secret: &str,
    password: &str,
    confirmation: &str,
    locale: &str,
) -> Result<(), CompleteError> {
    if password != confirmation {
        return Err(CompleteError::Mismatch);
    }
    validate_new_password(password).map_err(|_| CompleteError::Weak)?;
    let now = unix_now().map_err(|_| CompleteError::Expired)?;
    let Some((_, exchange_expires, reset)) = store::find_exchange(auth.database(), &digest(secret))
        .await
        .map_err(|_| CompleteError::Expired)?
    else {
        return Err(CompleteError::Expired);
    };
    if exchange_expires <= now || reset.used_at.is_some() || reset.expires_at <= now {
        return Err(CompleteError::Expired);
    }
    let row = store::find_user_by_id(auth.database(), reset.user_id)
        .await
        .map_err(|_| CompleteError::Expired)?
        .ok_or(CompleteError::Expired)?;
    if !row.is_active {
        return Err(CompleteError::Expired);
    }
    if verify_password(password, &row.password_hash).unwrap_or(false) {
        return Err(CompleteError::Reused);
    }
    let password_hash = hash_password(password).map_err(|_| CompleteError::Weak)?;
    let email = row.email.clone();
    let user_id = row.id;
    let transaction = auth
        .database()
        .begin()
        .await
        .map_err(|_| CompleteError::Expired)?;
    let consumed = store::consume_password_reset(&transaction, reset.id, now)
        .await
        .map_err(|_| CompleteError::Expired)?;
    if !consumed {
        let _ = transaction.rollback().await;
        return Err(CompleteError::Expired);
    }
    store::update_password(&transaction, user_id, &password_hash, now)
        .await
        .map_err(|_| CompleteError::Expired)?;
    store::revoke_user_sessions(&transaction, user_id, now)
        .await
        .map_err(|_| CompleteError::Expired)?;
    store::invalidate_other_resets(&transaction, user_id, reset.id, now)
        .await
        .map_err(|_| CompleteError::Expired)?;
    store::delete_user_exchanges(&transaction, user_id)
        .await
        .map_err(|_| CompleteError::Expired)?;
    store::insert_password_audit(
        &transaction,
        user_id,
        "password reset completed",
        Uuid::new_v4(),
        now,
    )
    .await
    .map_err(|_| CompleteError::Expired)?;
    transaction
        .commit()
        .await
        .map_err(|_| CompleteError::Expired)?;
    if let Some(mail) = mail.filter(|mail| mail.can_send()) {
        let outbox_id = Uuid::new_v4();
        let _ = store::insert_outbox(
            auth.database(),
            outbox_id,
            "password_changed",
            &email,
            None,
            now,
        )
        .await;
        match mail
            .notify(Notice::PasswordChanged {
                to: email,
                locale: if locale == "en" {
                    "en".into()
                } else {
                    "de".into()
                },
            })
            .await
        {
            Ok(_) => {
                let _ =
                    store::finish_outbox(auth.database(), outbox_id, Some(now), None, None).await;
            }
            Err(error) => {
                let _ = store::finish_outbox(
                    auth.database(),
                    outbox_id,
                    None,
                    Some(now),
                    Some(error_code(error)),
                )
                .await;
            }
        }
    }
    Ok(())
}

pub async fn show_request(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let locale = locale_from(&headers, None);
    let csrf = random_token();
    let config = cookie_config(state.auth.as_ref());
    html_response(
        StatusCode::OK,
        &state,
        &page(locale, "request", None, Some(&csrf)),
        vec![reset_cookie(&config, CSRF_COOKIE, &csrf, 30 * 60)],
    )
}

pub async fn submit_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    body: Bytes,
) -> Response {
    let locale = locale_from(&headers, None);
    let Some(auth) = state.auth.as_ref() else {
        return missing_auth();
    };
    if body.len() > 8 * 1024 {
        return forbidden_page(locale);
    }
    let form = form_pairs(&String::from_utf8_lossy(&body));
    let locale = locale_from(&headers, form.get("locale").map(String::as_str));
    if auth
        .origin_allowed(&headers, &axum::http::Method::POST)
        .is_err()
        || !csrf_matches(&headers, form.get("csrf_token").map(String::as_str))
    {
        return forbidden_page(locale);
    }
    let client = auth.client_key(&headers, &peer.0);
    request_reset(
        auth,
        state.mail.as_ref(),
        form.get("email").map(String::as_str).unwrap_or(""),
        &client,
        locale,
    )
    .await;
    html_response(
        StatusCode::OK,
        &state,
        &page(locale, "accepted", None, None),
        Vec::new(),
    )
}

pub async fn confirm_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
) -> Response {
    let locale = locale_from(&headers, query.get("lang").map(String::as_str));
    let Some(auth) = state.auth.as_ref() else {
        return missing_auth();
    };
    let token = query.get("token").map(String::as_str).unwrap_or("");
    match exchange_token(auth, token).await {
        Ok(issued) => {
            let mut response = Redirect::to("/auth/password-reset/choose").into_response();
            secure_headers(&mut response, &state);
            if let Ok(value) = HeaderValue::from_str(&issued.cookie) {
                response.headers_mut().append(header::SET_COOKIE, value);
            }
            response
        }
        Err(_) => html_response(
            StatusCode::BAD_REQUEST,
            &state,
            &page(locale, "expired", None, None),
            Vec::new(),
        ),
    }
}

pub async fn show_choose(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let locale = locale_from(&headers, None);
    let Some(auth) = state.auth.as_ref() else {
        return missing_auth();
    };
    if exchange_open(auth, &headers).await.is_err() {
        return html_response(
            StatusCode::BAD_REQUEST,
            &state,
            &page(locale, "expired", None, None),
            Vec::new(),
        );
    }
    let csrf = random_token();
    html_response(
        StatusCode::OK,
        &state,
        &page(locale, "choose", None, Some(&csrf)),
        vec![reset_cookie(auth.config(), CSRF_COOKIE, &csrf, 30 * 60)],
    )
}

pub async fn submit_choose(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(auth) = state.auth.as_ref() else {
        return missing_auth();
    };
    if body.len() > 8 * 1024 {
        return forbidden_page("de");
    }
    let form = form_pairs(&String::from_utf8_lossy(&body));
    let locale = locale_from(&headers, form.get("locale").map(String::as_str));
    if auth
        .origin_allowed(&headers, &axum::http::Method::POST)
        .is_err()
        || !csrf_matches(&headers, form.get("csrf_token").map(String::as_str))
    {
        return forbidden_page(locale);
    }
    let Some(secret) = read_cookie(&headers, RESET_COOKIE) else {
        return html_response(
            StatusCode::BAD_REQUEST,
            &state,
            &page(locale, "expired", None, None),
            Vec::new(),
        );
    };
    match complete_reset(
        auth,
        state.mail.as_ref(),
        &secret,
        form.get("password").map(String::as_str).unwrap_or(""),
        form.get("password_confirm")
            .map(String::as_str)
            .unwrap_or(""),
        locale,
    )
    .await
    {
        Ok(()) => html_response(
            StatusCode::OK,
            &state,
            &page(locale, "done", None, None),
            vec![
                reset_cookie(auth.config(), RESET_COOKIE, "", 0),
                reset_cookie(auth.config(), CSRF_COOKIE, "", 0),
            ],
        ),
        Err(CompleteError::Mismatch) => choose_again(&state, auth, locale, "mismatch"),
        Err(CompleteError::Weak) => choose_again(&state, auth, locale, "weak"),
        Err(CompleteError::Reused) => choose_again(&state, auth, locale, "reused"),
        Err(CompleteError::Expired) => html_response(
            StatusCode::BAD_REQUEST,
            &state,
            &page(locale, "expired", None, None),
            vec![reset_cookie(auth.config(), RESET_COOKIE, "", 0)],
        ),
    }
}

async fn exchange_open(auth: &AuthService, headers: &HeaderMap) -> Result<(), CompleteError> {
    let secret = read_cookie(headers, RESET_COOKIE).ok_or(CompleteError::Expired)?;
    let now = unix_now().map_err(|_| CompleteError::Expired)?;
    let Some((_, expires, reset)) = store::find_exchange(auth.database(), &digest(&secret))
        .await
        .map_err(|_| CompleteError::Expired)?
    else {
        return Err(CompleteError::Expired);
    };
    if expires <= now || reset.used_at.is_some() || reset.expires_at <= now {
        Err(CompleteError::Expired)
    } else {
        Ok(())
    }
}

async fn sweep(auth: &AuthService) -> Result<(), AuthError> {
    let now = unix_now()?;
    let stale = store::stale_outbox(auth.database(), now.saturating_sub(120)).await?;
    for (outbox_id, reset_id) in stale {
        if let Some(reset_id) = reset_id {
            store::consume_password_reset(auth.database(), reset_id, now).await?;
        }
        store::finish_outbox(
            auth.database(),
            outbox_id,
            None,
            Some(now),
            Some("mail.expired"),
        )
        .await?;
    }
    Ok(())
}

fn choose_again(state: &AppState, auth: &AuthService, locale: &str, error: &str) -> Response {
    let csrf = random_token();
    html_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        state,
        &page(locale, "choose", Some(error), Some(&csrf)),
        vec![reset_cookie(auth.config(), CSRF_COOKIE, &csrf, 30 * 60)],
    )
}

fn html_response(
    status: StatusCode,
    state: &AppState,
    body: &str,
    cookies: Vec<String>,
) -> Response {
    let mut response = (status, Html(body.to_owned())).into_response();
    secure_headers(&mut response, state);
    for cookie in cookies {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

fn secure_headers(response: &mut Response, state: &AppState) {
    let headers = response.headers_mut();
    let _ = state;
    insert(headers, header::REFERRER_POLICY, "no-referrer");
    insert(headers, header::CACHE_CONTROL, "no-store");
    insert(headers, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    insert(
        headers,
        header::CONTENT_SECURITY_POLICY,
        "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
    );
}

fn insert(headers: &mut HeaderMap, name: header::HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
    }
}

fn forbidden_page(locale: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Html(page(locale, "forbidden", None, None)),
    )
        .into_response()
}

fn missing_auth() -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(page("de", "missing", None, None)),
    )
        .into_response()
}

fn cookie_config(auth: Option<&AuthService>) -> crate::AuthConfig {
    auth.map(|auth| auth.config().clone())
        .unwrap_or_else(crate::AuthConfig::development)
}

fn reset_cookie(config: &crate::AuthConfig, name: &str, value: &str, max_age: i64) -> String {
    let mut cookie = set_cookie(config, value, max_age);
    cookie = cookie.replacen(&config.cookie_name, name, 1);
    cookie.replace("Path=/", "Path=/auth/password-reset")
}

fn csrf_matches(headers: &HeaderMap, field: Option<&str>) -> bool {
    let Some(field) = field else {
        return false;
    };
    let Some(cookie) = read_cookie(headers, CSRF_COOKIE) else {
        return false;
    };
    constant_eq(&cookie, field)
}

fn locale_from(headers: &HeaderMap, chosen: Option<&str>) -> &'static str {
    if chosen == Some("en") {
        return "en";
    }
    if chosen == Some("de") {
        return "de";
    }
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.to_ascii_lowercase().starts_with("en"))
        .map(|_| "en")
        .unwrap_or("de")
}

fn form_pairs(body: &str) -> HashMap<String, String> {
    let mut pairs = HashMap::new();
    for pair in body.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        pairs.insert(decode(key), decode(value));
    }
    pairs
}

fn decode(value: &str) -> String {
    let value = value.replace('+', " ");
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                output.push(byte);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn trusted_origin(origin: &str) -> bool {
    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };
    if rest.contains('@') || rest.contains('/') || rest.contains('?') || rest.contains('#') {
        return false;
    }
    match scheme {
        "http" => {
            let host = rest.split(':').next().unwrap_or("");
            host == "127.0.0.1" || host == "localhost"
        }
        "https" => {
            let host = rest.split(':').next().unwrap_or("");
            !host.is_empty()
                && !host
                    .chars()
                    .any(|char| char.is_control() || char.is_whitespace())
        }
        _ => false,
    }
}

fn valid_token_shape(token: &str) -> bool {
    (32..=128).contains(&token.len())
        && token
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '-' || char == '_')
}

fn error_code(error: MailError) -> &'static str {
    match error {
        MailError::Disabled => "mail.disabled",
        MailError::Invalid => "mail.invalid",
        MailError::Unavailable => "mail.unavailable",
        MailError::Timeout => "mail.timeout",
        MailError::Rejected => "mail.rejected",
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("entropy");
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0b11) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((second & 0b1111) << 2) | (third >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            output.push(TABLE[(third & 0b111111) as usize] as char);
        }
    }
    output
}

fn digest(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in left.iter().zip(right.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn page(locale: &str, kind: &str, error: Option<&str>, csrf: Option<&str>) -> String {
    let english = locale == "en";
    let title = match kind {
        "accepted" if english => "Check your email",
        "accepted" => "E-Mail prüfen",
        "choose" if english => "Choose a new password",
        "choose" => "Neues Passwort wählen",
        "done" if english => "Password changed",
        "done" => "Passwort geändert",
        "expired" if english => "Link not valid",
        "expired" => "Link ungültig",
        "forbidden" if english => "Request rejected",
        "forbidden" => "Anfrage abgelehnt",
        "missing" => "Nicht gefunden",
        "request" if english => "Reset password",
        _ => "Passwort zurücksetzen",
    };
    let body = match kind {
        "request" => request_form(english, csrf.unwrap_or("")),
        "accepted" => format!("<p>{}</p>", if english { ACK_EN } else { ACK_DE }),
        "choose" => choose_form(english, csrf.unwrap_or(""), error),
        "done" => format!(
            "<p>{}</p>",
            escape(if english {
                "The password was changed. Existing sessions are no longer valid."
            } else {
                "Das Passwort wurde geändert. Bestehende Sitzungen sind nicht mehr gültig."
            })
        ),
        "expired" => format!(
            "<p>{}</p><p><a href=\"/auth/password-reset\">{}</a></p>",
            escape(if english {
                "This link is expired, already used, or unknown."
            } else {
                "Dieser Link ist abgelaufen, bereits verwendet oder unbekannt."
            }),
            escape(if english {
                "Request a new link"
            } else {
                "Neuen Link anfordern"
            })
        ),
        "forbidden" => format!(
            "<p>{}</p>",
            escape(if english {
                "The form could not be confirmed."
            } else {
                "Das Formular konnte nicht bestätigt werden."
            })
        ),
        _ => "<p>Authentifizierung ist nicht konfiguriert.</p>".to_owned(),
    };
    format!(
        r#"<!DOCTYPE html><html lang="{locale}" data-theme="system"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title}</title><style>{style}</style></head><body><main><h1>{title}</h1>{body}</main></body></html>"#,
        style = STYLE
    )
}

fn request_form(english: bool, csrf: &str) -> String {
    let email = if english { "Email" } else { "E-Mail" };
    let submit = if english {
        "Request email"
    } else {
        "E-Mail anfordern"
    };
    format!(
        r#"<form method="post" action="/auth/password-reset"><input type="hidden" name="csrf_token" value="{csrf}"><input type="hidden" name="locale" value="{locale}"><label for="email">{email}</label><input id="email" name="email" type="email" autocomplete="username" required maxlength="254"><button type="submit">{submit}</button></form>"#,
        csrf = escape(csrf),
        locale = if english { "en" } else { "de" },
        email = escape(email),
        submit = escape(submit),
    )
}

fn choose_form(english: bool, csrf: &str, error: Option<&str>) -> String {
    let message = match error {
        Some("mismatch") if english => "The passwords do not match.",
        Some("mismatch") => "Die Passwörter stimmen nicht überein.",
        Some("weak") if english => "Use 12 to 128 characters.",
        Some("weak") => "Verwende 12 bis 128 Zeichen.",
        Some("reused") if english => "Choose a different password.",
        Some("reused") => "Wähle ein anderes Passwort.",
        _ => "",
    };
    let alert = if message.is_empty() {
        String::new()
    } else {
        format!(
            r#"<p role="alert">{message}</p>"#,
            message = escape(message)
        )
    };
    format!(
        r#"{alert}<form method="post" action="/auth/password-reset/choose"><input type="hidden" name="csrf_token" value="{csrf}"><input type="hidden" name="locale" value="{locale}"><label for="password">{password}</label><input id="password" name="password" type="password" autocomplete="new-password" required minlength="12" maxlength="128" aria-invalid="{invalid}"><label for="password_confirm">{confirm}</label><input id="password_confirm" name="password_confirm" type="password" autocomplete="new-password" required minlength="12" maxlength="128"><button type="submit">{submit}</button></form>"#,
        csrf = escape(csrf),
        locale = if english { "en" } else { "de" },
        password = escape(if english {
            "New password"
        } else {
            "Neues Passwort"
        }),
        confirm = escape(if english {
            "Repeat password"
        } else {
            "Passwort wiederholen"
        }),
        submit = escape(if english {
            "Save password"
        } else {
            "Passwort speichern"
        }),
        invalid = if error.is_some() { "true" } else { "false" },
    )
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = r#":root{color-scheme:light;--ink:#142033;--muted:#526074;--line:#6d7b90;--paper:#f4f7fb;--card:#fff;--action:#0f172a;--action-ink:#fff;--focus:#0f766e;--danger:#9f1239;--font:"Segoe UI",sans-serif}*{box-sizing:border-box}body{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--paper);color:var(--ink);font-family:var(--font)}main{width:min(100%,28rem);margin:1.25rem;padding:1.75rem;background:var(--card);border:1px solid var(--line);border-radius:1rem}h1{margin:0 0 1rem;font-size:1.6rem}label{display:block;margin:1rem 0 .4rem}input{width:100%;min-height:2.5rem;padding:.5rem .75rem;border:1px solid var(--line);border-radius:.6rem;background:var(--card);color:var(--ink)}button{margin-top:1.25rem;min-height:2.5rem;padding:.5rem 1rem;border:0;border-radius:.6rem;background:var(--action);color:var(--action-ink)}a{color:var(--focus)}p[role="alert"]{color:var(--danger)}:focus-visible{outline:3px solid var(--focus);outline-offset:2px}@media (prefers-color-scheme:dark){:root{color-scheme:dark;--ink:#e8eef8;--muted:#b7c3d6;--line:#8aa0bd;--paper:#0b1220;--card:#172234;--action:#e8eef8;--action-ink:#0f172a;--focus:#5eead4;--danger:#fb7185}}"#;
