//! Eigene Admin-Seiten unter `/admin/pages/{key}`.

use crate::html::{self, error_page, locale_of, parse_form, require_get, require_post};
use crate::registry::AdminRegistry;
use crate::views::ViewRequest;
use axum::body::Bytes;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use ferrioxid_auth::AppState;

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
    let form = body.as_ref().map(parse_form).unwrap_or_default();
    let request = ViewRequest {
        user: &user,
        db: &state.db,
        locale: &locale,
        form: &form,
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

async fn holds(state: &AppState, user: &ferrioxid_auth::AuthUser, code: &str) -> Result<bool, ()> {
    let Some(auth) = state.auth.as_ref() else {
        return Err(());
    };
    auth.holds(user, code).await.map_err(|_| ())
}
