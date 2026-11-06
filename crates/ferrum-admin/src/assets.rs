//! Liefert das gebaute Admin-Frontend aus einem Verzeichnis. Node.js läuft dabei nicht mit.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use std::path::{Component, Path, PathBuf};

pub fn dist_dir() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("FERRUM_ADMIN_DIST") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
        return None;
    }
    let compiled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/admin-ui/dist");
    compiled.is_dir().then_some(compiled)
}

pub async fn index() -> Response {
    serve_path("index.html").await
}

pub async fn asset(path: String) -> Response {
    if path.is_empty() || path == "index.html" {
        return index().await;
    }
    let response = serve_path(&path).await;
    if response.status() == StatusCode::NOT_FOUND {
        return index().await;
    }
    response
}

async fn serve_path(relative: &str) -> Response {
    let Some(root) = dist_dir() else {
        return text(
            StatusCode::SERVICE_UNAVAILABLE,
            "Admin-Oberfläche ist nicht gebaut. Im Verzeichnis apps/admin-ui npm run build ausführen.",
        );
    };
    let Some(file) = safe_join(&root, relative) else {
        return text(StatusCode::NOT_FOUND, "Nicht gefunden");
    };
    if !file.is_file() {
        return text(StatusCode::NOT_FOUND, "Nicht gefunden");
    }
    match std::fs::read(&file) {
        Ok(bytes) => {
            let mut response = Response::new(Body::from(bytes));
            if let Ok(kind) = HeaderValue::from_str(content_type(&file)) {
                response.headers_mut().insert(header::CONTENT_TYPE, kind);
            }
            response
        }
        Err(error) => {
            tracing::error!(%error, "admin asset could not be read");
            text(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Datei konnte nicht gelesen werden",
            )
        }
    }
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
    (status, message.to_owned()).into_response_text()
}

trait TextResponse {
    fn into_response_text(self) -> Response;
}

impl TextResponse for (StatusCode, String) {
    fn into_response_text(self) -> Response {
        let mut response = Response::new(Body::from(self.1));
        *response.status_mut() = self.0;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        response
    }
}
