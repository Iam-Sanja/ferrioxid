//! HTTP für den zweiten Faktor. Die Sitzung entsteht erst nach dem Code.

use crate::routes::{append_cookie, auth_error, current_user, error_response, AppState, ClientIp};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub(crate) struct PasswordOnly {
    password: String,
}

#[derive(Deserialize)]
pub(crate) struct FactorBody {
    code: String,
}

#[derive(Deserialize)]
pub(crate) struct PasswordFactorBody {
    password: String,
    code: String,
}

#[derive(Deserialize)]
pub(crate) struct CompleteBody {
    challenge: String,
    code: String,
}

#[derive(Deserialize)]
pub(crate) struct RevokeBody {
    session_id: uuid::Uuid,
}

#[derive(Deserialize)]
pub(crate) struct ReauthBody {
    password: String,
    code: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct CloseBody {
    password: String,
}

pub(crate) async fn enroll_totp(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<PasswordOnly>,
) -> axum::response::Response {
    let user = match current_user(&state, &headers, &axum::http::Method::POST, &peer.0).await {
        Ok(user) => user,
        Err(error) => return auth_error(error),
    };
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    match auth.begin_totp(&user, &body.password).await {
        Ok(enrollment) => Json(json!({
            "secret": enrollment.secret_base32,
            "otpauth_uri": enrollment.otpauth_uri
        }))
        .into_response(),
        Err(error) => auth_error(error),
    }
}

pub(crate) async fn confirm_totp(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<FactorBody>,
) -> axum::response::Response {
    let user = match current_user(&state, &headers, &axum::http::Method::POST, &peer.0).await {
        Ok(user) => user,
        Err(error) => return auth_error(error),
    };
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    match auth.confirm_totp(&user, &body.code).await {
        Ok(codes) => Json(json!({ "recovery_codes": codes.codes })).into_response(),
        Err(error) => auth_error(error),
    }
}

pub(crate) async fn disable_totp(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<PasswordFactorBody>,
) -> axum::response::Response {
    let user = match current_user(&state, &headers, &axum::http::Method::POST, &peer.0).await {
        Ok(user) => user,
        Err(error) => return auth_error(error),
    };
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    match auth.disable_totp(&user, &body.password, &body.code).await {
        Ok(()) => Json(json!({ "status": "totp disabled" })).into_response(),
        Err(error) => auth_error(error),
    }
}

pub(crate) async fn complete_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<CompleteBody>,
) -> axum::response::Response {
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    if let Err(error) = auth.origin_allowed(&headers, &axum::http::Method::POST) {
        return auth_error(error);
    }
    let anonymous = crate::cookie::read_cookie(&headers, &auth.config().anonymous_cookie_name);
    match auth
        .complete_pending_login(
            &body.challenge,
            &body.code,
            &auth.client_key(&headers, &peer.0),
            anonymous.as_deref(),
        )
        .await
    {
        Ok(issued) => {
            let mut response = Json(json!({
                "user": issued.user,
                "csrf_token": issued.csrf_token
            }))
            .into_response();
            append_cookie(&mut response, &issued.cookie);
            for cookie in &issued.clear_cookies {
                append_cookie(&mut response, cookie);
            }
            response
        }
        Err(error) => auth_error(error),
    }
}

pub(crate) async fn revoke_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<RevokeBody>,
) -> axum::response::Response {
    let user = match current_user(&state, &headers, &axum::http::Method::POST, &peer.0).await {
        Ok(user) => user,
        Err(error) => return auth_error(error),
    };
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    match auth.revoke_session_for(&user, body.session_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => auth_error(error),
    }
}

pub(crate) async fn reauthenticate(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<ReauthBody>,
) -> axum::response::Response {
    let user = match current_user(&state, &headers, &axum::http::Method::POST, &peer.0).await {
        Ok(user) => user,
        Err(error) => return auth_error(error),
    };
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    match auth
        .reauthenticate(&user, &body.password, body.code.as_deref())
        .await
    {
        Ok(grant) => Json(json!({
            "proof": grant.proof,
            "expires_at": grant.expires_at
        }))
        .into_response(),
        Err(error) => auth_error(error),
    }
}

pub(crate) async fn close_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<CloseBody>,
) -> axum::response::Response {
    let user = match current_user(&state, &headers, &axum::http::Method::POST, &peer.0).await {
        Ok(user) => user,
        Err(error) => return auth_error(error),
    };
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    match auth.close_account(&user, &body.password).await {
        Ok(()) => {
            let mut response = Json(json!({ "status": "account closed" })).into_response();
            if let Ok(cookie) = auth.logout(None).await {
                append_cookie(&mut response, &cookie);
            }
            response
        }
        Err(error) => auth_error(error),
    }
}

use axum::response::IntoResponse;
