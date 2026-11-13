//! Öffentliches Kontaktformular. Jede Änderung verlangt einen Auth-Service und eine gültige Sitzungs-CSRF.

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use fexu::auth::AppState;
use fexu::forms::{
    contact_widgets, message, render_fields, render_page, ContactForm, FexuForm, FormErrors,
    FormValues,
};
use fexu::views::{fail, redirect_to};
use std::sync::Arc;

pub async fn show(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match page(
        &state,
        &headers,
        &FormValues::default(),
        &FormErrors::default(),
    )
    .await
    {
        Ok((html, cookie)) => html_response(html, cookie),
        Err(()) => fail(
            &headers,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            "request.rejected",
            "request rejected",
            false,
        ),
    }
}

pub async fn thanks() -> Html<String> {
    Html(render_page(
        "Kontakt",
        &format!("<p role=\"status\">{}</p>", message("de", "form.thanks")),
    ))
}

pub async fn submit(
    State(state): State<AppState>,
    Extension(routes): Extension<Arc<fexu::urls::RouteMap>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(auth) = state.auth.as_ref() else {
        return fail(
            &headers,
            None,
            StatusCode::FORBIDDEN,
            "form.csrf",
            "form.csrf",
            false,
        );
    };
    let values = match FormValues::parse_urlencoded(&String::from_utf8_lossy(&body)) {
        Ok(values) => values,
        Err(errors) => {
            return match page(&state, &headers, &FormValues::default(), &errors).await {
                Ok((html, cookie)) => html_response(html, cookie),
                Err(()) => fail(
                    &headers,
                    None,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "request.rejected",
                    "request rejected",
                    false,
                ),
            };
        }
    };
    let supplied = values.get("csrf_token").unwrap_or("");
    if auth
        .authenticate_browser_form(&headers, &Method::POST, supplied)
        .await
        .is_err()
    {
        return fail(
            &headers,
            None,
            StatusCode::FORBIDDEN,
            "form.csrf",
            "form.csrf",
            false,
        );
    }
    match ContactForm::from_form(&values) {
        Ok(_form) => match routes.path("shop.thanks", &[]) {
            Ok(target) => redirect_to(&target).unwrap_or_else(|error| error.into_response()),
            Err(_) => fail(
                &headers,
                None,
                StatusCode::INTERNAL_SERVER_ERROR,
                "url.unknown",
                "Internal server error",
                false,
            ),
        },
        Err(errors) => match page(&state, &headers, &values, &errors).await {
            Ok((html, cookie)) => {
                let mut response = (StatusCode::UNPROCESSABLE_ENTITY, Html(html)).into_response();
                if let Some(cookie) = cookie {
                    append_set_cookie(&mut response, &cookie);
                }
                response
            }
            Err(()) => fail(
                &headers,
                None,
                StatusCode::INTERNAL_SERVER_ERROR,
                "request.rejected",
                "request rejected",
                false,
            ),
        },
    }
}

async fn page(
    state: &AppState,
    headers: &HeaderMap,
    values: &FormValues,
    errors: &FormErrors,
) -> Result<(String, Option<String>), ()> {
    let (csrf, cookie) = match state.auth.as_ref() {
        Some(auth) => {
            if let Ok(token) = auth.csrf_from_request(headers).await {
                (Some(token), None)
            } else {
                let session = auth
                    .ensure_anonymous_session(headers)
                    .await
                    .map_err(|_| ())?;
                (Some(session.csrf_token), session.cookie)
            }
        }
        None => (None, None),
    };
    let form = render_fields(
        "/contact",
        csrf.as_deref(),
        values,
        errors,
        &contact_widgets("de"),
        "de",
    );
    Ok((render_page("Kontakt", &form), cookie))
}

fn html_response(html: String, cookie: Option<String>) -> Response {
    let mut response = Html(html).into_response();
    if let Some(cookie) = cookie {
        append_set_cookie(&mut response, &cookie);
    }
    response
}

fn append_set_cookie(response: &mut Response, value: &str) {
    if let Ok(header_value) = HeaderValue::from_str(value) {
        response
            .headers_mut()
            .append(header::SET_COOKIE, header_value);
    }
}
