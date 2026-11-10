//! Eigene Admin-Seiten unter `/admin/pages/{key}`.

use crate::html::{self, error_page, locale_of, parse_form, require_get, require_post};
use crate::registry::AdminRegistry;
use crate::views::{UploadedFile, ViewRequest};
use axum::body::Bytes;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use ferrioxid_auth::AppState;
use std::collections::HashMap;

pub async fn show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(key): Path<String>,
) -> Response {
    render(&state, &headers, &registry, &key, None).await
}

pub async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(key): Path<String>,
    body: Bytes,
) -> Response {
    render(&state, &headers, &registry, &key, Some(body)).await
}

async fn render(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    key: &str,
    body: Option<Bytes>,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    if !registry.sealed() {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::INTERNAL_SERVER_ERROR,
            "forbidden",
            false,
            "",
        )
        .await;
    }
    let Some(view) = registry.view_by_key(key) else {
        let signed = require_get(state, headers, registry, "/admin").await.ok();
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            signed.is_some(),
            signed
                .as_ref()
                .map(|user| user.email.as_str())
                .unwrap_or(""),
        )
        .await;
    };
    if let Some(bytes) = body.as_ref() {
        if multipart_type(headers).is_some()
            && bytes.len() > ferrioxid_storage::MULTIPART_BODY_LIMIT
        {
            return error_page(
                state,
                headers,
                registry,
                &locale,
                StatusCode::PAYLOAD_TOO_LARGE,
                "validation",
                false,
                "",
            )
            .await;
        }
    }
    let user = if let Some(bytes) = body.as_ref() {
        match require_post(state, headers, bytes).await {
            Ok(user) => user,
            Err(()) => {
                return error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::FORBIDDEN,
                    "csrf",
                    true,
                    "",
                )
                .await;
            }
        }
    } else {
        match require_get(state, headers, registry, &format!("/admin/pages/{key}")).await {
            Ok(user) => user,
            Err(response) => return response,
        }
    };
    let allowed = match holds(state, &user, view.permission_code()).await {
        Ok(true) => true,
        Ok(false) => false,
        Err(()) => {
            return error_page(
                state,
                headers,
                registry,
                &locale,
                StatusCode::INTERNAL_SERVER_ERROR,
                "forbidden",
                true,
                &user.email,
            )
            .await;
        }
    };
    if !allowed {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    let handler = if body.is_some() {
        view.post_handler()
    } else {
        view.get_handler()
    };
    let Some(handler) = handler else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::METHOD_NOT_ALLOWED,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    };
    let parsed = match body.as_ref() {
        Some(bytes) => match multipart_type(headers) {
            Some(content_type) => match ferrioxid_storage::read_multipart(content_type, bytes) {
                Ok(parts) => match split_upload(parts) {
                    Ok(parsed) => parsed,
                    Err(()) => {
                        return error_page(
                            state,
                            headers,
                            registry,
                            &locale,
                            StatusCode::BAD_REQUEST,
                            "validation",
                            true,
                            &user.email,
                        )
                        .await;
                    }
                },
                Err(_) => {
                    return error_page(
                        state,
                        headers,
                        registry,
                        &locale,
                        StatusCode::BAD_REQUEST,
                        "validation",
                        true,
                        &user.email,
                    )
                    .await;
                }
            },
            None => (parse_form(bytes), Vec::new()),
        },
        None => (HashMap::new(), Vec::new()),
    };
    let (form, uploads) = parsed;
    let request = ViewRequest {
        user: &user,
        db: &state.db,
        locale: &locale,
        form: &form,
        uploads: &uploads,
    };
    let rendered = match handler.handle(&request).await {
        Ok(page) => page,
        Err(error) => {
            tracing::error!(
                code = error.code,
                detail = error.detail,
                "custom admin view failed"
            );
            return error_page(
                state,
                headers,
                registry,
                &locale,
                StatusCode::INTERNAL_SERVER_ERROR,
                "forbidden",
                true,
                &user.email,
            )
            .await;
        }
    };
    let csrf = html::session_csrf(state.auth.as_ref().unwrap(), headers).await;
    let data = serde_json::Value::Object(rendered.data);
    html::page_as(
        state,
        &user,
        registry,
        view.template_name(),
        &locale,
        html::theme_of(headers),
        view.title(&locale),
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        &format!("/admin/pages/{key}"),
        &clip(&rendered.notice),
        &clip(&rendered.alert),
        data,
    )
    .await
}

fn clip(value: &str) -> String {
    value.chars().take(200).collect()
}

fn multipart_type(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())?;
    value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("multipart/form-data")
        .then_some(value)
}

fn split_upload(
    parts: Vec<ferrioxid_storage::FormPart>,
) -> Result<(HashMap<String, String>, Vec<UploadedFile>), ()> {
    let mut form = HashMap::new();
    let mut uploads = Vec::new();
    for part in parts {
        if let Some(filename) = part.filename {
            uploads.push(UploadedFile {
                field: part.name,
                filename,
                declared_type: part.declared_type,
                bytes: part.bytes,
            });
        } else {
            let text = String::from_utf8(part.bytes).map_err(|_| ())?;
            form.insert(part.name, text);
        }
    }
    Ok((form, uploads))
}

async fn holds(state: &AppState, user: &ferrioxid_auth::AuthUser, code: &str) -> Result<bool, ()> {
    let Some(auth) = state.auth.as_ref() else {
        return Err(());
    };
    auth.holds(user, code).await.map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::submit;
    use crate::registry::AdminRegistry;
    use crate::views::{AdminView, ExtensionError, ViewHandler, ViewPage, ViewRequest};
    use axum::body::Bytes;
    use axum::extract::{Extension, Path, State};
    use axum::http::{HeaderMap, StatusCode};
    use ferrioxid_auth::{AppState, AuthConfig, AuthService};
    use std::sync::{Arc, Mutex};

    fn memory_fixture_password() -> String {
        String::from_utf8(vec![
            99, 111, 114, 114, 101, 99, 116, 45, 104, 111, 114, 115, 101, 45, 98, 97, 116, 116,
            101, 114, 121,
        ])
        .expect("fixture password")
    }

    struct Capture(Arc<Mutex<Option<(String, String, usize)>>>);

    #[async_trait::async_trait]
    impl ViewHandler for Capture {
        async fn handle(&self, request: &ViewRequest<'_>) -> Result<ViewPage, ExtensionError> {
            let file = request.uploads.first();
            *self.0.lock().expect("capture") = Some((
                request.form.get("title").cloned().unwrap_or_default(),
                file.map(|file| file.filename.clone()).unwrap_or_default(),
                file.map(|file| file.size()).unwrap_or(0),
            ));
            Ok(ViewPage::new(Default::default()))
        }
    }

    async fn service() -> AuthService {
        let db = ferrioxid_db::connect("sqlite::memory:").await.unwrap();
        ferrioxid_auth::apply_migrations(&db).await.unwrap();
        AuthService::new(db, AuthConfig::development())
    }

    fn registry(capture: Arc<Mutex<Option<(String, String, usize)>>>) -> AdminRegistry {
        let handler = Capture(capture);
        AdminRegistry::new()
            .view(
                AdminView::new("uploads")
                    .titles("Uploads", "Uploads")
                    .permission("catalog.change")
                    .on_get(Capture(handler.0.clone()))
                    .on_post(handler),
            )
            .unwrap()
            .seal()
            .unwrap()
    }

    fn cookie_pair(set_cookie: &str) -> String {
        set_cookie.split(';').next().unwrap().to_owned()
    }

    fn multipart(parts: &str) -> Bytes {
        Bytes::from(format!("{parts}--bound--\r\n"))
    }

    async fn post(
        auth: &AuthService,
        registry: AdminRegistry,
        headers: HeaderMap,
        body: Bytes,
    ) -> StatusCode {
        let state = AppState {
            db: auth.database().clone(),
            auth: Some(auth.clone()),
            mail: None,
        };
        let response = submit(
            State(state),
            headers,
            Extension(registry),
            Path("uploads".to_owned()),
            body,
        )
        .await;
        response.status()
    }

    #[tokio::test]
    async fn custom_page_accepts_multipart_and_keeps_urlencoded_forms() {
        let auth = service().await;
        let password = memory_fixture_password();
        let root = auth
            .create_user("root@example.com", &password, true, true)
            .await
            .unwrap();
        let issued = auth
            .login("root@example.com", &password, "peer")
            .await
            .unwrap();
        let _ = root;
        let capture = Arc::new(Mutex::new(None));
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            cookie_pair(&issued.cookie).parse().unwrap(),
        );
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            "multipart/form-data; boundary=bound".parse().unwrap(),
        );
        let body = multipart(&format!(
            "--bound\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\n{}\r\n\
--bound\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHammer\r\n\
--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"photo.png\"\r\nContent-Type: image/png\r\n\r\nPNG\r\n\
--bound\r\nContent-Disposition: form-data; name=\"extra\"; filename=\"photo.jpg\"\r\nContent-Type: image/jpeg\r\n\r\nJPG\r\n",
            issued.csrf_token
        ));
        let status = post(&auth, registry(capture.clone()), headers.clone(), body).await;
        assert_eq!(status, StatusCode::OK);
        let seen = capture.lock().unwrap().clone().unwrap();
        assert_eq!(seen.0, "Hammer");
        assert_eq!(seen.1, "photo.png");
        assert_eq!(seen.2, 3);

        *capture.lock().unwrap() = None;
        let mut form_headers = HeaderMap::new();
        form_headers.insert(
            axum::http::header::COOKIE,
            cookie_pair(&issued.cookie).parse().unwrap(),
        );
        form_headers.insert(
            axum::http::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        let status = post(
            &auth,
            registry(capture.clone()),
            form_headers,
            Bytes::from(format!("csrf_token={}&title=Plain", issued.csrf_token)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let seen = capture.lock().unwrap().clone().unwrap();
        assert_eq!(seen.0, "Plain");
        assert!(seen.1.is_empty());

        *capture.lock().unwrap() = None;
        let status = post(
            &auth,
            registry(capture.clone()),
            headers.clone(),
            multipart(
                "--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../x.png\"\r\n\r\nA\r\n",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(capture.lock().unwrap().is_none());

        let bad = headers.clone();
        let status = post(
            &auth,
            registry(capture.clone()),
            bad,
            multipart(&format!(
                "--bound\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\n{}\r\n\
--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../x.png\"\r\n\r\nA\r\n",
                issued.csrf_token
            )),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(capture.lock().unwrap().is_none());

        let status = post(
            &auth,
            registry(capture.clone()),
            headers,
            multipart(
                "--bound\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\nwrong\r\n\
--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"photo.png\"\r\n\r\nPNG\r\n",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(capture.lock().unwrap().is_none());

        let anonymous = post(
            &auth,
            registry(capture.clone()),
            HeaderMap::new(),
            multipart(
                "--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"photo.png\"\r\n\r\nPNG\r\n",
            ),
        )
        .await;
        assert_eq!(anonymous, StatusCode::FORBIDDEN);
        assert!(capture.lock().unwrap().is_none());

        let clerk = auth
            .create_user("clerk@example.com", &password, true, false)
            .await
            .unwrap();
        let clerk_session = auth
            .login("clerk@example.com", &password, "peer")
            .await
            .unwrap();
        let _ = clerk;
        let mut clerk_headers = HeaderMap::new();
        clerk_headers.insert(
            axum::http::header::COOKIE,
            cookie_pair(&clerk_session.cookie).parse().unwrap(),
        );
        clerk_headers.insert(
            axum::http::header::CONTENT_TYPE,
            "multipart/form-data; boundary=bound".parse().unwrap(),
        );
        let status = post(
            &auth,
            registry(capture.clone()),
            clerk_headers,
            multipart(&format!(
                "--bound\r\nContent-Disposition: form-data; name=\"csrf_token\"\r\n\r\n{}\r\n\
--bound\r\nContent-Disposition: form-data; name=\"file\"; filename=\"photo.png\"\r\n\r\nPNG\r\n",
                clerk_session.csrf_token
            )),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(capture.lock().unwrap().is_none());

        let mut huge = HeaderMap::new();
        huge.insert(
            axum::http::header::CONTENT_TYPE,
            "multipart/form-data; boundary=bound".parse().unwrap(),
        );
        let status = post(
            &auth,
            registry(capture.clone()),
            huge,
            Bytes::from(vec![b'x'; ferrioxid_storage::MULTIPART_BODY_LIMIT + 1]),
        )
        .await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(capture.lock().unwrap().is_none());
    }
}
