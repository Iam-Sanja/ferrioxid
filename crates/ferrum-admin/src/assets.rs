//! Liefert das Admin-Frontend aus dem Binary oder aus `FERRUM_ADMIN_DIST`.
//! Node.js läuft dabei nicht mit.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use include_dir::{include_dir, Dir};
use std::path::{Component, Path, PathBuf};

static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded-admin");

/// Expliziter Entwicklungs-Override. Ohne diese Variable dient das eingebettete Frontend.
pub fn dist_dir() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var("FERRUM_ADMIN_DIST").ok()?);
    path.is_dir().then_some(path)
}

pub async fn index() -> Response {
    serve("index.html")
}

pub async fn asset(path: String) -> Response {
    let relative = if path.is_empty() {
        "index.html"
    } else {
        path.as_str()
    };
    if safe_relative(relative).is_none() {
        return text(StatusCode::NOT_FOUND, "Nicht gefunden");
    }
    if let Some(response) = lookup(relative) {
        return response;
    }
    if Path::new(relative).extension().is_none() {
        return lookup("index.html").unwrap_or_else(unavailable);
    }
    text(StatusCode::NOT_FOUND, "Nicht gefunden")
}

fn lookup(relative: &str) -> Option<Response> {
    if let Some(root) = dist_dir() {
        let file = safe_join(&root, relative)?;
        if !file.is_file() {
            return None;
        }
        return Some(match std::fs::read(&file) {
            Ok(bytes) => bytes_response(relative, bytes),
            Err(error) => {
                tracing::error!(%error, "admin asset could not be read");
                text(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Datei konnte nicht gelesen werden",
                )
            }
        });
    }
    let file = EMBEDDED.get_file(relative)?;
    Some(bytes_response(relative, file.contents().to_vec()))
}

fn unavailable() -> Response {
    text(
        StatusCode::SERVICE_UNAVAILABLE,
        "Admin-Oberfläche ist nicht eingebettet. Im Verzeichnis apps/admin-ui npm run build ausführen und danach cargo build.",
    )
}

fn serve(relative: &str) -> Response {
    lookup(relative).unwrap_or_else(unavailable)
}

fn bytes_response(relative: &str, bytes: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(bytes));
    if let Ok(kind) = HeaderValue::from_str(content_type(Path::new(relative))) {
        response.headers_mut().insert(header::CONTENT_TYPE, kind);
    }
    response
}

fn safe_relative(relative: &str) -> Option<&str> {
    let path = Path::new(relative);
    if path.is_absolute() {
        return None;
    }
    path.components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
        .then_some(relative)
}

fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    let mut file = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => file.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    file.starts_with(root).then_some(file)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
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
