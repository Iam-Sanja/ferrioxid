//! Eingebettete Admin-Dateien. Es gibt keinen Zugriff auf das Dateisystem des Projekts.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;

const CSS: &str = include_str!("../static/css/admin.css");
const JS: &str = include_str!("../static/js/admin.js");

pub async fn asset(path: String) -> Response {
    match path.as_str() {
        "admin.css" => bytes("text/css; charset=utf-8", CSS.as_bytes()),
        "admin.js" => bytes("text/javascript; charset=utf-8", JS.as_bytes()),
        _ => text(StatusCode::NOT_FOUND, "Nicht gefunden"),
    }
}

fn bytes(content_type: &'static str, bytes: &[u8]) -> Response {
    let mut response = Response::new(Body::from(bytes.to_vec()));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

fn text(status: StatusCode, message: &str) -> Response {
    let mut response = Response::new(Body::from(message.to_owned()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}
