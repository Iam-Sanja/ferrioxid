//! Upload-Routen sind nicht öffentlich, solange sie nicht ausdrücklich eingehängt werden.

use crate::{LocalStorage, Storage, StorageError};
use axum::body::to_bytes;
use axum::extract::{Extension, Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use ferrioxid_auth::{AppState, AuthError};
use ferrioxid_core::{problem_json, FerrioxidError};
use std::sync::Arc;
use uuid::Uuid;

pub fn routes(storage: Arc<LocalStorage>) -> Router<AppState> {
    Router::new()
        .route("/uploads", post(create))
        .route("/uploads/{id}", get(download).delete(remove))
        .layer(Extension(storage))
}

async fn create(
    State(state): State<AppState>,
    Extension(storage): Extension<Arc<LocalStorage>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let limit = storage.max_bytes() as usize;
    let bytes = match to_bytes(request.into_body(), limit.saturating_add(1)).await {
        Ok(bytes) if bytes.len() <= limit => bytes,
        _ => return storage_error(StorageError::TooLarge),
    };
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let (csrf, filename, payload) = if let Some(boundary) = boundary(content_type) {
        match multipart(&bytes, boundary) {
            Ok(parts) => {
                let csrf = field_text(&parts, "csrf_token").unwrap_or_default();
                let Some(file) = parts.into_iter().find(|part| part.name == "file") else {
                    return storage_error(StorageError::TypeRejected);
                };
                (
                    csrf,
                    file.filename.unwrap_or_else(|| "file".to_owned()),
                    file.body,
                )
            }
            Err(error) => return storage_error(error),
        }
    } else {
        let name = headers
            .get("x-original-name")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("file")
            .to_owned();
        let csrf = headers
            .get(ferrioxid_auth::csrf_header_name())
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned();
        (csrf, name, bytes.to_vec())
    };
    let Some(auth) = state.auth.as_ref() else {
        return auth_response(AuthError::Forbidden);
    };
    let user = match auth.authenticate_csrf(&headers, &Method::POST, &csrf).await {
        Ok(user) => user,
        Err(error) => return auth_response(error),
    };
    match storage.save(user.id, &filename, &payload).await {
        Ok(stored) => Json(serde_json::json!({
            "id": stored.id,
            "original_name": stored.original_name,
            "content_type": stored.content_type,
            "size": stored.size
        }))
        .into_response(),
        Err(error) => storage_error(error),
    }
}

async fn download(
    State(state): State<AppState>,
    Extension(storage): Extension<Arc<LocalStorage>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    let Some(auth) = state.auth.as_ref() else {
        return auth_response(AuthError::Forbidden);
    };
    let user = match auth.authenticate(&headers, &Method::GET).await {
        Ok(user) => user,
        Err(error) => return auth_response(error),
    };
    match storage.read(user.id, id).await {
        Ok((meta, bytes)) => {
            let mut response = Response::new(axum::body::Body::from(bytes));
            let disposition = format!(
                "attachment; filename=\"{}\"",
                meta.original_name.replace('"', "")
            );
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            headers.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            if let Ok(value) = HeaderValue::from_str(&disposition) {
                headers.insert(header::CONTENT_DISPOSITION, value);
            }
            response
        }
        Err(error) => storage_error(error),
    }
}

async fn remove(
    State(state): State<AppState>,
    Extension(storage): Extension<Arc<LocalStorage>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    let Some(auth) = state.auth.as_ref() else {
        return auth_response(AuthError::Forbidden);
    };
    let user = match auth.authenticate(&headers, &Method::DELETE).await {
        Ok(user) => user,
        Err(error) => return auth_response(error),
    };
    match storage.delete(user.id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => storage_error(error),
    }
}

fn storage_error(error: StorageError) -> Response {
    let (status, code, detail) = match error {
        StorageError::NotFound => (404, "resource.not_found", "file was not found"),
        StorageError::TooLarge => (413, "upload.too_large", "upload is too large"),
        StorageError::TypeRejected => (415, "upload.type", "file type is not allowed"),
        StorageError::Failed => (500, "request.rejected", "Request rejected"),
        StorageError::InvalidImage => (415, "upload.image", "image was rejected"),
    };
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_REQUEST),
        Json(serde_json::json!({
            "type": "about:blank",
            "code": code,
            "title": "Upload error",
            "status": status,
            "detail": detail,
            "params": {}
        })),
    )
        .into_response()
}

fn auth_response(error: AuthError) -> Response {
    let mapped = FerrioxidError::from(error);
    let (status, title, detail) = match &mapped {
        FerrioxidError::Unauthorized(message) => (401, "Unauthorized", message.as_str()),
        FerrioxidError::Denied(message) => (403, "Forbidden", message.as_str()),
        FerrioxidError::Forbidden => (403, "Forbidden", "Access denied"),
        FerrioxidError::Validation(_) => (422, "Validation failed", "Request rejected"),
        _ => (500, "Internal server error", "Request rejected"),
    };
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_REQUEST),
        Json(problem_json(status, title, detail)),
    )
        .into_response()
}

struct Part {
    name: String,
    filename: Option<String>,
    body: Vec<u8>,
}

fn boundary(content_type: &str) -> Option<String> {
    let rest = content_type
        .split(';')
        .find(|part| part.trim().starts_with("boundary="))?;
    let value = rest
        .trim()
        .trim_start_matches("boundary=")
        .trim_matches('"');
    if value.is_empty() || value.len() > 70 || value.contains('\n') {
        None
    } else {
        Some(value.to_owned())
    }
}

fn multipart(body: &[u8], boundary: String) -> Result<Vec<Part>, StorageError> {
    let marker = format!("--{boundary}");
    let text = String::from_utf8_lossy(body);
    if text.matches(&marker).count() > 10 {
        return Err(StorageError::Failed);
    }
    let mut parts = Vec::new();
    let delimiter = format!("--{boundary}").into_bytes();
    let mut rest = body;
    while let Some(position) = find_subslice(rest, &delimiter) {
        let after = &rest[position + delimiter.len()..];
        if after.starts_with(b"--") {
            break;
        }
        let after = after.strip_prefix(b"\r\n").unwrap_or(after);
        let Some(header_end) = find_subslice(after, b"\r\n\r\n") else {
            return Err(StorageError::Failed);
        };
        let header = &after[..header_end];
        let content = &after[header_end + 4..];
        let next = find_subslice(content, &delimiter).unwrap_or(content.len());
        let mut part_body = &content[..next];
        if part_body.ends_with(b"\r\n") {
            part_body = &part_body[..part_body.len() - 2];
        }
        let header = String::from_utf8_lossy(header);
        let name = header_value(&header, "name").unwrap_or_default();
        if name.is_empty() {
            return Err(StorageError::Failed);
        }
        parts.push(Part {
            filename: header_value(&header, "filename"),
            name,
            body: part_body.to_vec(),
        });
        if parts.len() > 8 {
            return Err(StorageError::Failed);
        }
        rest = &content[next..];
    }
    if parts.is_empty() {
        Err(StorageError::Failed)
    } else {
        Ok(parts)
    }
}

fn field_text(parts: &[Part], name: &str) -> Option<String> {
    parts
        .iter()
        .find(|part| part.name == name)
        .map(|part| String::from_utf8_lossy(&part.body).trim().to_owned())
}

fn header_value(header: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = header.find(&needle)? + needle.len();
    let end = header[start..].find('"')? + start;
    Some(header[start..end].to_owned())
}

/// Maximum body accepted by [`read_multipart`]. Callers must enforce it before buffering more.
pub const MULTIPART_BODY_LIMIT: usize = 1_048_576;

const MULTIPART_PART_LIMIT: usize = 16;

/// One text or file field from a `multipart/form-data` body.
///
/// `declared_type` is the part's Content-Type header. It is not proof of the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormPart {
    pub name: String,
    pub filename: Option<String>,
    pub declared_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultipartError {
    TooLarge,
    Malformed,
    DuplicateField(String),
    InvalidFilename(String),
}

impl std::fmt::Display for MultipartError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(formatter, "multipart body exceeds the limit"),
            Self::Malformed => write!(formatter, "multipart body is malformed"),
            Self::DuplicateField(name) => write!(formatter, "field `{name}` is repeated"),
            Self::InvalidFilename(name) => write!(formatter, "filename `{name}` is not accepted"),
        }
    }
}

impl std::error::Error for MultipartError {}

/// Parses one multipart body. Duplicate field names and path-like filenames are errors.
///
/// The body is not written anywhere. A value larger than [`MULTIPART_BODY_LIMIT`] is rejected
/// before the parts are copied.
/// Reads one text field without applying filename rules to the other parts.
///
/// Authentication can read `csrf_token` before a later filename is rejected.
pub fn multipart_text_field(content_type: &str, body: &[u8], field: &str) -> Option<String> {
    parse_parts(content_type, body, false)
        .ok()
        .and_then(|parts| {
            parts.into_iter().find_map(|part| {
                if part.name == field && part.filename.is_none() {
                    String::from_utf8(part.bytes).ok()
                } else {
                    None
                }
            })
        })
}

pub fn read_multipart(content_type: &str, body: &[u8]) -> Result<Vec<FormPart>, MultipartError> {
    parse_parts(content_type, body, true)
}

fn parse_parts(
    content_type: &str,
    body: &[u8],
    check_filenames: bool,
) -> Result<Vec<FormPart>, MultipartError> {
    if body.len() > MULTIPART_BODY_LIMIT {
        return Err(MultipartError::TooLarge);
    }
    let boundary = boundary(content_type).ok_or(MultipartError::Malformed)?;
    let delimiter = format!("--{boundary}").into_bytes();
    if delimiter.len() < 3 {
        return Err(MultipartError::Malformed);
    }
    let mut rest = body;
    let mut parts = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    while let Some(position) = find_subslice(rest, &delimiter) {
        let after = &rest[position + delimiter.len()..];
        if after.starts_with(b"--") {
            break;
        }
        let after = after.strip_prefix(b"\r\n").unwrap_or(after);
        let Some(header_end) = find_subslice(after, b"\r\n\r\n") else {
            return Err(MultipartError::Malformed);
        };
        let header = String::from_utf8_lossy(&after[..header_end]);
        let content = &after[header_end + 4..];
        let next = find_subslice(content, &delimiter).unwrap_or(content.len());
        let mut part_body = &content[..next];
        if part_body.ends_with(b"\r\n") {
            part_body = &part_body[..part_body.len() - 2];
        }
        let name = header_value(&header, "name").ok_or(MultipartError::Malformed)?;
        if name.is_empty() || name.len() > 80 || name.chars().any(|char| char.is_control()) {
            return Err(MultipartError::Malformed);
        }
        if !names.insert(name.clone()) {
            return Err(MultipartError::DuplicateField(name));
        }
        let filename = header_value(&header, "filename");
        if check_filenames {
            if let Some(filename) = &filename {
                if invalid_filename(filename) {
                    return Err(MultipartError::InvalidFilename(filename.clone()));
                }
            }
        }
        parts.push(FormPart {
            name,
            filename,
            declared_type: part_content_type(&header),
            bytes: part_body.to_vec(),
        });
        if parts.len() > MULTIPART_PART_LIMIT {
            return Err(MultipartError::Malformed);
        }
        rest = &content[next..];
    }
    if parts.is_empty() {
        Err(MultipartError::Malformed)
    } else {
        Ok(parts)
    }
}

fn invalid_filename(name: &str) -> bool {
    name.is_empty()
        || name.len() > 120
        || name
            .chars()
            .any(|char| char.is_control() || matches!(char, '/' | '\\' | ':'))
        || name == "."
        || name == ".."
}

fn part_content_type(header: &str) -> Option<String> {
    header.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if !name.trim().eq_ignore_ascii_case("content-type") {
            return None;
        }
        let value = value.trim();
        if value.is_empty() || value.len() > 120 || value.chars().any(|char| char.is_control()) {
            None
        } else {
            Some(value.to_owned())
        }
    })
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::routes;
    use crate::LocalStorage;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use ferrioxid_auth::{AppState, AuthConfig, AuthService};
    use std::sync::Arc;
    use tower::ServiceExt;
    use uuid::Uuid;

    async fn call(app: axum::Router, request: Request<Body>) -> (StatusCode, String) {
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    #[tokio::test]
    async fn private_downloads_are_isolated_and_anonymous_access_is_denied() {
        let db = ferrioxid_db::connect("sqlite::memory:").await.unwrap();
        ferrioxid_auth::apply_migrations(&db).await.unwrap();
        let auth = AuthService::new(db.clone(), AuthConfig::development());
        let password = "correct-horse-battery";
        auth.create_user("ada@example.com", password, false, false)
            .await
            .unwrap();
        auth.create_user("grace@example.com", password, false, false)
            .await
            .unwrap();
        let ada = auth
            .login("ada@example.com", password, "peer")
            .await
            .unwrap();
        let grace = auth
            .login("grace@example.com", password, "peer")
            .await
            .unwrap();
        let root = std::env::temp_dir().join(format!("ferrioxid-upload-{}", Uuid::new_v4()));
        let storage = Arc::new(LocalStorage::open(&root, 1024, &["text/plain"]).unwrap());
        let app = routes(storage).with_state(AppState {
            db,
            auth: Some(auth),
            mail: None,
        });
        let missing = Uuid::nil();
        let (status, _) = call(
            app.clone(),
            Request::get(format!("/uploads/{missing}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let request = Request::post("/uploads")
            .header("cookie", ada.cookie.split(';').next().unwrap())
            .header("x-csrf-token", &ada.csrf_token)
            .header("x-original-name", "../../notes.txt")
            .body(Body::from("hello ferrioxid"))
            .unwrap();
        let (status, body) = call(app.clone(), request).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(!body.contains(&root.display().to_string()));
        let id = body
            .split("\"id\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_owned();
        assert!(body.contains("notes.txt"));

        let request = Request::get(format!("/uploads/{id}"))
            .header("cookie", grace.cookie.split(';').next().unwrap())
            .body(Body::empty())
            .unwrap();
        let (status, _) = call(app.clone(), request).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let request = Request::delete(format!("/uploads/{id}"))
            .header("cookie", grace.cookie.split(';').next().unwrap())
            .header("x-csrf-token", &grace.csrf_token)
            .body(Body::empty())
            .unwrap();
        let (status, _) = call(app.clone(), request).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let request = Request::delete(format!("/uploads/{id}"))
            .header("cookie", ada.cookie.split(';').next().unwrap())
            .body(Body::empty())
            .unwrap();
        let (status, _) = call(app.clone(), request).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let request = Request::get(format!("/uploads/{id}"))
            .header("cookie", ada.cookie.split(';').next().unwrap())
            .body(Body::empty())
            .unwrap();
        let (status, body) = call(app.clone(), request).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("hello ferrioxid"));

        let request = Request::delete(format!("/uploads/{id}"))
            .header("cookie", ada.cookie.split(';').next().unwrap())
            .header("x-csrf-token", &ada.csrf_token)
            .body(Body::empty())
            .unwrap();
        let (status, _) = call(app.clone(), request).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn multipart_keeps_files_and_rejects_duplicate_or_unsafe_names() {
        let body = b"\
--bound\r\n\
Content-Disposition: form-data; name=\"title\"\r\n\
\r\n\
Hammer\r\n\
--bound\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"photo.png\"\r\n\
Content-Type: image/png\r\n\
\r\n\
PNGDATA\r\n\
--bound\r\n\
Content-Disposition: form-data; name=\"alt\"; filename=\"second.jpg\"\r\n\
Content-Type: image/jpeg\r\n\
\r\n\
JPEG\r\n\
--bound--\r\n";
        let parts = super::read_multipart("multipart/form-data; boundary=bound", body).unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].bytes, b"Hammer");
        assert!(parts[0].filename.is_none());
        assert_eq!(parts[1].filename.as_deref(), Some("photo.png"));
        assert_eq!(parts[1].declared_type.as_deref(), Some("image/png"));
        assert_eq!(parts[1].bytes, b"PNGDATA");
        assert_eq!(parts[2].filename.as_deref(), Some("second.jpg"));

        let duplicate = b"\
--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\n\r\nA\r\n\
--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"b.png\"\r\n\r\nB\r\n\
--bound--\r\n";
        assert!(matches!(
            super::read_multipart("multipart/form-data; boundary=bound", duplicate),
            Err(super::MultipartError::DuplicateField(_))
        ));
        let escaped = b"\
--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../a.png\"\r\n\r\nA\r\n\
--bound--\r\n";
        assert!(matches!(
            super::read_multipart("multipart/form-data; boundary=bound", escaped),
            Err(super::MultipartError::InvalidFilename(_))
        ));
        assert!(matches!(
            super::read_multipart(
                "multipart/form-data; boundary=bound",
                &vec![b'x'; 1_048_577]
            ),
            Err(super::MultipartError::TooLarge)
        ));
    }
}
