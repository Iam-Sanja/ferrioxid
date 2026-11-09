//! Öffentliches Kontaktformular. Eine vorhandene Sitzung muss das bestehende CSRF-Token senden.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{Html, IntoResponse};
use ferrioxid::auth::AppState;
use ferrioxid::forms::{
    contact_widgets, message, render_fields, render_page, ContactForm, FerrioxidForm, FormErrors,
    FormValues,
};

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

pub async fn submit(
    State(state): State<AppState>,
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
                let mut errors = FormErrors::default();
                errors.push("", "form.csrf");
                return (
                    StatusCode::FORBIDDEN,
                    Html(page(&state, &headers, &values, &errors).await),
                )
                    .into_response();
            }
        }
    }
    match ContactForm::from_form(&values) {
        Ok(_form) => Html(render_page(
            "Kontakt",
            &format!("<p role=\"status\">{}</p>", message("de", "form.thanks")),
        ))
        .into_response(),
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
