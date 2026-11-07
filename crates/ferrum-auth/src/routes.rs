//! Auth-Endpunkte. Session-Tokens bleiben im Cookie.

use crate::error::AuthError;
use crate::service::{AuthService, AuthUser, PublicUser};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;

use axum::extract::ConnectInfo;

#[derive(Clone)]
pub struct AppState {
    pub db: sea_orm::DatabaseConnection,
    pub auth: Option<AuthService>,
    pub mail: Option<ferrum_mail::MailService>,
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct PasswordBody {
    current_password: String,
    new_password: String,
}

pub fn mount(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/auth/csrf", get(csrf))
        .route("/auth/change-password", post(change_password))
        .route(
            "/auth/password-reset",
            get(crate::recovery::show_request).post(crate::recovery::submit_request),
        )
        .route(
            "/auth/password-reset/confirm",
            get(crate::recovery::confirm_token),
        )
        .route(
            "/auth/password-reset/choose",
            get(crate::recovery::show_choose).post(crate::recovery::submit_choose),
        )
}

pub struct ClientIp(pub String);

impl<S> axum::extract::FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let ip = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip().to_string())
            .unwrap_or_else(|| "unknown".to_owned());
        Ok(ClientIp(ip))
    }
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<LoginBody>,
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
    let client = auth.client_key(&headers, &peer.0);
    match auth.login(&body.email, &body.password, &client).await {
        Ok(issued) => {
            let mut response = Json(json!({
                "user": issued.user,
                "csrf_token": issued.csrf_token
            }))
            .into_response();
            if let Ok(value) = HeaderValue::from_str(&issued.cookie) {
                response
                    .headers_mut()
                    .insert(axum::http::header::SET_COOKIE, value);
            }
            response
        }
        Err(error) => auth_error(error),
    }
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> axum::response::Response {
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    let token = crate::cookie::read_cookie(&headers, &auth.config().cookie_name);
    if token.is_some() {
        match auth.authenticate(&headers, &axum::http::Method::POST).await {
            Ok(_) => {}
            Err(AuthError::Unauthenticated) => {}
            Err(error) => return auth_error(error),
        }
    }
    match auth.logout(token.as_deref()).await {
        Ok(cookie) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                response
                    .headers_mut()
                    .insert(axum::http::header::SET_COOKIE, value);
            }
            response
        }
        Err(error) => auth_error(error),
    }
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
) -> axum::response::Response {
    match current_user(&state, &headers, &axum::http::Method::GET, &peer.0).await {
        Ok(user) => Json(PublicUser {
            id: user.id,
            email: user.email,
            is_active: user.is_active,
            is_staff: user.is_staff,
            is_superuser: user.is_superuser,
        })
        .into_response(),
        Err(error) => auth_error(error),
    }
}

async fn csrf(State(state): State<AppState>, headers: HeaderMap) -> axum::response::Response {
    let Some(auth) = state.auth.as_ref() else {
        return error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "authentication is not configured",
        );
    };
    if let Err(error) = auth.presented_origin_allowed(&headers) {
        return auth_error(error);
    }
    let token = match crate::cookie::read_cookie(&headers, &auth.config().cookie_name) {
        Some(token) => token,
        None => return auth_error(AuthError::Unauthenticated),
    };
    match auth.refresh_csrf(&token).await {
        Ok(csrf_token) => Json(json!({ "csrf_token": csrf_token })).into_response(),
        Err(error) => auth_error(error),
    }
}

async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(body): Json<PasswordBody>,
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
        .change_password(&user, &body.current_password, &body.new_password)
        .await
    {
        Ok(cookie) => {
            let mut response = Json(json!({"status": "password changed"})).into_response();
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                response
                    .headers_mut()
                    .insert(axum::http::header::SET_COOKIE, value);
            }
            response
        }
        Err(error) => auth_error(error),
    }
}

pub async fn current_user(
    state: &AppState,
    headers: &HeaderMap,
    method: &axum::http::Method,
    peer: &str,
) -> Result<AuthUser, AuthError> {
    let _ = peer;
    let auth = state.auth.as_ref().ok_or(AuthError::Forbidden)?;
    auth.authenticate(headers, method).await
}

fn auth_error(error: AuthError) -> axum::response::Response {
    match error {
        AuthError::RateLimited => error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many requests",
            "Too many login attempts",
        ),
        AuthError::InvalidCredentials => error_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized",
            "Invalid email or password",
        ),
        AuthError::Unauthenticated => error_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized",
            "Authentication required",
        ),
        AuthError::Forbidden => error_response(StatusCode::FORBIDDEN, "Forbidden", "Access denied"),
        AuthError::CsrfRejected => error_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "csrf token is missing or invalid",
        ),
        AuthError::OriginRejected => {
            error_response(StatusCode::FORBIDDEN, "Forbidden", "origin is not allowed")
        }
        AuthError::Validation(message) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Validation failed",
            &message,
        ),
        AuthError::Database(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
            "Database operation failed",
        ),
    }
}

fn error_response(
    status: StatusCode,
    title: &'static str,
    detail: &str,
) -> axum::response::Response {
    (
        status,
        Json(ferrum_core::problem_json(status.as_u16(), title, detail)),
    )
        .into_response()
}

use axum::response::IntoResponse;
