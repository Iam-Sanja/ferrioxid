//! Öffentliches Kontaktformular. Eine vorhandene Sitzung muss das bestehende CSRF-Token senden.

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{Html, IntoResponse};
use ferrioxid::auth::AppState;
use ferrioxid::forms::{
    contact_widgets, message, render_fields, render_page, ContactForm, FerrioxidForm, FormErrors,
    FormValues,
};
use ferrioxid::views::{fail, redirect_to};
use std::sync::Arc;

pub async fn show(State(state): State<AppState>, headers: HeaderMap) -> Html<String> {
    Html(
        page(
            &state,
            &headers,
            &FormValues::default(),
            &FormErrors::default(),
        )
        .await,
    )
}

pub async fn thanks() -> Html<String> {
    Html(render_page(
        "Kontakt",
        &format!("<p role=\"status\">{}</p>", message("de", "form.thanks")),
    ))
}

pub async fn submit(
    State(state): State<AppState>,
    Extension(routes): Extension<Arc<ferrioxid::urls::RouteMap>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let values = match FormValues::parse_urlencoded(&String::from_utf8_lossy(&body)) {
        Ok(values) => values,
        Err(errors) => {
            return Html(page(&state, &headers, &FormValues::default(), &errors).await)
                .into_response()
        }
    };
    if let Some(auth) = state.auth.as_ref() {
        if auth.csrf_from_request(&headers).await.is_ok() {
            let supplied = values.get("csrf_token").unwrap_or("");
            if auth
                .authenticate_csrf(&headers, &Method::POST, supplied)
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
        }
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
        Err(errors) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Html(page(&state, &headers, &values, &errors).await),
        )
            .into_response(),
    }
}

async fn page(
    state: &AppState,
    headers: &HeaderMap,
    values: &FormValues,
    errors: &FormErrors,
) -> String {
    let csrf = match state.auth.as_ref() {
        Some(auth) => auth.csrf_from_request(headers).await.ok(),
        None => None,
    };
    let form = render_fields(
        "/contact",
        csrf.as_deref(),
        values,
        errors,
        &contact_widgets("de"),
        "de",
    );
    render_page("Kontakt", &form)
}
