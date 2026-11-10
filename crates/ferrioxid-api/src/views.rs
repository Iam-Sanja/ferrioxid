//! Kleine Antworten für Anwendungsviews. Auth bleibt im vorhandenen Dienst.

use crate::pages::{message, Site};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Json;
use ferrioxid_auth::{AuthService, AuthUser, ObjectPolicy};
use ferrioxid_core::{localize_detail, problem_json};
use ferrioxid_middleware::RequestId;
use tera::Context;

#[derive(Debug)]
pub struct ViewError {
    pub status: StatusCode,
    pub code: &'static str,
    pub detail: String,
}

impl ViewError {
    pub fn new(status: StatusCode, code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status,
            code,
            detail: detail.into(),
        }
    }
}

impl IntoResponse for ViewError {
    fn into_response(self) -> Response {
        respond(
            None,
            &HeaderMap::new(),
            None,
            self.status,
            self.code,
            &self.detail,
            false,
        )
    }
}

pub fn render(site: &Site, name: &str, context: &Context, debug: bool) -> Response {
    match site.render(name, context) {
        Ok(body) => Html(body).into_response(),
        Err(_error) => {
            let show_details = debug;
            tracing::warn!(
                show_details,
                code = "view.template",
                "template rendering failed"
            );
            respond(
                Some(site),
                &HeaderMap::new(),
                None,
                StatusCode::INTERNAL_SERVER_ERROR,
                "view.template",
                "Internal server error",
                false,
            )
        }
    }
}

pub fn json(value: &serde_json::Value) -> Response {
    Json(value).into_response()
}

pub fn redirect_to(target: &str) -> Result<Response, ViewError> {
    if !safe_redirect(target) {
        return Err(ViewError::new(
            StatusCode::BAD_REQUEST,
            "view.redirect",
            "Redirects müssen auf einen lokalen Pfad zeigen.",
        ));
    }
    Ok(Redirect::to(target).into_response())
}

pub fn fail(
    headers: &HeaderMap,
    request_id: Option<&RequestId>,
    status: StatusCode,
    code: &'static str,
    detail: &str,
    debug: bool,
) -> Response {
    fail_in(None, headers, request_id, status, code, detail, debug)
}

pub fn fail_in(
    site: Option<&Site>,
    headers: &HeaderMap,
    request_id: Option<&RequestId>,
    status: StatusCode,
    code: &'static str,
    detail: &str,
    debug: bool,
) -> Response {
    respond(site, headers, request_id, status, code, detail, debug)
}

pub async fn require_mutation(
    auth: Option<&AuthService>,
    headers: &HeaderMap,
    token: &str,
) -> Result<AuthUser, ViewError> {
    let Some(auth) = auth else {
        return Err(ViewError::new(
            StatusCode::UNAUTHORIZED,
            "auth.not_configured",
            "authentication is not configured",
        ));
    };
    auth.authenticate_csrf(headers, &axum::http::Method::POST, token)
        .await
        .map_err(from_auth)
}

pub async fn require_user(
    auth: Option<&AuthService>,
    headers: &HeaderMap,
) -> Result<AuthUser, ViewError> {
    let Some(auth) = auth else {
        return Err(ViewError::new(
            StatusCode::UNAUTHORIZED,
            "auth.not_configured",
            "authentication is not configured",
        ));
    };
    auth.authenticate(headers, &axum::http::Method::GET)
        .await
        .map_err(from_auth)
}

pub async fn require_staff(
    auth: Option<&AuthService>,
    headers: &HeaderMap,
) -> Result<AuthUser, ViewError> {
    let user = require_user(auth, headers).await?;
    if user.is_active && user.is_staff {
        Ok(user)
    } else {
        Err(ViewError::new(
            StatusCode::FORBIDDEN,
            "auth.forbidden",
            "Access denied",
        ))
    }
}

pub async fn require_permission(
    auth: Option<&AuthService>,
    headers: &HeaderMap,
    permission: &str,
) -> Result<AuthUser, ViewError> {
    let Some(auth) = auth else {
        return Err(ViewError::new(
            StatusCode::UNAUTHORIZED,
            "auth.not_configured",
            "authentication is not configured",
        ));
    };
    let user = require_user(Some(auth), headers).await?;
    match auth.holds(&user, permission).await {
        Ok(true) => Ok(user),
        Ok(false) => Err(ViewError::new(
            StatusCode::FORBIDDEN,
            "auth.forbidden",
            "Access denied",
        )),
        Err(_) => Err(ViewError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server.error",
            "Internal server error",
        )),
    }
}

pub fn require_object_read<T, P>(user: &AuthUser, object: &T) -> Result<(), ViewError>
where
    P: ObjectPolicy<T>,
{
    if P::can_read(user, object) {
        Ok(())
    } else {
        Err(ViewError::new(
            StatusCode::FORBIDDEN,
            "auth.forbidden",
            "Access denied",
        ))
    }
}

pub fn form_fields(body: &str) -> Result<Vec<(String, String)>, ViewError> {
    if body.len() > 8 * 1024 {
        return Err(ViewError::new(
            StatusCode::BAD_REQUEST,
            "request.rejected",
            "Request rejected",
        ));
    }
    let mut fields = Vec::new();
    for pair in body.split('&') {
        if pair.is_empty() {
            continue;
        }
        let Some((key, value)) = pair.split_once('=') else {
            return Err(ViewError::new(
                StatusCode::BAD_REQUEST,
                "request.rejected",
                "Request rejected",
            ));
        };
        fields.push((decode(key), decode(value)));
    }
    Ok(fields)
}

pub fn validation(headers: &HeaderMap, field: &str, detail: &str) -> Response {
    let body = format!("field:{field}: {detail}");
    respond(
        None,
        headers,
        None,
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation.field",
        &body,
        false,
    )
}

fn respond(
    site: Option<&Site>,
    headers: &HeaderMap,
    request_id: Option<&RequestId>,
    status: StatusCode,
    code: &'static str,
    detail: &str,
    debug: bool,
) -> Response {
    let public_detail = public_detail(status, detail, debug);
    let id = request_id.map(|value| value.0.as_str()).unwrap_or("");
    tracing::warn!(
        request_id = %id,
        code,
        status = status.as_u16(),
        "view response"
    );
    if wants_html(headers) {
        let locale = locale_from(headers);
        let title = message(locale, status_key(status));
        let shown = localize_detail(locale, &public_detail);
        let body = site
            .and_then(|site| {
                crate::pages::render_status_template(
                    site,
                    template_name(status),
                    locale,
                    title,
                    &shown,
                    status.as_u16(),
                )
            })
            .or_else(|| {
                crate::pages::render_installed_status(
                    template_name(status),
                    locale,
                    title,
                    &shown,
                    status.as_u16(),
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "<!DOCTYPE html><html lang=\"{locale}\"><head><meta charset=\"utf-8\"><title>{}</title></head><body><h1>{}</h1><p>{}</p></body></html>",
                    escape(title),
                    escape(title),
                    escape(&shown)
                )
            });
        (status, Html(body)).into_response()
    } else {
        (
            status,
            Json(problem_json(
                status.as_u16(),
                status_title(status),
                &public_detail,
            )),
        )
            .into_response()
    }
}

fn public_detail(status: StatusCode, detail: &str, debug: bool) -> String {
    let lower = detail.to_ascii_lowercase();
    if lower.contains("password")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("stack backtrace")
    {
        return "Request rejected".to_owned();
    }
    if status == StatusCode::INTERNAL_SERVER_ERROR && !debug {
        return "Internal server error".to_owned();
    }
    detail.to_owned()
}

fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/html") && !value.contains("application/json"))
}

fn locale_from(headers: &HeaderMap) -> &'static str {
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.to_ascii_lowercase().starts_with("en"))
        .map(|_| "en")
        .unwrap_or("de")
}

fn template_name(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "errors/400.html",
        401 => "errors/401.html",
        403 => "errors/403.html",
        404 => "errors/404.html",
        405 => "errors/405.html",
        409 => "errors/409.html",
        422 => "errors/422.html",
        _ => "errors/500.html",
    }
}

fn status_key(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        405 => "method_not_allowed",
        409 => "conflict",
        422 => "validation",
        _ => "server_error",
    }
}

fn status_title(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "Bad request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not found",
        405 => "Method not allowed",
        409 => "Conflict",
        422 => "Validation failed",
        _ => "Internal server error",
    }
}

fn from_auth(error: ferrioxid_auth::AuthError) -> ViewError {
    let (status, code, detail) = match error {
        ferrioxid_auth::AuthError::Unauthenticated => (
            StatusCode::UNAUTHORIZED,
            "auth.required",
            "Authentication required",
        ),
        ferrioxid_auth::AuthError::Forbidden => {
            (StatusCode::FORBIDDEN, "auth.forbidden", "Access denied")
        }
        ferrioxid_auth::AuthError::CsrfRejected => (
            StatusCode::FORBIDDEN,
            "auth.csrf",
            "csrf token is missing or invalid",
        ),
        ferrioxid_auth::AuthError::OriginRejected => (
            StatusCode::FORBIDDEN,
            "auth.origin",
            "origin is not allowed",
        ),
        _ => (
            StatusCode::UNAUTHORIZED,
            "auth.required",
            "Authentication required",
        ),
    };
    ViewError::new(status, code, detail)
}

fn safe_redirect(target: &str) -> bool {
    target.starts_with('/')
        && !target.starts_with("//")
        && !target.contains("://")
        && !target.contains('\\')
        && !target.contains("..")
        && !target.chars().any(|char| char.is_control())
        && target.len() <= 512
}

fn decode(value: &str) -> String {
    let value = value.replace('+', " ");
    percent_decode(&value)
}

fn percent_decode(value: &str) -> String {
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

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::{
        fail, fail_in, form_fields, redirect_to, require_mutation, require_staff, validation,
    };
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use ferrioxid_middleware::{apply, Standard};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn mutation_without_auth_stays_unauthorized() {
        let headers = axum::http::HeaderMap::new();
        let error = require_mutation(None, &headers, "token").await.unwrap_err();
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert_eq!(error.code, "auth.not_configured");
    }

    #[test]
    fn redirect_rejects_external_targets() {
        assert!(redirect_to("/home").is_ok());
        assert!(redirect_to("https://example.invalid").is_err());
        assert!(redirect_to("//example.invalid").is_err());
    }

    #[tokio::test]
    async fn responses_keep_problem_json_and_escape_html() {
        let headers = axum::http::HeaderMap::new();
        let response = validation(&headers, "name", "<script>");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("validation.field"));
        let mut html_headers = axum::http::HeaderMap::new();
        html_headers.insert("accept", "text/html".parse().unwrap());
        let page = fail(
            &html_headers,
            None,
            StatusCode::NOT_FOUND,
            "resource.not_found",
            "<script>",
            false,
        );
        let bytes = page.into_body().collect().await.unwrap().to_bytes();
        let page = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(page.contains("&lt;script&gt;"));
        assert!(!page.contains("<script>"));
        let hidden = fail(
            &headers,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            "server.error",
            "password=super-secret-value",
            true,
        );
        let bytes = hidden.into_body().collect().await.unwrap().to_bytes();
        let hidden = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(!hidden.contains("super-secret-value"));
    }

    #[tokio::test]
    async fn staff_route_and_request_id_stay_closed() {
        assert!(require_staff(None, &axum::http::HeaderMap::new())
            .await
            .is_err());
        let app = apply(
            axum::Router::new().route(
                "/closed",
                axum::routing::get(|| async { StatusCode::FORBIDDEN }),
            ),
            Standard::default(),
        );
        let response = app
            .oneshot(Request::get("/closed").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response.headers().get("x-request-id").is_some());
        assert!(form_fields("name=Ada")
            .unwrap()
            .iter()
            .any(|field| field.0 == "name"));
    }

    #[tokio::test]
    async fn custom_error_templates_hide_internal_details() {
        use axum::http::header::CONTENT_TYPE;
        let site = crate::pages::Site::from_named(
            &[
                (
                    "errors/422.html".to_owned(),
                    "<p>{{ status }} {{ detail }}</p>".to_owned(),
                ),
                (
                    "errors/500.html".to_owned(),
                    "<p>custom-down {{ detail }}</p>".to_owned(),
                ),
            ],
            &crate::urls::RouteMap::default(),
        )
        .unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("accept", "text/html".parse().unwrap());
        let page = fail_in(
            Some(&site),
            &headers,
            None,
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation.field",
            "field:title: validation.required",
            false,
        );
        assert_eq!(page.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = String::from_utf8(
            page.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(body.contains("422"), "{body}");
        assert!(
            body.contains("required") || body.contains("erforderlich"),
            "{body}"
        );
        assert!(!body.contains("SELECT"), "{body}");
        let hidden = fail_in(
            Some(&site),
            &headers,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            "server.error",
            "SELECT id FROM notes; stack backtrace",
            false,
        );
        assert_eq!(hidden.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let hidden_body = String::from_utf8(
            hidden
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(hidden_body.contains("custom-down"), "{hidden_body}");
        assert!(!hidden_body.contains("SELECT"), "{hidden_body}");
        assert!(!hidden_body.to_ascii_lowercase().contains("stack backtrace"));
        let json_headers = axum::http::HeaderMap::new();
        let api = fail_in(
            Some(&site),
            &json_headers,
            None,
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation.field",
            "field:title: validation.required",
            false,
        );
        assert_eq!(api.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(api
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .contains("application/json"));
    }

    #[tokio::test]
    async fn fail_renders_the_installed_project_template() {
        use axum::http::header::CONTENT_TYPE;
        let site = crate::pages::Site::from_named(
            &[
                (
                    "errors/404.html".to_owned(),
                    "<p>PROJECT-404 {{ not_found }} {{ app_name }}</p>".to_owned(),
                ),
                (
                    "errors/500.html".to_owned(),
                    "<p>PROJECT-500 {{ detail }}</p>".to_owned(),
                ),
            ],
            &crate::urls::RouteMap::default(),
        )
        .unwrap();
        let _guard = crate::pages::install_error_site(
            std::sync::Arc::new(site),
            crate::pages::ErrorLabels {
                app_name: "TestShop".into(),
                project_name: "TestShop".into(),
                site_title: "TestShop".into(),
                primary_color: "#112233".into(),
                admin: false,
                debug: false,
            },
        );
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("accept", "text/html".parse().unwrap());
        headers.insert("accept-language", "de".parse().unwrap());
        let page = fail(
            &headers,
            None,
            StatusCode::NOT_FOUND,
            "resource.missing",
            "gone",
            false,
        );
        let body = String::from_utf8(
            page.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(body.contains("PROJECT-404"), "{body}");
        assert!(body.contains("Diese Seite existiert nicht."), "{body}");
        assert!(body.contains("TestShop"), "{body}");
        let hidden = fail(
            &headers,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            "server.error",
            "SELECT id FROM notes",
            false,
        );
        let hidden_body = String::from_utf8(
            hidden
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(hidden_body.contains("PROJECT-500"), "{hidden_body}");
        assert!(
            hidden_body.contains("Internal server error"),
            "{hidden_body}"
        );
        assert!(!hidden_body.contains("SELECT"), "{hidden_body}");
        let api = fail(
            &axum::http::HeaderMap::new(),
            None,
            StatusCode::NOT_FOUND,
            "resource.missing",
            "gone",
            false,
        );
        assert!(api
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .contains("application/json"));
        let api_body =
            String::from_utf8(api.into_body().collect().await.unwrap().to_bytes().to_vec())
                .unwrap();
        assert!(!api_body.contains("PROJECT-404"), "{api_body}");
        let fallback = fail(
            &headers,
            None,
            StatusCode::FORBIDDEN,
            "auth.forbidden",
            "no",
            false,
        );
        let fallback_body = String::from_utf8(
            fallback
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(fallback_body.contains("<h1>"), "{fallback_body}");
        assert!(!fallback_body.contains("PROJECT-404"), "{fallback_body}");
    }
}
