//! Cookie-Schreibzugriffe brauchen einen CSRF-Nachweis, bevor ein Handler läuft.
//!
//! Ohne `ferrioxid_session` oder `ferrioxid_anonymous` bleibt die Anfrage unberührt.
//! Das gilt auch für `Authorization: Bearer`: der Header wird hier nicht geprüft.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use ferrioxid_auth::{csrf_header_name, AppState, AuthError};
use ferrioxid_core::{problem_json, FerrioxidError};
use http_body_util::{BodyExt, Limited};

use super::ApiError;

const BODY_CAP: usize = 8 * 1024 * 1024;
const DEFAULT_SESSION_COOKIES: [&str; 2] = ["ferrioxid_session", "ferrioxid_anonymous"];

pub async fn enforce(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if request.method().is_safe() {
        return next.run(request).await;
    }
    let protected = match state.auth.as_ref() {
        Some(auth) => auth.request_has_session_cookie(request.headers()),
        None => default_session_cookie(request.headers()),
    };
    if !protected {
        return next.run(request).await;
    }
    let Some(auth) = state.auth.clone() else {
        return reject(AuthError::CsrfRejected, request.headers());
    };
    let header_token = header_csrf(request.headers());
    let (request, field) = if header_token.is_empty() {
        match body_field(request).await {
            Ok(pair) => pair,
            Err(BodyRead::TooLarge) => {
                return problem(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Payload too large",
                    "request body is too large",
                );
            }
            Err(BodyRead::Unreadable) => {
                return problem(
                    StatusCode::BAD_REQUEST,
                    "Bad request",
                    "request body could not be read",
                );
            }
        }
    } else {
        (request, String::new())
    };
    match auth
        .guard_cookie_write(request.headers(), request.method(), &field)
        .await
    {
        Ok(_) => next.run(request).await,
        Err(error) => reject(error, request.headers()),
    }
}

fn default_session_cookie(headers: &HeaderMap) -> bool {
    DEFAULT_SESSION_COOKIES
        .iter()
        .any(|name| cookie_named(headers, name))
}

fn cookie_named(headers: &HeaderMap, name: &str) -> bool {
    let Some(raw) = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    raw.split(';').any(|part| {
        part.trim()
            .split_once('=')
            .is_some_and(|(key, _)| key.trim() == name)
    })
}

fn header_csrf(headers: &HeaderMap) -> String {
    headers
        .get(csrf_header_name())
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim()
        .to_owned()
}

enum BodyRead {
    TooLarge,
    Unreadable,
}

async fn body_field(request: Request) -> Result<(Request, String), BodyRead> {
    let (parts, body) = request.into_parts();
    let content_type = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let bytes = match Limited::new(body, BODY_CAP).collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(error) => {
            return Err(
                if error
                    .downcast_ref::<http_body_util::LengthLimitError>()
                    .is_some()
                {
                    BodyRead::TooLarge
                } else {
                    BodyRead::Unreadable
                },
            );
        }
    };
    let field = csrf_field(&content_type, &bytes);
    Ok((Request::from_parts(parts, Body::from(bytes)), field))
}

fn csrf_field(content_type: &str, body: &[u8]) -> String {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime == "application/json" {
        return String::new();
    }
    if mime == "multipart/form-data" {
        return boundary(content_type)
            .map(|marker| multipart_csrf(body, &marker))
            .unwrap_or_default();
    }
    if mime == "application/x-www-form-urlencoded" || mime.is_empty() {
        return form_csrf(body);
    }
    String::new()
}

fn boundary(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|part| {
        let value = part.trim().strip_prefix("boundary=")?;
        let value = value.trim().trim_matches('"');
        if value.is_empty()
            || value.len() > 70
            || value
                .chars()
                .any(|char| char.is_control() || char.is_whitespace())
        {
            return None;
        }
        Some(value.to_owned())
    })
}

fn form_csrf(body: &[u8]) -> String {
    let Ok(text) = std::str::from_utf8(body) else {
        return String::new();
    };
    let mut token = String::new();
    for pair in text.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if decode(key) == "csrf_token" && token.is_empty() {
            token = bounded(decode(value));
        }
    }
    token
}

fn multipart_csrf(body: &[u8], marker: &str) -> String {
    let delimiter = format!("--{marker}").into_bytes();
    let mut rest = body;
    while let Some(start) = find_slice(rest, &delimiter) {
        rest = &rest[start + delimiter.len()..];
        if rest.starts_with(b"--") {
            break;
        }
        let rest_part = rest.strip_prefix(b"\r\n").unwrap_or(rest);
        let Some(split) = find_slice(rest_part, b"\r\n\r\n") else {
            continue;
        };
        let headers = &rest_part[..split];
        let content = &rest_part[split + 4..];
        if !is_csrf_part(headers) {
            continue;
        }
        let end = find_slice(content, b"\r\n--").unwrap_or(content.len());
        return bounded(String::from_utf8_lossy(&content[..end]).trim().to_owned());
    }
    String::new()
}

fn is_csrf_part(headers: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(headers) else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    lower.contains("name=\"csrf_token\"") || lower.contains("name='csrf_token'")
}

fn bounded(token: String) -> String {
    if token.len() > 256 {
        String::new()
    } else {
        token
    }
}

fn decode(value: &str) -> String {
    let plus = value.replace('+', " ");
    let bytes = plus.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&plus[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn find_slice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn prefers_html(headers: &HeaderMap) -> bool {
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if mime.eq_ignore_ascii_case("application/x-www-form-urlencoded")
        || mime.eq_ignore_ascii_case("multipart/form-data")
    {
        return true;
    }
    if mime.eq_ignore_ascii_case("application/json") {
        return false;
    }
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/html") && !value.contains("application/json"))
}

fn reject(error: AuthError, headers: &HeaderMap) -> Response {
    let error = match error {
        AuthError::Unauthenticated => AuthError::CsrfRejected,
        other => other,
    };
    if prefers_html(headers) && matches!(error, AuthError::CsrfRejected) {
        return html_csrf(headers);
    }
    ApiError(FerrioxidError::from(error)).into_response()
}

fn html_csrf(headers: &HeaderMap) -> Response {
    let english = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("en"));
    let sentence = if english {
        "The security check failed."
    } else {
        "Die Sicherheitsprüfung ist fehlgeschlagen."
    };
    (
        StatusCode::FORBIDDEN,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        format!("<!DOCTYPE html><html><body><p>{sentence}</p></body></html>"),
    )
        .into_response()
}

fn problem(status: StatusCode, title: &'static str, detail: &'static str) -> Response {
    (status, Json(problem_json(status.as_u16(), title, detail))).into_response()
}
