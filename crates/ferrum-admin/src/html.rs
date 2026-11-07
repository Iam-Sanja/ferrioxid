//! Serverseitige Admin-Seiten. Schreibende Aktionen sind POST und nutzen das bestehende CSRF.

use crate::http::{
    access_scope, display_column, filters_from, gate, list_blocked, permit, reject_locked,
};
use crate::localization::{catalog, normalize, notice, validation_text};
use crate::registry::{redact, sensitive, AdminRegistry, ManagedModel};
use crate::rendering::render;
use axum::body::Bytes;
use axum::extract::{Extension, Path, Query, RawQuery, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use ferrum_auth::{AppState, AuthConfig, AuthError, AuthUser, CookieSameSite};
use ferrum_core::{ColumnKind, ColumnSpec, FerrumError};
use sea_orm::Value as DbValue;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub fn routes(router: Router<AppState>, registry: AdminRegistry) -> Router<AppState> {
    router.merge(
        Router::new()
            .route("/admin", get(dashboard))
            .route("/admin/", get(dashboard))
            .route("/admin/login", get(login_form).post(login_submit))
            .route("/admin/logout", post(logout))
            .route("/admin/preferences", post(preferences))
            .route("/admin/models", get(model_index))
            .route("/admin/models/{model}", get(model_list))
            .route(
                "/admin/models/{model}/new",
                get(model_new).post(model_create),
            )
            .route("/admin/models/{model}/{id}", get(model_detail))
            .route(
                "/admin/models/{model}/{id}/edit",
                get(model_edit).post(model_update),
            )
            .route(
                "/admin/models/{model}/{id}/delete",
                get(model_delete).post(model_remove),
            )
            .route("/admin/directory/users", get(users))
            .route("/admin/directory/users/{id}/active", post(user_active))
            .route("/admin/directory/users/{id}/staff", post(user_staff))
            .route("/admin/directory/roles", get(roles).post(role_create))
            .route("/admin/directory/permissions", get(permissions_page))
            .route(
                "/admin/directory/roles/{role}/permissions",
                post(permission_change),
            )
            .route("/admin/directory/assignments", post(assignment))
            .layer(Extension(registry)),
    )
}

async fn login_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let locale = locale_of(
        &headers,
        registry.branding().default_locale.as_str(),
        registry.branding().fallback_locale.as_str(),
    );
    let token = login_token(&headers);
    let mut response = page(
        &registry,
        "login.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)["login"].as_str().unwrap_or("Login"),
        false,
        false,
        "",
        "",
        "/admin/login",
        "",
        "",
        json!({
            "login_csrf": token,
            "next": safe_next(query.get("next").map(String::as_str).unwrap_or("/admin")).unwrap_or("/admin"),
            "email": ""
        }),
    );
    append_cookie(
        &mut response,
        &login_cookie(state.auth.as_ref().map(|auth| auth.config()), &token),
    );
    response
}

async fn login_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let form = parse_form(&body);
    let Some(auth) = state.auth.as_ref() else {
        return error_page(
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            false,
            "",
        );
    };
    let supplied = form.get("csrf_token").map(String::as_str).unwrap_or("");
    if let Err(reason) = login_gate(auth, &headers, supplied) {
        log_login_rejection(&headers, reason);
        return error_page(&registry, &locale, StatusCode::FORBIDDEN, "csrf", false, "");
    }
    let next =
        safe_next(form.get("next").map(String::as_str).unwrap_or("/admin")).unwrap_or("/admin");
    match auth
        .login(
            form.get("email").map(String::as_str).unwrap_or(""),
            form.get("password").map(String::as_str).unwrap_or(""),
            &auth.client_key(&headers, "html"),
        )
        .await
    {
        Ok(issued) => {
            let mut response = redirect(next);
            append_cookie(&mut response, &issued.cookie);
            append_cookie(
                &mut response,
                &clear_cookie("ferrum_login_csrf", auth.config()),
            );
            response
        }
        Err(AuthError::RateLimited) => error_page(
            &registry,
            &locale,
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            false,
            "",
        ),
        Err(AuthError::InvalidCredentials) => {
            let token = login_token(&headers);
            let mut response = page(
                &registry,
                "login.html",
                &locale,
                theme_of(&headers),
                catalog(&locale)["login"].as_str().unwrap_or("Login"),
                false,
                false,
                "",
                "",
                "/admin/login",
                "",
                &validation_text(&locale, "Invalid email or password"),
                json!({ "login_csrf": token, "next": next, "email": form.get("email").cloned().unwrap_or_default() }),
            );
            *response.status_mut() = StatusCode::UNAUTHORIZED;
            append_cookie(&mut response, &login_cookie(Some(auth.config()), &token));
            response
        }
        Err(_) => error_page(
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            false,
            "",
        ),
    }
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let Some(auth) = state.auth.as_ref() else {
        return error_page(
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            false,
            "",
        );
    };
    let form = parse_form(&body);
    if auth
        .authenticate_csrf(
            &headers,
            &Method::POST,
            form.get("csrf_token").map(String::as_str).unwrap_or(""),
        )
        .await
        .is_err()
    {
        return error_page(&registry, &locale, StatusCode::FORBIDDEN, "csrf", true, "");
    }
    let token = cookie(&headers, &auth.config().cookie_name);
    let cleared = auth.logout(token.as_deref()).await.unwrap_or_default();
    let mut response = redirect("/admin/login");
    if !cleared.is_empty() {
        append_cookie(&mut response, &cleared);
    }
    response
}

async fn preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    if require_post(&state, &headers, &body).await.is_err() {
        return error_page(&registry, &locale, StatusCode::FORBIDDEN, "csrf", true, "");
    }
    let form = parse_form(&body);
    let next =
        safe_next(form.get("next").map(String::as_str).unwrap_or("/admin")).unwrap_or("/admin");
    let chosen = match form.get("locale").map(String::as_str).unwrap_or("") {
        "de" => "de",
        "en" => "en",
        _ => locale.as_str(),
    };
    let theme = match form.get("theme").map(String::as_str).unwrap_or("system") {
        "light" => "light",
        "dark" => "dark",
        _ => "system",
    };
    let config = state.auth.as_ref().map(|auth| auth.config());
    let mut response = redirect(next);
    append_cookie(
        &mut response,
        &preference_cookie("ferrum_locale", chosen, config),
    );
    append_cookie(
        &mut response,
        &preference_cookie("ferrum_theme", theme, config),
    );
    response
}

async fn dashboard(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_get(&state, &headers, "/admin").await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(auth) = state.auth.as_ref() else {
        return error_page(
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            false,
            "",
        );
    };
    let mut models = Vec::new();
    for model in registry.models() {
        if permit(auth, &user, model.meta(), "read").await.is_ok() && !list_blocked(model.as_ref())
        {
            models.push(json!({ "key": model.meta().key, "label": model.meta().label }));
        }
    }
    let csrf = session_csrf(auth, &headers).await;
    page(
        &registry,
        "dashboard.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)["dashboard"]
            .as_str()
            .unwrap_or("Dashboard"),
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        "/admin",
        &flash(
            &locale,
            query.get("notice").map(String::as_str).unwrap_or(""),
        ),
        "",
        json!({ "models": models, "user_email": user.email }),
    )
}

async fn model_index(State(state): State<AppState>, headers: HeaderMap) -> Response {
    require_get(&state, &headers, "/admin/models")
        .await
        .map_or_else(|response| response, |_| redirect("/admin"))
}

async fn model_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    RawQuery(raw): RawQuery,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_get(&state, &headers, &format!("/admin/models/{model_key}")).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    };
    if gate(&state, &user, model.as_ref(), "read").await.is_err() || list_blocked(model.as_ref()) {
        return error_page(
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        );
    }
    let page_number = params
        .get("page")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    let sort = params.get("sort").map(String::as_str).unwrap_or("id");
    if !model
        .meta()
        .fields
        .iter()
        .any(|field| field.name == sort && (field.sortable || field.name == "id"))
    {
        return error_page(
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    let filters = match filters_from(raw.as_deref().unwrap_or(""), model.columns(), model.meta()) {
        Ok(filters) => filters,
        Err(_) => {
            return error_page(
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &user.email,
            )
        }
    };
    let filter_refs: Vec<(&str, DbValue)> = filters
        .iter()
        .map(|(name, value)| (*name, value.clone()))
        .collect();
    let search_columns: Vec<&str> = model
        .meta()
        .fields
        .iter()
        .filter(|field| field.searchable)
        .map(|field| field.name)
        .collect();
    let query = params.get("q").cloned().unwrap_or_default();
    if !query.is_empty() && search_columns.is_empty() {
        return error_page(
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    let scope = access_scope(model.as_ref(), &user, false);
    let page_size = 25_u64;
    let listed = model
        .page(
            &state.db,
            &search_columns,
            (!query.is_empty()).then_some(query.as_str()),
            sort,
            params.get("dir").map(String::as_str) == Some("desc"),
            &filter_refs,
            u64::from(page_number - 1) * page_size,
            page_size,
            &scope,
        )
        .await;
    let (mut rows, has_more, total) = match listed {
        Ok(value) => value,
        Err(_) => {
            return error_page(
                &registry,
                &locale,
                StatusCode::INTERNAL_SERVER_ERROR,
                "forbidden",
                true,
                &user.email,
            )
        }
    };
    for row in &mut rows {
        redact(model.meta(), row);
    }
    let columns = list_columns(
        model.meta(),
        &model_key,
        &query,
        params.get("dir").map(String::as_str).unwrap_or("asc"),
        sort,
    );
    let rendered_rows: Vec<Value> = rows.iter().map(|row| json!({
        "id": row.get("id").and_then(Value::as_str).unwrap_or(""),
        "cells": columns.iter().map(|column| display_cell(&locale, registry.branding().currency.as_str(), column["name"].as_str().unwrap_or(""), row.get(column["name"].as_str().unwrap_or("")).unwrap_or(&Value::Null))).collect::<Vec<_>>()
    })).collect();
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    let list_path = format!("/admin/models/{model_key}");
    let preserved = preserve_query(&params, page_number);
    page(
        &registry,
        "list.html",
        &locale,
        theme_of(&headers),
        model.meta().label,
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        &list_path,
        &flash(
            &locale,
            params.get("notice").map(String::as_str).unwrap_or(""),
        ),
        "",
        json!({
            "model_label": model.meta().label,
            "list_path": list_path,
            "query": query,
            "filters": filter_inputs(model.meta(), model.columns(), &params),
            "can_create": permit(state.auth.as_ref().unwrap(), &user, model.meta(), "create").await.is_ok(),
            "columns": columns,
            "rows": rendered_rows,
            "total": total,
            "shown": rows.len(),
            "page": page_number,
            "previous": (page_number > 1).then(|| format!("{list_path}?{preserved}&page={}", page_number - 1)),
            "next_page": has_more.then(|| format!("{list_path}?{preserved}&page={}", page_number + 1))
        }),
    )
}

async fn model_new(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
) -> Response {
    form_page(
        &state,
        &headers,
        &registry,
        &model_key,
        None,
        Map::new(),
        "",
    )
    .await
}

async fn model_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
    body: Bytes,
) -> Response {
    save_form(&state, &headers, &registry, &model_key, None, &body).await
}

async fn model_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_get(&state, &headers, &format!("/admin/models/{model_key}/{id}")).await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    };
    if gate(&state, &user, model.as_ref(), "read").await.is_err() {
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    }
    let Some(mut record) = read_visible(&state, &user, model.as_ref(), id, "read").await else {
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    };
    redact(model.meta(), &mut record);
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    let list_path = format!("/admin/models/{model_key}");
    page(
        &registry,
        "detail.html",
        &locale,
        theme_of(&headers),
        model.meta().label,
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        &list_path,
        &flash(
            &locale,
            query.get("notice").map(String::as_str).unwrap_or(""),
        ),
        "",
        json!({
            "model_label": model.meta().label,
            "list_path": list_path,
            "id": id.to_string(),
            "fields": visible_fields(&locale, registry.branding().currency.as_str(), model.meta(), model.columns(), &record),
            "can_update": permit(state.auth.as_ref().unwrap(), &user, model.meta(), "update").await.is_ok(),
            "can_delete": permit(state.auth.as_ref().unwrap(), &user, model.meta(), "delete").await.is_ok()
        }),
    )
}

async fn model_edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
) -> Response {
    if let Err(response) = require_get(
        &state,
        &headers,
        &format!("/admin/models/{model_key}/{id}/edit"),
    )
    .await
    {
        return response;
    }
    let Some(record) = load_for_form(&state, &headers, &registry, &model_key, id).await else {
        let locale = locale_of(
            &headers,
            &registry.branding().default_locale,
            &registry.branding().fallback_locale,
        );
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            "",
        );
    };
    form_page(
        &state,
        &headers,
        &registry,
        &model_key,
        Some(id),
        record,
        "",
    )
    .await
}

async fn model_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
    body: Bytes,
) -> Response {
    save_form(&state, &headers, &registry, &model_key, Some(id), &body).await
}

async fn model_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_get(
        &state,
        &headers,
        &format!("/admin/models/{model_key}/{id}/delete"),
    )
    .await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    };
    if gate(&state, &user, model.as_ref(), "delete").await.is_err()
        || read_visible(&state, &user, model.as_ref(), id, "delete")
            .await
            .is_none()
    {
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    }
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    page(
        &registry,
        "delete.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)["delete"].as_str().unwrap_or("Delete"),
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        &format!("/admin/models/{model_key}"),
        "",
        "",
        json!({
            "action": format!("/admin/models/{model_key}/{id}/delete"),
            "cancel": format!("/admin/models/{model_key}/{id}"),
            "label": id.to_string()
        }),
    )
}

async fn model_remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_post(&state, &headers, &body).await {
        Ok(user) => user,
        Err(_) => return error_page(&registry, &locale, StatusCode::FORBIDDEN, "csrf", true, ""),
    };
    let Some(model) = registry.find(&model_key) else {
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    };
    if gate(&state, &user, model.as_ref(), "delete").await.is_err()
        || read_visible(&state, &user, model.as_ref(), id, "delete")
            .await
            .is_none()
    {
        return error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    }
    let scope = access_scope(model.as_ref(), &user, true);
    match model.remove(&state.db, id, &scope).await {
        Ok(ferrum_db::DeleteOutcome::Deleted) => {
            redirect(&format!("/admin/models/{model_key}?notice=deleted"))
        }
        _ => error_page(
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        ),
    }
}

async fn users(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_directory(&state, &headers, "/admin/directory/users").await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let auth = state.auth.as_ref().unwrap();
    let listed = auth.list_users().await.unwrap_or_default();
    let links = auth.list_user_role_names().await.unwrap_or_default();
    let mut roles: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (id, name) in links {
        roles.entry(id).or_default().push(name);
    }
    let query = params.get("q").cloned().unwrap_or_default();
    let mut rows: Vec<_> = listed
        .into_iter()
        .filter(|candidate| {
            query.is_empty()
                || candidate
                    .email
                    .to_ascii_lowercase()
                    .contains(&query.to_ascii_lowercase())
        })
        .collect();
    let descending = params.get("dir").map(String::as_str) == Some("desc");
    rows.sort_by(|left, right| {
        if descending {
            right.email.cmp(&left.email)
        } else {
            left.email.cmp(&right.email)
        }
    });
    let page_number = params
        .get("page")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let total = rows.len();
    let start = (page_number - 1) * 25;
    let slice = rows.into_iter().skip(start).take(25);
    let users: Vec<Value> = slice
        .map(|candidate| {
            let mut names = roles.remove(&candidate.id).unwrap_or_default();
            names.sort();
            json!({
                "id": candidate.id,
                "email": candidate.email,
                "is_active": candidate.is_active,
                "is_staff": candidate.is_staff,
                "is_superuser": candidate.is_superuser,
                "roles": names.join(", "),
                "mutable": !candidate.is_superuser && candidate.id != user.id
            })
        })
        .collect();
    let csrf = session_csrf(auth, &headers).await;
    page(
        &registry,
        "users.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)["users"].as_str().unwrap_or("Users"),
        true,
        true,
        &user.email,
        &csrf,
        "/admin/directory/users",
        &flash(
            &locale,
            params.get("notice").map(String::as_str).unwrap_or(""),
        ),
        "",
        json!({
            "users": users,
            "query": query,
            "total": total,
            "email_dir": if descending { "asc" } else { "desc" },
            "previous": (page_number > 1).then(|| format!("/admin/directory/users?q={query}&page={}", page_number - 1)),
            "next_page": (start + 25 < total).then(|| format!("/admin/directory/users?q={query}&page={}", page_number + 1))
        }),
    )
}

async fn user_active(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(id): Path<Uuid>,
    body: Bytes,
) -> Response {
    mutate_user(&state, &headers, &registry, id, &body, true).await
}

async fn user_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(id): Path<Uuid>,
    body: Bytes,
) -> Response {
    mutate_user(&state, &headers, &registry, id, &body, false).await
}

async fn roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_directory(&state, &headers, "/admin/directory/roles").await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let listed = state
        .auth
        .as_ref()
        .unwrap()
        .list_roles()
        .await
        .unwrap_or_default();
    let roles: Vec<Value> = listed
        .into_iter()
        .map(|(_, name)| json!({ "name": name }))
        .collect();
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    page(
        &registry,
        "roles.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)["roles"].as_str().unwrap_or("Roles"),
        true,
        true,
        &user.email,
        &csrf,
        "/admin/directory/roles",
        "",
        "",
        json!({ "roles": roles }),
    )
}

async fn role_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_directory_post(&state, &headers, &registry, &body).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let name = parse_form(&body).get("name").cloned().unwrap_or_default();
    if !identifier(&name) {
        return error_page(
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    if state
        .auth
        .as_ref()
        .unwrap()
        .ensure_role(&name)
        .await
        .is_err()
    {
        return error_page(
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    redirect("/admin/directory/roles?notice=updated")
}

async fn permissions_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_directory(&state, &headers, "/admin/directory/permissions").await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let auth = state.auth.as_ref().unwrap();
    let listed = auth.list_roles().await.unwrap_or_default();
    let mut roles = Vec::new();
    for (_, name) in listed {
        if let Some(selected) = params.get("role") {
            if selected != &name {
                continue;
            }
        }
        let permissions = auth.list_role_permissions(&name).await.unwrap_or_default();
        roles.push(json!({ "name": name, "permissions": permissions }));
    }
    let csrf = session_csrf(auth, &headers).await;
    page(
        &registry,
        "permissions.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)["permissions"]
            .as_str()
            .unwrap_or("Permissions"),
        true,
        true,
        &user.email,
        &csrf,
        "/admin/directory/permissions",
        "",
        "",
        json!({ "roles": roles }),
    )
}

async fn permission_change(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(role): Path<String>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_directory_post(&state, &headers, &registry, &body).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let form = parse_form(&body);
    let code = form.get("code").cloned().unwrap_or_default();
    if !identifier(&role) || !identifier(&code) {
        return error_page(
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    let auth = state.auth.as_ref().unwrap();
    let result = if form.get("action").map(String::as_str) == Some("grant") {
        auth.ensure_permission(&code)
            .await
            .and(auth.grant_permission(&role, &code).await)
    } else {
        auth.revoke_permission(&role, &code).await
    };
    if result.is_err() {
        return error_page(
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    redirect("/admin/directory/permissions?notice=updated")
}

async fn assignment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_directory_post(&state, &headers, &registry, &body).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let form = parse_form(&body);
    let email = form.get("email").cloned().unwrap_or_default();
    let role = form.get("role").cloned().unwrap_or_default();
    if !identifier(&role) {
        return error_page(
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    let auth = state.auth.as_ref().unwrap();
    let result = if form.get("action").map(String::as_str) == Some("revoke") {
        auth.revoke_role(&email, &role).await
    } else {
        auth.assign_role(&email, &role).await
    };
    if result.is_err() {
        return error_page(
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    redirect("/admin/directory/users?notice=updated")
}

async fn form_page(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    model_key: &str,
    id: Option<Uuid>,
    values: Map<String, Value>,
    alert: &str,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let back = format!("/admin/models/{model_key}");
    let user = match require_get(state, headers, &back).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(model_key) else {
        return error_page(
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    };
    let action_name = if id.is_some() { "update" } else { "create" };
    if gate(state, &user, model.as_ref(), action_name)
        .await
        .is_err()
    {
        return error_page(
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        );
    }
    if id.is_some() && values.is_empty() {
        return error_page(
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    }
    let csrf = session_csrf(state.auth.as_ref().unwrap(), headers).await;
    let fields = input_fields(&locale, registry, state, &user, model.as_ref(), &values).await;
    let action = match id {
        Some(id) => format!("/admin/models/{model_key}/{id}/edit"),
        None => format!("/admin/models/{model_key}/new"),
    };
    let list_path = back.clone();
    let cancel = id
        .map(|value| format!("/admin/models/{model_key}/{value}"))
        .unwrap_or_else(|| back.clone());
    page(
        registry,
        "form.html",
        &locale,
        theme_of(headers),
        model.meta().label,
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        &back,
        "",
        alert,
        json!({
            "model_label": model.meta().label,
            "list_path": list_path,
            "action": action,
            "cancel": cancel,
            "fields": fields
        }),
    )
}

async fn save_form(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    model_key: &str,
    id: Option<Uuid>,
    body: &Bytes,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_post(state, headers, body).await {
        Ok(user) => user,
        Err(_) => return error_page(registry, &locale, StatusCode::FORBIDDEN, "csrf", true, ""),
    };
    let Some(model) = registry.find(model_key) else {
        return error_page(
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    };
    let action_name = if id.is_some() { "update" } else { "create" };
    if gate(state, &user, model.as_ref(), action_name)
        .await
        .is_err()
    {
        return error_page(
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        );
    }
    let form = parse_form(body);
    let (object, mut raw) = payload(model.meta(), model.columns(), &form);
    if let Err(message) = reject_locked(model.meta(), &object, id.is_none()) {
        return form_page(
            state,
            headers,
            registry,
            model_key,
            id,
            raw,
            &validation_text(&locale, &message),
        )
        .await;
    }
    let result = if let Some(id) = id {
        if read_visible(state, &user, model.as_ref(), id, "update")
            .await
            .is_none()
        {
            return error_page(
                registry,
                &locale,
                StatusCode::NOT_FOUND,
                "not_found",
                true,
                &user.email,
            );
        }
        model
            .update(
                &state.db,
                id,
                Value::Object(object),
                &access_scope(model.as_ref(), &user, true),
            )
            .await
    } else {
        model.create(&state.db, Value::Object(object), &user).await
    };
    match result {
        Ok(created) => {
            let target = created
                .get("id")
                .and_then(Value::as_str)
                .map(|value| {
                    format!(
                        "/admin/models/{model_key}/{value}?notice={}",
                        if id.is_some() { "saved" } else { "created" }
                    )
                })
                .unwrap_or_else(|| format!("/admin/models/{model_key}?notice=saved"));
            redirect(&target)
        }
        Err(FerrumError::Validation(message)) => {
            let field = message.split_whitespace().next().unwrap_or("").to_owned();
            raw.insert(
                "__error".into(),
                json!({ "field": field, "text": validation_text(&locale, &message) }),
            );
            form_page(
                state,
                headers,
                registry,
                model_key,
                id,
                raw,
                &validation_text(&locale, &message),
            )
            .await
        }
        Err(FerrumError::NotFound) => error_page(
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        ),
        Err(_) => error_page(
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        ),
    }
}

async fn load_for_form(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    model_key: &str,
    id: Uuid,
) -> Option<Map<String, Value>> {
    let user = require_get(state, headers, "/admin").await.ok()?;
    let model = registry.find(model_key)?;
    let mut record = read_visible(state, &user, model.as_ref(), id, "update").await?;
    redact(model.meta(), &mut record);
    record.as_object().cloned()
}

async fn read_visible(
    state: &AppState,
    user: &AuthUser,
    model: &dyn ManagedModel,
    id: Uuid,
    action: &str,
) -> Option<Value> {
    if list_blocked(model) {
        return None;
    }
    let scope = access_scope(model, user, action != "read");
    let value = model.read(&state.db, id, &scope).await.ok()??;
    if model.policy_bound()
        && model.descriptor().requires_object_policy
        && !user.is_superuser
        && !model.object_allows(user, action, &value)
    {
        return None;
    }
    Some(value)
}

async fn input_fields(
    _locale: &str,
    registry: &AdminRegistry,
    state: &AppState,
    user: &AuthUser,
    model: &dyn ManagedModel,
    values: &Map<String, Value>,
) -> Vec<Value> {
    let error_field = values
        .get("__error")
        .and_then(|value| value.get("field"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let error_text = values
        .get("__error")
        .and_then(|value| value.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut fields = Vec::new();
    for field in model.meta().fields {
        if field.hidden || field.readonly || sensitive(field.name) || field.name == "id" {
            continue;
        }
        let Some(column) = model
            .columns()
            .iter()
            .find(|column| column.name == field.name)
        else {
            continue;
        };
        let current = values.get(field.name).cloned().unwrap_or(Value::Null);
        let mut input = json!({
            "name": field.name,
            "label": field.label,
            "required": !column.nullable && column.default_bool.is_none(),
            "invalid": error_field == field.name,
            "error": if error_field == field.name { error_text } else { "" },
            "value": match &current {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                Value::Bool(flag) => flag.to_string(),
                _ => String::new(),
            },
            "checked": current.as_bool().unwrap_or(false) || current.as_str() == Some("true")
        });
        if column.foreign_table.is_some() {
            input["kind"] = json!("select");
            input["options"] = relation_options(
                registry,
                state,
                user,
                column,
                current.as_str().unwrap_or(""),
            )
            .await;
            input["input"] = json!("text");
        } else if column.kind == ColumnKind::Boolean {
            input["kind"] = json!("boolean");
        } else {
            input["kind"] = json!("text");
            input["input"] = json!(match column.kind {
                ColumnKind::BigInt => "number",
                ColumnKind::Uuid => "text",
                _ => "text",
            });
        }
        fields.push(input);
    }
    fields
}

async fn relation_options(
    registry: &AdminRegistry,
    state: &AppState,
    user: &AuthUser,
    column: &ColumnSpec,
    current: &str,
) -> Value {
    let Some(table) = column.foreign_table else {
        return json!([]);
    };
    let Some(target) = registry
        .models()
        .iter()
        .find(|candidate| candidate.descriptor().table == table)
    else {
        return json!([]);
    };
    if list_blocked(target.as_ref())
        || permit(state.auth.as_ref().unwrap(), user, target.meta(), "read")
            .await
            .is_err()
    {
        return json!([]);
    }
    let Some(label) = display_column(target.meta()) else {
        return json!([]);
    };
    let scope = access_scope(target.as_ref(), user, false);
    let Ok((rows, _, _)) = target
        .page(&state.db, &[], None, label, false, &[], 0, 100, &scope)
        .await
    else {
        return json!([]);
    };
    Value::Array(rows.into_iter().filter_map(|row| {
        let id = row.get("id")?.as_str()?;
        Some(json!({ "id": id, "label": row.get(label)?.as_str().unwrap_or(""), "selected": id == current }))
    }).collect())
}

fn payload(
    meta: &ferrum_core::AdminModelMeta,
    columns: &[ColumnSpec],
    form: &HashMap<String, String>,
) -> (Map<String, Value>, Map<String, Value>) {
    let mut object = Map::new();
    let mut raw = Map::new();
    for (key, value) in form {
        if key == "csrf_token" || key == "next" {
            continue;
        }
        raw.insert(key.clone(), json!(value));
        let Some(field) = meta.fields.iter().find(|field| field.name == key) else {
            object.insert(key.clone(), json!(value));
            continue;
        };
        let Some(column) = columns.iter().find(|column| column.name == field.name) else {
            continue;
        };
        object.insert(key.clone(), typed_value(column, value));
    }
    (object, raw)
}

fn typed_value(column: &ColumnSpec, value: &str) -> Value {
    if value.is_empty() && column.nullable {
        return Value::Null;
    }
    match column.kind {
        ColumnKind::Boolean => json!(matches!(value, "true" | "1" | "on")),
        ColumnKind::BigInt => value
            .parse::<i64>()
            .map(|number| json!(number))
            .unwrap_or_else(|_| json!(value)),
        _ => json!(value),
    }
}

fn visible_fields(
    locale: &str,
    currency: &str,
    meta: &ferrum_core::AdminModelMeta,
    _columns: &[ColumnSpec],
    record: &Value,
) -> Vec<Value> {
    meta.fields.iter().filter(|field| !field.hidden && !sensitive(field.name)).map(|field| json!({
        "label": field.label,
        "readonly": field.readonly || field.name == "id",
        "value": display_cell(locale, currency, field.name, record.get(field.name).unwrap_or(&Value::Null))
    })).collect::<Vec<_>>()
}

fn list_columns(
    meta: &ferrum_core::AdminModelMeta,
    model: &str,
    query: &str,
    dir: &str,
    sort: &str,
) -> Vec<Value> {
    meta.fields.iter().filter(|field| field.list_display && !field.hidden && !sensitive(field.name)).map(|field| {
        let next_dir = if sort == field.name && dir != "desc" { "desc" } else { "asc" };
        json!({
            "name": field.name,
            "label": field.label,
            "mark": if sort == field.name { if dir == "desc" { "↓" } else { "↑" } } else { "" },
            "sort_href": format!("/admin/models/{model}?q={}&sort={}&dir={next_dir}", url_encode(query), field.name)
        })
    }).collect()
}

fn filter_inputs(
    meta: &ferrum_core::AdminModelMeta,
    _columns: &[ColumnSpec],
    params: &HashMap<String, String>,
) -> Vec<Value> {
    meta.fields
        .iter()
        .filter(|field| field.filterable && !field.hidden)
        .map(|field| {
            json!({
                "name": field.name,
                "label": field.label,
                "value": params.get(&format!("filter.{}", field.name)).cloned().unwrap_or_default()
            })
        })
        .collect()
}

fn display_cell(locale: &str, currency: &str, name: &str, value: &Value) -> String {
    match value {
        Value::Bool(true) => catalog(locale)["yes"].as_str().unwrap_or("Yes").to_owned(),
        Value::Bool(false) => catalog(locale)["no"].as_str().unwrap_or("No").to_owned(),
        Value::Number(number) if name.ends_with("_cents") => {
            let cents = number.as_i64().unwrap_or(0);
            let sign = if cents < 0 { "-" } else { "" };
            format!(
                "{sign}{} {:.2}",
                currency,
                (cents.unsigned_abs() as f64) / 100.0
            )
        }
        Value::Number(number) => group_int(locale, number.as_i64().unwrap_or(0)),
        Value::String(text) => text.clone(),
        Value::Null => "—".to_owned(),
        other => other.to_string(),
    }
}

fn group_int(locale: &str, value: i64) -> String {
    let separator = if locale == "de" { '.' } else { ',' };
    let digits: Vec<char> = value.unsigned_abs().to_string().chars().collect();
    let mut output = String::new();
    for (index, char) in digits.iter().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(separator);
        }
        output.push(*char);
    }
    if value < 0 {
        format!("-{output}")
    } else {
        output
    }
}

async fn mutate_user(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    id: Uuid,
    body: &Bytes,
    active_flag: bool,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_directory_post(state, headers, registry, body).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let form = parse_form(body);
    let auth = state.auth.as_ref().unwrap();
    let Some(target) = auth
        .list_users()
        .await
        .ok()
        .and_then(|users| users.into_iter().find(|candidate| candidate.id == id))
    else {
        return error_page(
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        );
    };
    if target.is_superuser
        || (user.id == id
            && form_flag(&form, if active_flag { "active" } else { "staff" }) == Some(false))
    {
        return error_page(
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        );
    }
    let result = if active_flag {
        auth.set_user_active(&target.email, form_flag(&form, "active").unwrap_or(false))
            .await
    } else {
        auth.set_staff(id, form_flag(&form, "staff").unwrap_or(false))
            .await
    };
    if result.is_err() {
        return error_page(
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        );
    }
    redirect("/admin/directory/users?notice=updated")
}

#[allow(clippy::result_large_err)]
async fn require_get(
    state: &AppState,
    headers: &HeaderMap,
    next: &str,
) -> Result<AuthUser, Response> {
    let Some(auth) = state.auth.as_ref() else {
        return Err(plain(
            StatusCode::FORBIDDEN,
            "authentication is not configured",
        ));
    };
    match auth.authenticate(headers, &Method::GET).await {
        Ok(user) if user.is_staff => Ok(user),
        Ok(_) => Err(plain(StatusCode::FORBIDDEN, "staff access is required")),
        Err(AuthError::Unauthenticated) => Err(redirect(&format!(
            "/admin/login?next={}",
            url_encode(safe_next(next).unwrap_or("/admin"))
        ))),
        Err(_) => Err(plain(StatusCode::FORBIDDEN, "staff access is required")),
    }
}

async fn require_post(state: &AppState, headers: &HeaderMap, body: &Bytes) -> Result<AuthUser, ()> {
    let Some(auth) = state.auth.as_ref() else {
        return Err(());
    };
    let form = parse_form(body);
    let user = auth
        .authenticate_csrf(
            headers,
            &Method::POST,
            form.get("csrf_token").map(String::as_str).unwrap_or(""),
        )
        .await
        .map_err(|_| ())?;
    user.is_staff.then_some(user).ok_or(())
}

#[allow(clippy::result_large_err)]
async fn require_directory(
    state: &AppState,
    headers: &HeaderMap,
    next: &str,
) -> Result<AuthUser, Response> {
    let user = require_get(state, headers, next).await?;
    if user.is_superuser {
        Ok(user)
    } else {
        Err(plain(
            StatusCode::FORBIDDEN,
            "this directory action requires a superuser",
        ))
    }
}

#[allow(clippy::result_large_err)]
async fn require_directory_post(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    body: &Bytes,
) -> Result<AuthUser, Response> {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let Some(auth) = state.auth.as_ref() else {
        return Err(plain(
            StatusCode::FORBIDDEN,
            "authentication is not configured",
        ));
    };
    let form = parse_form(body);
    match auth
        .authenticate_csrf(
            headers,
            &Method::POST,
            form.get("csrf_token").map(String::as_str).unwrap_or(""),
        )
        .await
    {
        Ok(user) if user.is_superuser => Ok(user),
        Ok(user) => Err(error_page(
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        )),
        Err(_) => Err(error_page(
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "csrf",
            false,
            "",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn page(
    registry: &AdminRegistry,
    template: &str,
    locale: &str,
    theme: impl AsRef<str>,
    title: &str,
    signed_in: bool,
    superuser: bool,
    email: &str,
    csrf: &str,
    path: &str,
    notice_text: &str,
    alert: &str,
    extra: Value,
) -> Response {
    let theme = theme.as_ref();
    let branding = registry.branding();
    let mut value = json!({
        "locale": locale,
        "theme": theme,
        "title": title,
        "site_title": branding.site_title,
        "site_header": branding.site_header,
        "app_name": branding.app_name,
        "primary_color": branding.primary_color,
        "i18n": catalog(locale),
        "signed_in": signed_in,
        "superuser": superuser,
        "user_email": email,
        "csrf_token": csrf,
        "current_path": path,
        "notice": notice_text,
        "alert": alert
    });
    if let (Some(target), Some(source)) = (value.as_object_mut(), extra.as_object()) {
        for (key, item) in source {
            target.insert(key.clone(), item.clone());
        }
    }
    match render(template, value) {
        Ok(body) => Html(body).into_response(),
        Err(error) => {
            tracing::error!(%error, "admin template failed");
            plain(StatusCode::INTERNAL_SERVER_ERROR, "Request rejected")
        }
    }
}

fn error_page(
    registry: &AdminRegistry,
    locale: &str,
    status: StatusCode,
    key: &str,
    signed_in: bool,
    email: &str,
) -> Response {
    let message = catalog(locale)
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("Error")
        .to_owned();
    let mut response = page(
        registry,
        "error.html",
        locale,
        "system",
        &message,
        signed_in,
        false,
        email,
        "",
        "/admin",
        "",
        "",
        json!({ "message": message }),
    );
    *response.status_mut() = status;
    response
}

fn redirect(location: &str) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    response
}

fn plain(status: StatusCode, message: &str) -> Response {
    (status, message.to_owned()).into_response()
}

struct Html(String);
impl IntoResponse for Html {
    fn into_response(self) -> Response {
        let mut response = Response::new(axum::body::Body::from(self.0));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

fn locale_of(headers: &HeaderMap, default_locale: &str, fallback_locale: &str) -> String {
    let requested = cookie(headers, "ferrum_locale")
        .or_else(|| accept_language(headers))
        .unwrap_or_default();
    normalize(&requested, default_locale, fallback_locale)
}

fn theme_of(headers: &HeaderMap) -> String {
    match cookie(headers, "ferrum_theme").as_deref() {
        Some("light") => "light".to_owned(),
        Some("dark") => "dark".to_owned(),
        _ => "system".to_owned(),
    }
}

fn accept_language(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::ACCEPT_LANGUAGE)?.to_str().ok()?;
    if value.to_ascii_lowercase().contains("de") {
        Some("de".to_owned())
    } else if value.to_ascii_lowercase().contains("en") {
        Some("en".to_owned())
    } else {
        None
    }
}

async fn session_csrf(auth: &ferrum_auth::AuthService, headers: &HeaderMap) -> String {
    auth.csrf_from_request(headers).await.unwrap_or_default()
}

fn login_token(headers: &HeaderMap) -> String {
    cookie(headers, "ferrum_login_csrf")
        .filter(|value| valid_login_csrf(value))
        .unwrap_or_else(random_token)
}

fn login_gate(
    auth: &ferrum_auth::AuthService,
    headers: &HeaderMap,
    supplied: &str,
) -> Result<(), &'static str> {
    if auth.origin_allowed(headers, &Method::POST).is_err() {
        return Err("origin_rejected");
    }
    if supplied.is_empty() {
        return Err("form_token_missing");
    }
    let Some(expected) = cookie(headers, "ferrum_login_csrf") else {
        return Err("login_csrf_missing");
    };
    if !valid_login_csrf(&expected) || !constant_eq(&expected, supplied) {
        return Err("login_csrf_mismatch");
    }
    Ok(())
}

fn valid_login_csrf(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|char| char.is_ascii_hexdigit())
}

fn log_login_rejection(headers: &HeaderMap, reason: &'static str) {
    if reason != "origin_rejected" {
        tracing::warn!(reason, "admin login rejected");
        return;
    }
    let (source, class, port) = presented_origin_shape(headers);
    tracing::warn!(reason, source, class, port, "admin login rejected");
}

fn presented_origin_shape(headers: &HeaderMap) -> (&'static str, &'static str, u16) {
    let (source, raw) =
        if let Some(value) = headers.get("origin").and_then(|value| value.to_str().ok()) {
            ("origin", value)
        } else if let Some(value) = headers.get("referer").and_then(|value| value.to_str().ok()) {
            ("referer", value)
        } else {
            return ("absent", "absent", 0);
        };
    let value = raw.trim();
    if value.eq_ignore_ascii_case("null") {
        return (source, "null", 0);
    }
    let Some((scheme, rest)) = value.split_once("://") else {
        return (source, "malformed", 0);
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return (source, "malformed", 0);
    }
    let hostport = rest.split(['/', '?', '#', ' ', '@']).next().unwrap_or("");
    let (host, port) = if let Some(rest) = hostport.strip_prefix('[') {
        let Some((host, port)) = rest.split_once("]:") else {
            return (source, "malformed", 0);
        };
        (host, port)
    } else if let Some((host, port)) = hostport.rsplit_once(':') {
        (host, port)
    } else {
        (
            hostport,
            if scheme.eq_ignore_ascii_case("https") {
                "443"
            } else {
                "80"
            },
        )
    };
    let class = if host.eq_ignore_ascii_case("localhost") {
        "localhost"
    } else if host == "127.0.0.1" || host == "::1" {
        "loopback"
    } else if host.is_empty() {
        "malformed"
    } else {
        "other"
    };
    (source, class, port.parse().unwrap_or(0))
}

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn constant_eq(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |diff, (left, right)| diff | (left ^ right))
        == 0
}

fn login_cookie(config: Option<&AuthConfig>, token: &str) -> String {
    let secure = config.is_some_and(|config| config.cookie_secure);
    let same = match config.map(|config| config.same_site) {
        Some(CookieSameSite::Strict) => "Strict",
        _ => "Lax",
    };
    format!(
        "ferrum_login_csrf={token}; HttpOnly; Path=/; SameSite={same}; Max-Age=600{}",
        if secure { "; Secure" } else { "" }
    )
}

fn preference_cookie(name: &str, value: &str, config: Option<&AuthConfig>) -> String {
    let secure = config.is_some_and(|config| config.cookie_secure);
    let same = match config.map(|config| config.same_site) {
        Some(CookieSameSite::Strict) => "Strict",
        _ => "Lax",
    };
    format!(
        "{name}={value}; HttpOnly; Path=/; SameSite={same}; Max-Age=31536000{}",
        if secure { "; Secure" } else { "" }
    )
}

fn clear_cookie(name: &str, config: &AuthConfig) -> String {
    preference_cookie(name, "", Some(config)).replace("Max-Age=31536000", "Max-Age=0")
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|raw| {
            raw.split(';').find_map(|part| {
                let (key, value) = part.trim().split_once('=')?;
                (key == name).then(|| value.trim().to_owned())
            })
        })
}

fn append_cookie(response: &mut Response, value: &str) {
    if let Ok(header_value) = HeaderValue::from_str(value) {
        response
            .headers_mut()
            .append(header::SET_COOKIE, header_value);
    }
}

fn parse_form(body: &Bytes) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for pair in String::from_utf8_lossy(body).split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        values.insert(decode(key), decode(value));
    }
    values
}

fn decode(value: &str) -> String {
    let mut bytes = Vec::new();
    let input = value.as_bytes();
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'+' => {
                bytes.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < input.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    bytes.push(byte);
                    index += 3;
                } else {
                    bytes.push(b'%');
                    index += 1;
                }
            }
            byte => {
                bytes.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn safe_next(value: &str) -> Option<&str> {
    if value.starts_with("/admin")
        && !value.starts_with("//")
        && !value.contains('\\')
        && !value.contains("://")
        && !value.chars().any(|char| char.is_control())
        && value.len() <= 200
    {
        Some(value)
    } else {
        None
    }
}

fn url_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn flash(locale: &str, code: &str) -> String {
    notice(locale, code).unwrap_or_default()
}

fn preserve_query(params: &HashMap<String, String>, _page: u32) -> String {
    let mut pairs = Vec::new();
    for (key, value) in params {
        if key == "page" || key == "notice" {
            continue;
        }
        pairs.push(format!("{}={}", url_encode(key), url_encode(value)));
    }
    pairs.join("&")
}

fn form_flag(form: &HashMap<String, String>, name: &str) -> Option<bool> {
    match form.get(name).map(String::as_str) {
        Some("true") | Some("1") => Some(true),
        Some("false") | Some("0") => Some(false),
        _ => None,
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '.'))
}
