//! Statische Projektdateien. Getrennt von den eingebetteten Admin-Assets.
//!
//! Ausgeliefert wird nur, was nach der Kanonisierung im konfigurierten Wurzelverzeichnis liegt.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct StaticOptions {
    pub url_prefix: String,
    pub directory: PathBuf,
    pub cache_seconds: u64,
    pub max_bytes: u64,
}

impl StaticOptions {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            url_prefix: "/static".to_owned(),
            directory: directory.into(),
            cache_seconds: 3600,
            max_bytes: 5 * 1024 * 1024,
        }
    }
}

pub fn mount<S>(router: Router<S>, options: StaticOptions) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let prefix = options.url_prefix.trim_end_matches('/').to_owned();
    let route = format!("{prefix}/{{*path}}");
    let options = Arc::new(options);
    let head_options = options.clone();
    router.route(
        &route,
        get(move |request: Request| {
            let options = options.clone();
            async move { serve(options, request).await }
        })
        .head(move |request: Request| {
            let options = head_options.clone();
            async move { serve(options, request).await }
        }),
    )
}

async fn serve(options: Arc<StaticOptions>, request: Request) -> Response {
    let prefix = options.url_prefix.trim_end_matches('/');
    let path = request.uri().path();
    let Some(relative) = path.strip_prefix(prefix) else {
        return missing();
    };
    let relative = relative.trim_start_matches('/');
    match resolve(&options.directory, relative) {
        Ok(file) => {
            file_response(
                &options,
                &file,
                request.headers().get(header::IF_NONE_MATCH),
            )
            .await
        }
        Err(StaticError::Hidden) | Err(StaticError::Outside) | Err(StaticError::Directory) => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(StaticError::TooLarge) => StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(StaticError::Type) => StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum StaticError {
    Outside,
    Hidden,
    Directory,
    TooLarge,
    Type,
}

fn resolve(root: &Path, relative: &str) -> Result<PathBuf, StaticError> {
    if relative.is_empty() || relative.contains('\\') || relative.contains('\0') {
        return Err(StaticError::Outside);
    }
    let mut parts = Vec::new();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.starts_with('.') {
            return Err(if part.starts_with('.') && part != "." && part != ".." {
                StaticError::Hidden
            } else {
                StaticError::Outside
            });
        }
        let lower = part.to_ascii_lowercase();
        if lower == ".env"
            || lower.ends_with(".sqlite")
            || lower.ends_with(".sqlite3")
            || lower.ends_with(".pem")
        {
            return Err(StaticError::Hidden);
        }
        parts.push(part);
    }
    let candidate = parts
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part));
    if !candidate.is_file() {
        return Err(if candidate.is_dir() {
            StaticError::Directory
        } else {
            StaticError::Outside
        });
    }
    let root = std::fs::canonicalize(root).map_err(|_| StaticError::Outside)?;
    let file = std::fs::canonicalize(&candidate).map_err(|_| StaticError::Outside)?;
    if !file.starts_with(&root) {
        return Err(StaticError::Outside);
    }
    let extension = file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if mime(extension).is_none() {
        return Err(StaticError::Type);
    }
    let length = std::fs::metadata(&file)
        .map(|meta| meta.len())
        .unwrap_or(u64::MAX);
    if length > 5 * 1024 * 1024 {
        return Err(StaticError::TooLarge);
    }
    Ok(file)
}

fn mime(extension: &str) -> Option<&'static str> {
    Some(match extension.to_ascii_lowercase().as_str() {
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "map" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => return None,
    })
}

async fn file_response(
    options: &StaticOptions,
    file: &Path,
    if_none_match: Option<&HeaderValue>,
) -> Response {
    let bytes = match tokio::fs::read(file).await {
        Ok(bytes) => bytes,
        Err(_) => return missing(),
    };
    if bytes.len() as u64 > options.max_bytes {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let etag = format!("\"{:x}\"", hasher.finalize());
    if if_none_match.and_then(|value| value.to_str().ok()) == Some(etag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    let extension = file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let mut response = Response::new(Body::from(bytes.clone()));
    let headers = response.headers_mut();
    let content_type = mime(extension).unwrap_or("application/octet-stream");
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).unwrap_or(HeaderValue::from_static("0")),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag).unwrap_or(HeaderValue::from_static("\"0\"")),
    );
    let cache = format!("public, max-age={}", options.cache_seconds);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_str(&cache).unwrap_or(HeaderValue::from_static("no-store")),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn missing() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::{mount, resolve, StaticError, StaticOptions};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    #[test]
    fn traversal_hidden_files_and_types_are_rejected() {
        let root = std::env::temp_dir().join(format!("ferrum-static-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("css")).unwrap();
        fs::write(root.join("css/app.css"), b"body{}").unwrap();
        fs::write(root.join(".env"), b"SECRET=1").unwrap();
        assert!(resolve(&root, "css/app.css").is_ok());
        assert_eq!(resolve(&root, "../.env"), Err(StaticError::Outside));
        assert_eq!(resolve(&root, ".env"), Err(StaticError::Hidden));
        fs::write(root.join("note.html"), b"<html>").unwrap();
        assert_eq!(resolve(&root, "note.html"), Err(StaticError::Type));
        let outside =
            std::env::temp_dir().join(format!("ferrum-static-secret-{}", std::process::id()));
        fs::write(&outside, b"secret").unwrap();
        let link = root.join("css/escape.css");
        #[cfg(windows)]
        let created = std::os::windows::fs::symlink_file(&outside, &link);
        #[cfg(unix)]
        let created = std::os::unix::fs::symlink(&outside, &link);
        #[cfg(not(any(unix, windows)))]
        let created: std::io::Result<()> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlinks are not supported on this platform",
        ));
        if created.is_ok() {
            assert!(resolve(&root, "css/escape.css").is_err());
        }
        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn http_serves_css_and_sets_cache_headers() {
        let root = std::env::temp_dir().join(format!("ferrum-static-http-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("css")).unwrap();
        fs::create_dir_all(root.join("js")).unwrap();
        fs::create_dir_all(root.join("images")).unwrap();
        fs::write(root.join("css/app.css"), b"body{color:#111}").unwrap();
        fs::write(root.join("js/app.js"), b"var ready=true;").unwrap();
        fs::write(
            root.join("images/logo.png"),
            [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        )
        .unwrap();
        let app = mount(axum::Router::new(), StaticOptions::new(&root));
        let css = exchange(
            &app,
            "GET /static/css/app.css HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(css.contains("200"), "{css}");
        assert!(css.contains("text/css"));
        assert!(css.contains("max-age=3600"));
        assert!(css.contains("etag"));
        assert!(css.contains("body{color:#111}"));
        let script = exchange(
            &app,
            "GET /static/js/app.js HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(
            script.contains("application/javascript") || script.contains("text/javascript"),
            "{script}"
        );
        let image = exchange(
            &app,
            "GET /static/images/logo.png HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(image.contains("image/png"), "{image}");
        let missing = exchange(
            &app,
            "GET /static/missing.css HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(missing.contains("404"), "{missing}");
        let _ = fs::remove_dir_all(&root);
    }

    fn exchange(app: &axum::Router, request: &str) -> String {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let address = listener.local_addr().unwrap();
        let app = app.clone();
        runtime.spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}
