//! Serverseitige Admin-Seiten. Schreibende Aktionen sind POST und nutzen das bestehende CSRF.

use crate::actions::ActionOperation;
use crate::audit::{self, Event};
use crate::drafts;
use crate::http::{
    access_scope, display_column, filters_from, gate, list_blocked, permit, reject_locked,
};
use crate::localization::{bulk_notice, catalog, normalize, notice, validation_text};
use crate::registry::{redact, sensitive, AdminRegistry, ManagedModel};
use crate::rendering::render;
use axum::body::Bytes;
use axum::extract::{Extension, Path, Query, RawQuery, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use ferrioxid_auth::{AppState, AuthConfig, AuthError, AuthUser, CookieSameSite};
use ferrioxid_core::{ColumnKind, ColumnSpec, FerrioxidError};
use ferrioxid_db::{date_buckets, delete_record, get_record, query_labels, update_record};
use ferrioxid_forms::{
    reject_unknown, require_confirmation, required_bool, required_text, FormValues, TextSpec,
};
use sea_orm::{TransactionTrait, Value as DbValue};
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
            .route("/admin/audit", get(audit_page))
            .route("/admin/models", get(model_index))
            .route("/admin/models/{model}", get(model_list))
            .route("/admin/models/{model}/bulk", post(model_bulk))
            .route(
                "/admin/models/{model}/bulk/{draft}",
                get(bulk_confirm).post(bulk_apply),
            )
            .route("/admin/models/{model}/draft", post(model_draft))
            .route(
                "/admin/models/{model}/new",
                get(model_new).post(model_create),
            )
            .route("/admin/models/{model}/pick/{field}", get(model_pick))
            .route(
                "/admin/models/{model}/{id}/pick/{field}",
                get(model_record_pick),
            )
            .route("/admin/models/{model}/{id}/draft", post(model_record_draft))
            .route(
                "/admin/models/{model}/{id}/inline/{inline}/new",
                get(inline_new).post(inline_create),
            )
            .route(
                "/admin/models/{model}/{id}",
                get(model_detail).post(model_update),
            )
            .route(
                "/admin/models/{model}/{id}/edit",
                get(model_detail).post(model_update),
            )
            .route(
                "/admin/models/{model}/{id}/delete",
                get(model_delete).post(model_remove),
            )
            .route("/admin/directory/users", get(users))
            .route(
                "/admin/directory/users/new",
                get(user_new).post(user_create),
            )
            .route(
                "/admin/directory/users/{id}",
                get(user_detail).post(user_update),
            )
            .route(
                "/admin/directory/users/{id}/edit",
                get(user_edit).post(user_update),
            )
            .route("/admin/directory/users/{id}/password", post(user_password))
            .route("/admin/directory/users/{id}/active", post(user_active))
            .route("/admin/directory/users/{id}/staff", post(user_staff))
            .route("/admin/directory/roles", get(roles).post(role_create))
            .route(
                "/admin/directory/roles/new",
                get(role_new).post(role_create),
            )
            .route("/admin/directory/roles/{id}", get(role_detail))
            .route("/admin/directory/permissions", get(permissions_page))
            .route(
                "/admin/directory/roles/{role}/permissions",
                post(permission_change),
            )
            .route("/admin/directory/assignments", post(assignment))
            .route(
                "/admin/pages/{key}",
                get(crate::pages::show).post(crate::pages::submit),
            )
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
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            false,
            "",
        )
        .await;
    };
    let supplied = form.get("csrf_token").map(String::as_str).unwrap_or("");
    if let Err(reason) = login_gate(auth, &headers, supplied) {
        log_login_rejection(&headers, reason);
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "csrf",
            false,
            "",
        )
        .await;
    }
    let next =
        safe_next(form.get("next").map(String::as_str).unwrap_or("/admin")).unwrap_or("/admin");
    let anonymous = cookie(&headers, &auth.config().anonymous_cookie_name);
    match auth
        .login_replacing(
            form.get("email").map(String::as_str).unwrap_or(""),
            form.get("password").map(String::as_str).unwrap_or(""),
            &auth.client_key(&headers, "html"),
            anonymous.as_deref(),
        )
        .await
    {
        Ok(issued) => {
            let mut response = redirect(next);
            append_cookie(&mut response, &issued.cookie);
            for cookie in &issued.clear_cookies {
                append_cookie(&mut response, cookie);
            }
            append_cookie(
                &mut response,
                &clear_cookie("ferrioxid_login_csrf", auth.config()),
            );
            response
        }
        Err(AuthError::RateLimited) => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                false,
                "",
            )
            .await
        }
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
        Err(_) => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::FORBIDDEN,
                "forbidden",
                false,
                "",
            )
            .await
        }
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
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            false,
            "",
        )
        .await;
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
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "csrf",
            true,
            "",
        )
        .await;
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
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "csrf",
            true,
            "",
        )
        .await;
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
        &preference_cookie("ferrioxid_locale", chosen, config),
    );
    append_cookie(
        &mut response,
        &preference_cookie("ferrioxid_theme", theme, config),
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
    let user = match require_get(&state, &headers, &registry, "/admin").await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(_auth) = state.auth.as_ref() else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            false,
            "",
        )
        .await;
    };
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    page_as(
        &state,
        &user,
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
        json!({ "user_email": user.email }),
    )
    .await
}

async fn model_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_get(&state, &headers, &registry, "/admin/models").await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(auth) = state.auth.as_ref() else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    };
    let csrf = session_csrf(auth, &headers).await;
    page_as(
        &state,
        &user,
        &registry,
        "dashboard.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)["models"].as_str().unwrap_or("Models"),
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        "/admin/models",
        "",
        "",
        json!({ "user_email": user.email }),
    )
    .await
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
    let user = match require_get(
        &state,
        &headers,
        &registry,
        &format!("/admin/models/{model_key}"),
    )
    .await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if gate(&state, &user, model.as_ref(), "read").await.is_err() || list_blocked(model.as_ref()) {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    let page_number = params
        .get("page")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    let (sort, descending) = match resolved_sort(model.meta(), &params) {
        Ok(value) => value,
        Err(()) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &user.email,
            )
            .await;
        }
    };
    let filters = match filters_from(
        raw.as_deref().unwrap_or(""),
        model.columns(),
        model.meta(),
        registry.filters(),
    ) {
        Ok(filters) => filters,
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &user.email,
            )
            .await
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
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    let scope = access_scope(model.as_ref(), &user, false);
    let navigation = match date_navigation(
        &state,
        model.as_ref(),
        &locale,
        &params,
        &search_columns,
        (!query.is_empty()).then_some(query.as_str()),
        &filter_refs,
        &scope,
        &format!("/admin/models/{model_key}"),
    )
    .await
    {
        Ok(value) => value,
        Err(()) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &user.email,
            )
            .await;
        }
    };
    let page_size = u64::from(model.meta().per_page.clamp(1, 100));
    let listed = model
        .page(
            &state.db,
            &search_columns,
            (!query.is_empty()).then_some(query.as_str()),
            &sort,
            descending,
            &filter_refs,
            &navigation.bounds,
            u64::from(page_number - 1) * page_size,
            page_size,
            &scope,
        )
        .await;
    let (mut rows, has_more, total) = match listed {
        Ok(value) => value,
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::INTERNAL_SERVER_ERROR,
                "forbidden",
                true,
                &user.email,
            )
            .await
        }
    };
    for row in &mut rows {
        redact(model.meta(), row);
    }
    let columns = list_columns(
        model.meta(),
        model.columns(),
        &model_key,
        &params,
        &sort,
        descending,
    );
    let links = link_names(model.meta(), model.columns());
    let relations = match relation_names(&registry, &state, &user, model.columns(), &rows).await {
        Ok(relations) => relations,
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::INTERNAL_SERVER_ERROR,
                "forbidden",
                true,
                &user.email,
            )
            .await
        }
    };
    let rendered_rows: Vec<Value> = rows
        .iter()
        .map(|row| {
            let mut lead_taken = false;
            let cells: Vec<Value> = columns
                .iter()
                .map(|column| {
                    let name = column["name"].as_str().unwrap_or("");
                    let mut cell = present_cell(
                        &locale,
                        registry.branding().currency.as_str(),
                        model.columns(),
                        name,
                        row.get(name).unwrap_or(&Value::Null),
                        &relations,
                        user.is_superuser,
                        &mut lead_taken,
                    );
                    if links.contains(&name) {
                        cell["link"] = json!(true);
                    }
                    cell
                })
                .collect();
            json!({
            "id": row.get("id").and_then(Value::as_str).unwrap_or(""),
                    "cells": cells
                })
        })
        .collect();
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    let list_path = format!("/admin/models/{model_key}");
    let preserved = preserve_query(&params, page_number);
    let filters = filter_inputs(
        &locale,
        &registry,
        &state,
        &user,
        model.meta(),
        model.columns(),
        &params,
    )
    .await;
    let filtering = !query.is_empty()
        || filters.iter().any(|item| {
            item.get("value")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
        });
    let can_update = permit(state.auth.as_ref().unwrap(), &user, model.meta(), "update")
        .await
        .is_ok();
    let can_delete = permit(state.auth.as_ref().unwrap(), &user, model.meta(), "delete")
        .await
        .is_ok();
    let mut actions = Vec::new();
    for action in registry.actions_for(model.meta(), model.columns()) {
        if permit(
            state.auth.as_ref().unwrap(),
            &user,
            model.meta(),
            action.permission,
        )
        .await
        .is_ok()
        {
            actions.push(json!({
                "name": action.name,
                "label": if locale == "de" { action.label_de } else { action.label_en }
            }));
        }
    }
    let changed = params
        .get("changed")
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|count| *count <= 100);
    let notice = if params.get("notice").map(String::as_str) == Some("changed") {
        changed
            .map(|count| bulk_notice(&locale, count))
            .unwrap_or_default()
    } else {
        flash(
            &locale,
            params.get("notice").map(String::as_str).unwrap_or(""),
        )
    };
    page_as(
        &state,
        &user,
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
        &notice,
        "",
        json!({
            "model_key": model.meta().key,
            "model_label": model.meta().label,
            "actions": actions,
            "list_path": list_path,
            "query": query,
            "sort": if params.contains_key("sort") { sort.clone() } else { String::new() },
            "dir": if params.contains_key("dir") { if descending { "desc" } else { "asc" }.to_owned() } else { String::new() },
            "filters": filters,
            "filtering": filtering,
            "hierarchy": navigation.view,
            "can_create": permit(state.auth.as_ref().unwrap(), &user, model.meta(), "create").await.is_ok(),
            "can_update": can_update,
            "can_delete": can_delete,
            "columns": columns,
            "rows": rendered_rows,
            "total": total,
            "shown": rows.len(),
            "page": page_number,
            "previous": (page_number > 1).then(|| format!("{list_path}?{preserved}&page={}", page_number - 1)),
            "next_page": has_more.then(|| format!("{list_path}?{preserved}&page={}", page_number + 1))
        }),
    )
    .await
}

async fn model_new(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let mut values = Map::new();
    if params.contains_key("draft") {
        values.insert("_draft".to_owned(), json!(params.get("draft")));
        values.insert("_use_draft".to_owned(), json!(true));
    }
    if let Some(model) = registry.find(&model_key) {
        let mut picks = Map::new();
        apply_picks(model.meta(), model.columns(), &params, &mut picks);
        if !picks.is_empty() {
            values.insert("_picks".to_owned(), Value::Object(picks));
        }
    }
    let notice = flash(
        &locale_of(
            &headers,
            &registry.branding().default_locale,
            &registry.branding().fallback_locale,
        ),
        params.get("notice").map(String::as_str).unwrap_or(""),
    );
    form_page(
        &state,
        &headers,
        &registry,
        &model_key,
        None,
        values,
        FormFeedback {
            notice: &notice,
            alert: "",
        },
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
    let user = match require_get(
        &state,
        &headers,
        &registry,
        &format!("/admin/models/{model_key}/{id}"),
    )
    .await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if gate(&state, &user, model.as_ref(), "read").await.is_err() {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let Some(mut record) = read_visible(&state, &user, model.as_ref(), id, "read").await else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let can_update = gate(&state, &user, model.as_ref(), "update").await.is_ok()
        && read_visible(&state, &user, model.as_ref(), id, "update")
            .await
            .is_some();
    if can_update {
        return record_form(
            State(state),
            headers,
            Extension(registry),
            Path((model_key, id)),
            Query(query),
        )
        .await;
    }
    let can_delete = gate(&state, &user, model.as_ref(), "delete").await.is_ok()
        && read_visible(&state, &user, model.as_ref(), id, "delete")
            .await
            .is_some();
    redact(model.meta(), &mut record);
    let relations = relation_names(&registry, &state, &user, model.columns(), &[record.clone()])
        .await
        .unwrap_or_default();
    let fields = visible_fields(
        &locale,
        registry.branding().currency.as_str(),
        model.meta(),
        model.columns(),
        &record,
        &relations,
        user.is_superuser,
    );
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    let list_path = format!("/admin/models/{model_key}");
    page_as(
        &state,
        &user,
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
            "history_href": history_href(model.meta().key, &id.to_string()),
            "history": record_history(&state, &user, model.meta().key, &id.to_string()).await,
            "fields": fields,
            "can_update": false,
            "can_delete": can_delete,
            "inlines": inline_sections(&state, &registry, &user, model.as_ref(), id, &query, false, &locale).await
        }),
    )
    .await
}

async fn record_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    if let Err(response) = require_get(
        &state,
        &headers,
        &registry,
        &format!("/admin/models/{model_key}/{id}"),
    )
    .await
    {
        return response;
    }
    let Some(mut record) = load_for_form(&state, &headers, &registry, &model_key, id).await else {
        let locale = locale_of(
            &headers,
            &registry.branding().default_locale,
            &registry.branding().fallback_locale,
        );
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            "",
        )
        .await;
    };
    record.insert("_inline_query".to_owned(), json!(params));
    if params.contains_key("draft") {
        record.insert(
            "_draft".to_owned(),
            json!(params.get("draft").map(String::as_str).unwrap_or("")),
        );
        record.insert("_use_draft".to_owned(), json!(true));
    }
    if let Some(model) = registry.find(&model_key) {
        let mut picks = Map::new();
        apply_picks(model.meta(), model.columns(), &params, &mut picks);
        if !picks.is_empty() {
            record.insert("_picks".to_owned(), Value::Object(picks));
        }
    }
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let notice = flash(
        &locale,
        params.get("notice").map(String::as_str).unwrap_or(""),
    );
    form_page(
        &state,
        &headers,
        &registry,
        &model_key,
        Some(id),
        record,
        FormFeedback {
            notice: &notice,
            alert: "",
        },
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
        &registry,
        &format!("/admin/models/{model_key}/{id}/delete"),
    )
    .await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if gate(&state, &user, model.as_ref(), "delete").await.is_err() {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let Some(record) = read_visible(&state, &user, model.as_ref(), id, "delete").await else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let named = display_column(model.meta(), model.columns())
        .and_then(|name| record.get(name))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .to_owned();
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    page_as(
        &state,
        &user,
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
            "label": if named.is_empty() { id.to_string() } else { named },
            "record_id": id.to_string(),
            "model_label": model.meta().label,
            "list_path": format!("/admin/models/{model_key}")
        }),
    )
    .await
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
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::FORBIDDEN,
                "csrf",
                true,
                "",
            )
            .await
        }
    };
    let Some(model) = registry.find(&model_key) else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if gate(&state, &user, model.as_ref(), "delete").await.is_err()
        || read_visible(&state, &user, model.as_ref(), id, "delete")
            .await
            .is_none()
    {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let scope = access_scope(model.as_ref(), &user, true);
    match model.remove(&state.db, id, &scope, &user).await {
        Ok(ferrioxid_db::DeleteOutcome::Deleted) => {
            redirect(&format!("/admin/models/{model_key}?notice=deleted"))
        }
        _ => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::NOT_FOUND,
                "not_found",
                true,
                &user.email,
            )
            .await
        }
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
    let user = match require_directory(&state, &headers, &registry, "/admin/directory/users").await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let auth = state.auth.as_ref().unwrap();
    let listed = auth.list_users().await.unwrap_or_default();
    let mut emails: Vec<String> = listed
        .iter()
        .map(|candidate| candidate.email.clone())
        .collect();
    emails.sort();
    let mut role_choices: Vec<String> = auth
        .list_roles()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    role_choices.sort();
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
                "can_edit": !candidate.is_superuser,
                "mutable": !candidate.is_superuser && candidate.id != user.id
            })
        })
        .collect();
    let shown = users.len();
    let csrf = session_csrf(auth, &headers).await;
    page_as(
        &state,
        &user,
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
            "shown": shown,
            "page": page_number,
            "emails": emails,
            "role_choices": role_choices,
            "email_mark": if descending { "↓" } else { "↑" },
            "email_state": if descending { "descending" } else { "ascending" },
            "email_sort": format!(
                "/admin/directory/users?q={}&sort=email&dir={}",
                url_encode(&query),
                if descending { "asc" } else { "desc" }
            ),
            "email_dir": if descending { "asc" } else { "desc" },
            "previous": (page_number > 1).then(|| format!("/admin/directory/users?q={}&dir={}&page={}", url_encode(&query), if descending { "desc" } else { "asc" }, page_number - 1)),
            "next_page": (start + 25 < total).then(|| format!("/admin/directory/users?q={}&dir={}&page={}", url_encode(&query), if descending { "desc" } else { "asc" }, page_number + 1))
        }),
    )
    .await
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
    let user = match require_directory(&state, &headers, &registry, "/admin/directory/roles").await
    {
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
        .map(|(id, name)| {
            json!({
                "name": name,
                "href": format!("/admin/directory/roles/{id}"),
                "history": history_href("_ferrioxid_users", &name)
            })
        })
        .collect();
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    page_as(
        &state,
        &user,
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
    .await
}

async fn role_new(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user =
        match require_directory(&state, &headers, &registry, "/admin/directory/roles/new").await {
            Ok(user) => user,
            Err(response) => return response,
        };
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    page_as(
        &state,
        &user,
        &registry,
        "role_new.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)["add_role"]
            .as_str()
            .unwrap_or("New record"),
        true,
        true,
        &user.email,
        &csrf,
        "/admin/directory/roles/new",
        "",
        "",
        json!({}),
    )
    .await
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
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
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
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    redirect("/admin/directory/roles?notice=updated")
}

async fn permissions_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    role_permissions_page(state, headers, registry, params, None).await
}

async fn role_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    role_permissions_page(state, headers, registry, params, Some(id)).await
}

async fn role_permissions_page(
    state: AppState,
    headers: HeaderMap,
    registry: AdminRegistry,
    params: HashMap<String, String>,
    record_id: Option<Uuid>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_directory(&state, &headers, &registry, "/admin/directory/permissions")
        .await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let auth = state.auth.as_ref().unwrap();
    let listed = auth.list_roles().await.unwrap_or_default();
    let mut roles = Vec::new();
    for (id, name) in listed {
        if record_id.is_some_and(|selected| selected != id) {
            continue;
        }
        if record_id.is_none() {
            if let Some(selected) = params.get("role") {
                if selected != &name {
                    continue;
                }
            }
        }
        let permissions = auth.list_role_permissions(&name).await.unwrap_or_default();
        roles.push(json!({ "id": id, "name": name, "permissions": permissions }));
    }
    if record_id.is_some() && roles.is_empty() {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let csrf = session_csrf(auth, &headers).await;
    page_as(
        &state,
        &user,
        &registry,
        if record_id.is_some() { "role_detail.html" } else { "permissions.html" },
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
        &flash(&locale, params.get("notice").map(String::as_str).unwrap_or("")),
        "",
        json!({ "roles": roles, "record_role": record_id.is_some(),
            "history": if record_id.is_some() {
                record_history(&state, &user, "_ferrioxid_users", roles.first().and_then(|role| role["name"].as_str()).unwrap_or("")).await
            } else {
                Vec::<Value>::new()
            },
            "history_href": roles.first().and_then(|role| role["name"].as_str()).map(|name| history_href("_ferrioxid_users", name)).unwrap_or_default() }),
    )
    .await
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
    if !identifier(&role)
        || !identifier(&code)
        || !matches!(
            form.get("action").map(String::as_str),
            Some("grant" | "revoke")
        )
        || form.keys().any(|key| {
            !matches!(
                key.as_str(),
                "csrf_token" | "confirm" | "code" | "action" | "return_to"
            )
        })
    {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    if let Err(code) = confirmed(&body) {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            true,
            &user.email,
        )
        .await;
    }
    let auth = state.auth.as_ref().unwrap();
    let grant = form.get("action").map(String::as_str) == Some("grant");
    let result = crate::directory::grant_permission(auth, user.id, &role, &code, grant).await;
    if result.is_err() {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    if form.get("return_to").map(String::as_str) == Some("record") {
        if let Ok(roles) = auth.list_roles().await {
            if let Some((id, _)) = roles.iter().find(|(_, name)| name == &role) {
                return redirect(&format!("/admin/directory/roles/{id}?notice=updated"));
            }
        }
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
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    let auth = state.auth.as_ref().unwrap();
    let known = auth.list_roles().await.unwrap_or_default();
    if !known.iter().any(|(_, name)| name == &role) {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "unknown_role",
            true,
            &user.email,
        )
        .await;
    }
    if let Err(code) = confirmed(&body) {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            true,
            &user.email,
        )
        .await;
    }
    let result = crate::directory::assign_role(
        auth,
        user.id,
        &email,
        &role,
        form.get("action").map(String::as_str) != Some("revoke"),
    )
    .await;
    if result.is_err() {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    redirect("/admin/directory/users?notice=updated")
}

async fn model_pick(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, field)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    pick_page(
        &state, &headers, &registry, &model_key, None, &field, &params,
    )
    .await
}

async fn model_record_pick(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id, field)): Path<(String, Uuid, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    pick_page(
        &state,
        &headers,
        &registry,
        &model_key,
        Some(id),
        &field,
        &params,
    )
    .await
}

async fn pick_page(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    model_key: &str,
    record_id: Option<Uuid>,
    field: &str,
    params: &HashMap<String, String>,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let back = match record_id {
        Some(id) => format!("/admin/models/{model_key}/{id}"),
        None => format!("/admin/models/{model_key}/new"),
    };
    let user = match require_get(state, headers, registry, &back).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(model_key) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let action_name = if record_id.is_some() {
        "update"
    } else {
        "create"
    };
    if gate(state, &user, model.as_ref(), action_name)
        .await
        .is_err()
    {
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
    if let Some(id) = record_id {
        if load_for_form(state, headers, registry, model_key, id)
            .await
            .is_none()
        {
            return error_page(
                state,
                headers,
                registry,
                &locale,
                StatusCode::NOT_FOUND,
                "not_found",
                true,
                &user.email,
            )
            .await;
        }
    }
    let Some(column) = model
        .columns()
        .iter()
        .find(|column| column.name == field && column.foreign_table.is_some())
    else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if model
        .meta()
        .fields
        .iter()
        .any(|item| item.name == field && (item.hidden || item.readonly || sensitive(item.name)))
    {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let Some(target) = column
        .foreign_table
        .and_then(|table| registry.model_for_table(table))
    else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if list_blocked(target.as_ref())
        || permit(state.auth.as_ref().unwrap(), &user, target.meta(), "read")
            .await
            .is_err()
    {
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
    let Some(label) = display_column(target.meta(), target.columns()) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let page_number = params
        .get("page")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .clamp(1, 10_000);
    let page_size = u64::from(target.meta().per_page.clamp(1, 100));
    let search_columns: Vec<&str> = target
        .meta()
        .fields
        .iter()
        .filter(|item| item.searchable && !item.hidden && !sensitive(item.name))
        .map(|item| item.name)
        .collect();
    let query = params.get("q").cloned().unwrap_or_default();
    if query.chars().count() > 200 || (!query.is_empty() && search_columns.is_empty()) {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    let scope = access_scope(target.as_ref(), &user, false);
    let listed = target
        .page(
            &state.db,
            &search_columns,
            (!query.is_empty()).then_some(query.as_str()),
            label,
            false,
            &[],
            &[],
            u64::from(page_number - 1) * page_size,
            page_size,
            &scope,
        )
        .await;
    let (rows, has_more, total) = match listed {
        Ok(value) => value,
        Err(_) => {
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
            .await
        }
    };
    let picker = match record_id {
        Some(id) => format!("/admin/models/{model_key}/{id}/pick/{field}"),
        None => format!("/admin/models/{model_key}/pick/{field}"),
    };
    let mut preserved = if query.is_empty() {
        String::new()
    } else {
        format!("q={}", url_encode(&query))
    };
    if let Some(draft) = params
        .get("draft")
        .filter(|value| Uuid::parse_str(value).is_ok())
    {
        if !preserved.is_empty() {
            preserved.push('&');
        }
        preserved.push_str("draft=");
        preserved.push_str(&url_encode(draft));
    }
    let choices: Vec<Value> = rows
        .iter()
        .filter_map(|row| {
            let id = row.get("id")?.as_str()?;
            let text = row.get(label)?.as_str().unwrap_or("");
            Some(json!({
                "label": text,
                "href": choice_href(&back, field, id, params.get("draft").map(String::as_str))
            }))
        })
        .collect();
    let join = if preserved.is_empty() { "" } else { "&" };
    let csrf = session_csrf(state.auth.as_ref().unwrap(), headers).await;
    page_as(
        state,
        &user,
        registry,
        "pick.html",
        &locale,
        theme_of(headers),
        catalog(&locale)
            .get("pick_heading")
            .and_then(Value::as_str)
            .unwrap_or("Choose"),
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        &picker,
        "",
        "",
        json!({
            "back": back,
            "action": picker,
            "query": query,
            "rows": choices,
            "total": total,
            "page": page_number,
            "previous": (page_number > 1).then(|| format!("{picker}?{preserved}{join}page={}", page_number - 1)),
            "next_page": has_more.then(|| format!("{picker}?{preserved}{join}page={}", page_number + 1))
        }),
    )
    .await
}

struct FormFeedback<'a> {
    notice: &'a str,
    alert: &'a str,
}

async fn form_page(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    model_key: &str,
    id: Option<Uuid>,
    values: Map<String, Value>,
    feedback: FormFeedback<'_>,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let back = format!("/admin/models/{model_key}");
    let user = match require_get(state, headers, registry, &back).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(model_key) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let action_name = if id.is_some() { "update" } else { "create" };
    if gate(state, &user, model.as_ref(), action_name)
        .await
        .is_err()
    {
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
    if id.is_some() && values.is_empty() {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let mut values = values;
    let inline_query: HashMap<String, String> = values
        .remove("_inline_query")
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let draft_id = take_draft(state, &user, model_key, id, &mut values).await;
    if let Some(id) = id {
        values
            .entry("id".to_owned())
            .or_insert_with(|| json!(id.to_string()));
    }
    let custom_action = values
        .get("_action")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let csrf = session_csrf(state.auth.as_ref().unwrap(), headers).await;
    let fields = input_fields(&locale, registry, state, &user, model.as_ref(), &values).await;
    let sections = form_sections(model.meta(), &fields);
    let (inline_groups, basis) = if model.meta().inlines.iter().any(|item| item.form) {
        crate::compose::groups(
            &state.db,
            state.auth.as_ref().unwrap(),
            &user,
            model.as_ref(),
            |key| registry.find(key).cloned(),
            id,
            &values,
        )
        .await
    } else {
        (Vec::new(), String::new())
    };
    let action = custom_action.unwrap_or_else(|| match id {
        Some(id) => format!("/admin/models/{model_key}/{id}"),
        None => format!("/admin/models/{model_key}/new"),
    });
    let list_path = back.clone();
    let cancel = values
        .get("_cancel")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| back.clone());
    let mut response = page_as(
        state,
        &user,
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
        feedback.notice,
        feedback.alert,
        json!({
            "model_key": model_key,
            "existing": id.is_some(),
            "can_create": gate(state, &user, model.as_ref(), "create").await.is_ok(),
            "inlines": if let Some(record_id) = id {
                inline_sections(state, registry, &user, model.as_ref(), record_id, &inline_query, true, &locale).await
            } else { Vec::new() },
            "delete_href": if let Some(record_id) = id {
                (gate(state, &user, model.as_ref(), "delete").await.is_ok()
                    && read_visible(state, &user, model.as_ref(), record_id, "delete").await.is_some())
                    .then(|| format!("/admin/models/{model_key}/{record_id}/delete"))
            } else {
                None
            },
            "history_href": id.map(|value| history_href(model.meta().key, &value.to_string())).unwrap_or_default(),
            "history": match id {
                Some(record_id) => record_history(state, &user, model.meta().key, &record_id.to_string()).await,
                None => Vec::new(),
            },
            "model_label": model.meta().label,
            "list_path": list_path,
            "action": action,
            "cancel": cancel,
            "draft": draft_id,
            "sections": sections,
            "inline_groups": inline_groups,
            "basis": basis
        }),
    )
    .await;
    if !feedback.alert.is_empty() {
        *response.status_mut() = StatusCode::UNPROCESSABLE_ENTITY;
    }
    response
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
        Err(_) => {
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
            .await
        }
    };
    let Some(model) = registry.find(model_key) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let action_name = if id.is_some() { "update" } else { "create" };
    if gate(state, &user, model.as_ref(), action_name)
        .await
        .is_err()
    {
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
    let form = parse_form(body);
    let (mut object, mut raw) = payload(model.meta(), model.columns(), &form);
    if let Err((field, code)) = normalize_temporal(model.columns(), &mut object) {
        let text = validation_text(&locale, code);
        raw.insert("__error".into(), json!({ "field": field, "text": text }));
        return form_page(
            state,
            headers,
            registry,
            model_key,
            id,
            raw,
            FormFeedback {
                notice: "",
                alert: &text,
            },
        )
        .await;
    }
    if let Err(message) = reject_locked(model.meta(), &object, id.is_none())
        .and_then(|()| crate::widgets::enforce(registry.widgets(), model.meta().key, &object))
    {
        let text = validation_text(&locale, &message);
        return form_page(
            state,
            headers,
            registry,
            model_key,
            id,
            raw,
            FormFeedback {
                notice: "",
                alert: &text,
            },
        )
        .await;
    }
    let save = form.get("save").map(String::as_str).unwrap_or("detail");
    let formed = model.meta().inlines.iter().any(|item| item.form);
    if formed && save == "rows" {
        return match crate::compose::store_extra(&state.db, &user, model.as_ref(), id, &form).await
        {
            Ok(draft) => {
                let target = match id {
                    Some(id) => format!("/admin/models/{model_key}/{id}?draft={draft}"),
                    None => format!("/admin/models/{model_key}/new?draft={draft}"),
                };
                redirect(&target)
            }
            Err(_) => {
                error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "inline_limit",
                    true,
                    &user.email,
                )
                .await
            }
        };
    }
    if !matches!(save, "detail" | "edit" | "new" | "list") {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    if let Err(field) = ensure_relations(state, registry, &user, model.columns(), &object).await {
        let text = catalog(&locale)
            .get("invalid_relation")
            .and_then(Value::as_str)
            .unwrap_or("Choose a visible record.")
            .to_owned();
        raw.insert("__error".into(), json!({ "field": field, "text": text }));
        return form_page(
            state,
            headers,
            registry,
            model_key,
            id,
            raw,
            FormFeedback {
                notice: "",
                alert: &text,
            },
        )
        .await;
    }
    if formed {
        return match crate::compose::save(
            &state.db,
            state.auth.as_ref().unwrap(),
            &user,
            model.as_ref(),
            |key| registry.find(key).cloned(),
            id,
            object,
            &form,
        )
        .await
        {
            crate::compose::SaveResult::Saved(saved) => {
                if let Some(token) = form
                    .get("_draft")
                    .and_then(|value| Uuid::parse_str(value).ok())
                {
                    let _ = drafts::delete(&state.db, token, user.session_id, user.id).await;
                }
                let notice = if id.is_some() { "saved" } else { "created" };
                let target = match save {
                    "list" => format!("/admin/models/{model_key}?notice={notice}"),
                    "edit" => format!("/admin/models/{model_key}/{saved}?notice={notice}"),
                    "new" => format!("/admin/models/{model_key}/new?notice={notice}"),
                    _ => format!("/admin/models/{model_key}/{saved}?notice={notice}"),
                };
                redirect(&target)
            }
            crate::compose::SaveResult::Invalid(values) => {
                let text = validation_text(
                    &locale,
                    values
                        .get("__error")
                        .and_then(|value| value.get("text"))
                        .and_then(|value| value.as_str())
                        .unwrap_or("validation.inline_failed"),
                );
                let mut response = form_page(
                    state,
                    headers,
                    registry,
                    model_key,
                    id,
                    values,
                    FormFeedback {
                        notice: "",
                        alert: &text,
                    },
                )
                .await;
                *response.status_mut() = StatusCode::UNPROCESSABLE_ENTITY;
                response
            }
            crate::compose::SaveResult::Conflict(values) => {
                let text = validation_text(&locale, "conflict.stale");
                let mut response = form_page(
                    state,
                    headers,
                    registry,
                    model_key,
                    id,
                    values,
                    FormFeedback {
                        notice: "",
                        alert: &text,
                    },
                )
                .await;
                *response.status_mut() = StatusCode::CONFLICT;
                response
            }
            crate::compose::SaveResult::Missing => {
                error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::NOT_FOUND,
                    "not_found",
                    true,
                    &user.email,
                )
                .await
            }
            crate::compose::SaveResult::Denied => {
                error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::FORBIDDEN,
                    "forbidden",
                    true,
                    &user.email,
                )
                .await
            }
        };
    }
    let result = if let Some(id) = id {
        if read_visible(state, &user, model.as_ref(), id, "update")
            .await
            .is_none()
        {
            return error_page(
                state,
                headers,
                registry,
                &locale,
                StatusCode::NOT_FOUND,
                "not_found",
                true,
                &user.email,
            )
            .await;
        }
        model
            .update(
                &state.db,
                id,
                Value::Object(object),
                &access_scope(model.as_ref(), &user, true),
                &user,
            )
            .await
    } else {
        model.create(&state.db, Value::Object(object), &user).await
    };
    match result {
        Ok(created) => {
            if let Some(token) = form
                .get("_draft")
                .and_then(|value| Uuid::parse_str(value).ok())
            {
                let _ = drafts::delete(&state.db, token, user.session_id, user.id).await;
            }
            let notice = if id.is_some() { "saved" } else { "created" };
            let target = created
                .get("id")
                .and_then(Value::as_str)
                .map(|value| match save {
                    "list" => format!("/admin/models/{model_key}?notice={notice}"),
                    "edit" => format!("/admin/models/{model_key}/{value}?notice={notice}"),
                    "new" => format!("/admin/models/{model_key}/new?notice={notice}"),
                    _ => format!("/admin/models/{model_key}/{value}?notice={notice}"),
                })
                .unwrap_or_else(|| format!("/admin/models/{model_key}?notice=saved"));
            redirect(&target)
        }
        Err(FerrioxidError::Validation(message)) => {
            let field = message.split_whitespace().next().unwrap_or("").to_owned();
            raw.insert(
                "__error".into(),
                json!({ "field": field, "text": validation_text(&locale, &message) }),
            );
            let text = validation_text(&locale, &message);
            form_page(
                state,
                headers,
                registry,
                model_key,
                id,
                raw,
                FormFeedback {
                    notice: "",
                    alert: &text,
                },
            )
            .await
        }
        Err(FerrioxidError::NotFound) => {
            error_page(
                state,
                headers,
                registry,
                &locale,
                StatusCode::NOT_FOUND,
                "not_found",
                true,
                &user.email,
            )
            .await
        }
        Err(_) => {
            error_page(
                state,
                headers,
                registry,
                &locale,
                StatusCode::FORBIDDEN,
                "forbidden",
                true,
                &user.email,
            )
            .await
        }
    }
}

async fn load_for_form(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    model_key: &str,
    id: Uuid,
) -> Option<Map<String, Value>> {
    let user = require_get(state, headers, registry, "/admin").await.ok()?;
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
    locale: &str,
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
    let hint_select = catalog(locale)
        .get("hint_select")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let mut fields = Vec::new();
    for field in model.meta().fields {
        if field.hidden || sensitive(field.name) {
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
        let locked = values.get("_lock").and_then(Value::as_str) == Some(field.name);
        if field.readonly || field.name == "id" || locked {
            if field.name == "id" && !values.contains_key(field.name) {
                continue;
            }
            fields.push(json!({
                "name": field.name,
                "label": field.label,
                "fieldset": field.fieldset,
                "kind": "readonly",
                "required": false,
                "invalid": false,
                "error": "",
                "hint": "",
                "pick": "",
                "value": display_cell(locale, registry.branding().currency.as_str(), field.name, &current),
                "raw": match &current {
                    Value::String(text) => text.clone(),
                    _ => String::new(),
                },
                "locked": locked,
                "checked": false
            }));
            continue;
        }
        let mut input = json!({
            "name": field.name,
            "label": field.label,
            "fieldset": field.fieldset,
            "pick": if column.foreign_table.is_some() { pick_path(model.meta().key, values, field.name) } else { String::new() },
            "required": !column.nullable && column.default_bool.is_none(),
            "invalid": error_field == field.name,
            "error": if error_field == field.name { error_text } else { "" },
            "hint": if column.foreign_table.is_some() { hint_select.clone() } else { String::new() },
            "value": match &current {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                Value::Bool(flag) => flag.to_string(),
                _ => String::new(),
            },
            "checked": match &current {
                Value::Bool(flag) => *flag,
                Value::String(text) => text == "true" || text == "1",
                Value::Null => column.default_bool.unwrap_or(false),
                _ => false,
            }
        });
        if let Some(widget) = registry
            .widgets()
            .iter()
            .find(|widget| widget.model_key == model.meta().key && widget.field == field.name)
        {
            if matches!(widget.kind, crate::widgets::WidgetKind::Readonly) {
                fields.push(json!({
                    "name": field.name,
                    "label": field.label,
                    "fieldset": field.fieldset,
                    "kind": "readonly",
                    "required": false,
                    "invalid": false,
                    "error": "",
                    "hint": "",
                    "pick": "",
                    "value": display_cell(locale, registry.branding().currency.as_str(), field.name, &current),
                    "raw": "",
                    "locked": false,
                    "checked": false
                }));
                continue;
            }
            let (kind, input_type) = crate::widgets::presentation(widget.kind);
            input["kind"] = json!(kind);
            input["input"] = json!(input_type);
        } else if column.foreign_table.is_some() {
            input["kind"] = json!("select");
            input["options"] = relation_options(
                registry,
                state,
                user,
                column,
                current.as_str().unwrap_or(""),
            )
            .await;
            // Retain an unavailable current relation instead of silently selecting NULL.
            if let Some(id) = current.as_str().filter(|id| !id.is_empty()) {
                if let Some(options) = input["options"].as_array_mut() {
                    if !options.iter().any(|option| option["selected"] == true) {
                        options.insert(0, json!({
                            "id": id, "selected": true,
                            "label": catalog(locale)[if user.is_superuser { "missing_relation" } else { "hidden_relation" }]
                        }));
                    }
                }
            }
            input["input"] = json!("text");
        } else if column.kind == ColumnKind::Choice {
            input["kind"] = json!("select");
            input["options"] = json!(column
                .choices
                .iter()
                .map(|choice| json!({
                    "id": choice.value,
                    "label": if locale == "de" { choice.label_de } else { choice.label_en },
                    "selected": current.as_str() == Some(choice.value),
                }))
                .collect::<Vec<_>>());
            input["input"] = json!("text");
        } else if column.kind == ColumnKind::Boolean {
            input["kind"] = json!("boolean");
        } else {
            input["kind"] = json!("text");
            input["input"] = json!(match column.kind {
                ColumnKind::BigInt => "number",
                ColumnKind::Date => "date",
                ColumnKind::Time => "time",
                ColumnKind::DateTime => "datetime-local",
                ColumnKind::Uuid | ColumnKind::Text | ColumnKind::Boolean | ColumnKind::Choice => {
                    "text"
                }
                ColumnKind::Decimal => "text",
            });
            if column.kind == ColumnKind::DateTime {
                if let Some(text) = input["value"].as_str() {
                    input["value"] = json!(ferrioxid_core::temporal::datetime_local_value(text));
                }
                input["hint"] = json!(catalog(locale)
                    .get("utc_hint")
                    .and_then(Value::as_str)
                    .unwrap_or("UTC"));
            }
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
    let Some(target) = registry.model_for_table(table) else {
        return json!([]);
    };
    if list_blocked(target.as_ref())
        || permit(state.auth.as_ref().unwrap(), user, target.meta(), "read")
            .await
            .is_err()
    {
        return json!([]);
    }
    let Some(label) = display_column(target.meta(), target.columns()) else {
        return json!([]);
    };
    let scope = access_scope(target.as_ref(), user, false);
    let Ok((rows, _, _)) = target
        .page(&state.db, &[], None, label, false, &[], &[], 0, 100, &scope)
        .await
    else {
        return json!([]);
    };
    let mut options: Vec<Value> = rows
        .into_iter()
        .filter_map(|row| {
            let id = row.get("id")?.as_str()?;
            Some(json!({
                "id": id,
                "label": row.get(label)?.as_str().unwrap_or(""),
                "selected": id == current
            }))
        })
        .collect();
    if !current.is_empty()
        && !options
            .iter()
            .any(|option| option.get("id").and_then(Value::as_str) == Some(current))
    {
        if let Ok(id) = Uuid::parse_str(current) {
            if let Some(record) = read_visible(state, user, target.as_ref(), id, "read").await {
                options.insert(
                    0,
                    json!({
                        "id": current,
                        "label": record.get(label).and_then(Value::as_str).unwrap_or(""),
                        "selected": true
                    }),
                );
            }
        }
    }
    Value::Array(options)
}

fn pick_path(model_key: &str, values: &Map<String, Value>, _field: &str) -> String {
    match values
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        Some(id) => format!("/admin/models/{model_key}/{id}/draft"),
        None => format!("/admin/models/{model_key}/draft"),
    }
}

fn apply_picks(
    meta: &ferrioxid_core::AdminModelMeta,
    columns: &[ColumnSpec],
    params: &HashMap<String, String>,
    values: &mut Map<String, Value>,
) {
    for field in meta.fields {
        if field.hidden || field.readonly {
            continue;
        }
        let Some(column) = columns.iter().find(|column| column.name == field.name) else {
            continue;
        };
        if column.foreign_table.is_none() {
            continue;
        }
        let Some(value) = params.get(field.name) else {
            continue;
        };
        if Uuid::parse_str(value).is_ok() {
            values.insert(field.name.to_owned(), json!(value));
        }
    }
}

fn form_sections(meta: &ferrioxid_core::AdminModelMeta, fields: &[Value]) -> Vec<Value> {
    if meta.fieldsets.is_empty() {
        return vec![json!({
            "name": "",
            "description": "",
            "optional": false,
            "fields": fields
        })];
    }
    meta.fieldsets
        .iter()
        .map(|section| {
            let group: Vec<&Value> = fields
                .iter()
                .filter(|field| field.get("fieldset").and_then(Value::as_str) == Some(section.name))
                .collect();
            json!({
                "name": section.name,
                "description": section.description,
                "optional": section.optional,
                "fields": group
            })
        })
        .collect()
}

async fn ensure_relations(
    state: &AppState,
    registry: &AdminRegistry,
    user: &AuthUser,
    columns: &[ColumnSpec],
    object: &Map<String, Value>,
) -> Result<(), String> {
    for column in columns {
        let Some(table) = column.foreign_table else {
            continue;
        };
        let Some(value) = object.get(column.name) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let Some(text) = value.as_str() else {
            return Err(column.name.to_owned());
        };
        if text.is_empty() {
            continue;
        }
        let Ok(id) = Uuid::parse_str(text) else {
            return Err(column.name.to_owned());
        };
        let Some(target) = registry.model_for_table(table) else {
            if registry
                .models()
                .iter()
                .filter(|candidate| candidate.descriptor().table == table)
                .count()
                > 1
            {
                return Err(column.name.to_owned());
            }
            continue;
        };
        if read_visible(state, user, target.as_ref(), id, "read")
            .await
            .is_none()
        {
            return Err(column.name.to_owned());
        }
    }
    Ok(())
}

fn payload(
    meta: &ferrioxid_core::AdminModelMeta,
    columns: &[ColumnSpec],
    form: &HashMap<String, String>,
) -> (Map<String, Value>, Map<String, Value>) {
    let mut object = Map::new();
    let mut raw = Map::new();
    for (key, value) in form {
        if key == "csrf_token"
            || key == "next"
            || key == "save"
            || key == "pick_field"
            || key == "_action"
            || key == "_lock"
            || key == "_use_draft"
            || key == "_picks"
        {
            continue;
        }
        if key == "_draft" {
            raw.insert(key.clone(), json!(value));
            continue;
        }
        if key.starts_with("inline_") || key.starts_with("extra_") || key == "basis" {
            raw.insert(key.clone(), json!(value));
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
    meta: &ferrioxid_core::AdminModelMeta,
    columns: &[ColumnSpec],
    record: &Value,
    relations: &HashMap<String, RelationLabels>,
    superuser: bool,
) -> Vec<Value> {
    meta.fields
        .iter()
        .filter(|field| !field.hidden && !sensitive(field.name))
        .map(|field| {
            let mut lead = false;
            let cell = present_cell(
                locale,
                currency,
                columns,
                field.name,
                record.get(field.name).unwrap_or(&Value::Null),
                relations,
                superuser,
                &mut lead,
            );
            json!({
            "label": field.label,
            "readonly": field.readonly || field.name == "id",
                    "value": cell["text"].as_str().unwrap_or(""),
                    "relation": cell["relation"].as_bool().unwrap_or(false),
                    "href": cell["href"].as_str().unwrap_or(""),
                    "ref": cell["ref"].as_str().unwrap_or(""),
                    "short": cell["short"].as_str().unwrap_or("")
                })
        })
        .collect()
}

fn resolved_sort(
    meta: &ferrioxid_core::AdminModelMeta,
    params: &HashMap<String, String>,
) -> Result<(String, bool), ()> {
    if let Some(sort) = params.get("sort").filter(|value| !value.is_empty()) {
        if meta.fields.iter().any(|field| {
            field.name == sort && !field.hidden && (field.sortable || field.name == "id")
        }) {
            Ok((
                sort.clone(),
                params.get("dir").map(String::as_str) == Some("desc"),
            ))
        } else {
            Err(())
        }
    } else if let Some(ordering) = meta.ordering {
        Ok((
            ordering.trim_start_matches('-').to_owned(),
            ordering.starts_with('-'),
        ))
    } else {
        Ok((
            "id".to_owned(),
            params.get("dir").map(String::as_str) == Some("desc"),
        ))
    }
}

fn link_names(meta: &ferrioxid_core::AdminModelMeta, specs: &[ColumnSpec]) -> Vec<&'static str> {
    let listed = listed_fields(meta, specs);
    let chosen: Vec<&'static str> = listed
        .iter()
        .filter(|field| field.list_link)
        .map(|field| field.name)
        .collect();
    if !chosen.is_empty() {
        return chosen;
    }
    listed
        .iter()
        .find(|field| field.name != "id")
        .or_else(|| listed.first())
        .map(|field| vec![field.name])
        .unwrap_or_default()
}

fn list_columns(
    meta: &ferrioxid_core::AdminModelMeta,
    specs: &[ColumnSpec],
    model: &str,
    params: &HashMap<String, String>,
    sort: &str,
    descending: bool,
) -> Vec<Value> {
    let dir = if descending { "desc" } else { "asc" };
    listed_fields(meta, specs)
        .into_iter()
        .map(|field| {
            let next_dir = if sort == field.name && dir != "desc" {
                "desc"
            } else {
                "asc"
            };
            let sortable = field.sortable || field.name == "id";
            let active = sortable && sort == field.name;
            json!({
                "name": field.name,
                "label": field.label,
                "readonly": field.readonly && field.name != "id",
                "sortable": sortable,
                "sorted": active,
                "sort_state": if active { if dir == "desc" { "descending" } else { "ascending" } } else { "" },
                "mark": if active { if dir == "desc" { "↓" } else { "↑" } } else { "" },
                "sort_href": format!("/admin/models/{model}?{}", sort_query(params, field.name, next_dir))
            })
        })
        .collect()
}

/// Spalten der Liste.
///
/// 1. Felder mit `#[admin(list_display)]` in Struct-Reihenfolge. `id` bleibt dabei, weil das Makro es immer als Listenspalte führt.
/// 2. Ohne solche Felder das erste sichtbare Textfeld, danach `id`.
/// 3. Ohne Textfeld das erste andere sichtbare Feld, danach `id`.
/// 4. Sonst nur `id`.
///
/// `hidden` und sensible Namen fallen in jedem Schritt weg. Ein Feld heißt nicht zwingend `name`.
fn listed_fields<'a>(
    meta: &'a ferrioxid_core::AdminModelMeta,
    specs: &[ColumnSpec],
) -> Vec<&'a ferrioxid_core::AdminFieldMeta> {
    let visible = |field: &ferrioxid_core::AdminFieldMeta| !field.hidden && !sensitive(field.name);
    let explicit: Vec<_> = meta
        .fields
        .iter()
        .filter(|field| field.list_display && field.name != "id" && visible(field))
        .collect();
    if !explicit.is_empty() {
        return meta
            .fields
            .iter()
            .filter(|field| field.list_display && visible(field))
            .collect();
    }
    let mut chosen = Vec::new();
    let text = meta.fields.iter().find(|field| {
        field.name != "id"
            && visible(field)
            && column_kind(specs, field.name) == Some(ColumnKind::Text)
    });
    let other = meta.fields.iter().find(|field| {
        field.name != "id" && visible(field) && column_kind(specs, field.name).is_some()
    });
    if let Some(field) = text.or(other) {
        chosen.push(field);
    }
    if let Some(id) = meta
        .fields
        .iter()
        .find(|field| field.name == "id" && visible(field))
    {
        chosen.push(id);
    }
    chosen
}

fn column_kind(specs: &[ColumnSpec], name: &str) -> Option<ColumnKind> {
    specs
        .iter()
        .find(|column| column.name == name)
        .map(|column| column.kind)
}

async fn filter_inputs(
    locale: &str,
    registry: &AdminRegistry,
    state: &AppState,
    user: &AuthUser,
    meta: &ferrioxid_core::AdminModelMeta,
    columns: &[ColumnSpec],
    params: &HashMap<String, String>,
) -> Vec<Value> {
    let mut filters = Vec::new();
    for field in meta
        .fields
        .iter()
        .filter(|field| field.filterable && !field.hidden)
    {
        let column = columns.iter().find(|column| column.name == field.name);
        let raw = params
            .get(&format!("filter.{}", field.name))
            .cloned()
            .unwrap_or_default();
        let (kind, options) = match column {
            Some(column) if column.foreign_table.is_some() => (
                "uuid",
                relation_options(registry, state, user, column, &raw).await,
            ),
            Some(column) if column.kind == ColumnKind::Boolean => ("boolean", json!([])),
            Some(column) if column.kind == ColumnKind::BigInt => ("number", json!([])),
            Some(column) if column.kind == ColumnKind::Date => ("date", json!([])),
            Some(column) if column.kind == ColumnKind::Time => ("time", json!([])),
            Some(column) if column.kind == ColumnKind::DateTime => ("datetime", json!([])),
            _ => ("text", json!([])),
        };
        let value = if kind == "boolean" {
            match raw.as_str() {
                "true" | "1" => "true".to_owned(),
                "false" | "0" => "false".to_owned(),
                _ => String::new(),
            }
        } else {
            raw
        };
        filters.push(json!({
                "name": field.name,
                "label": field.label,
            "kind": kind,
            "value": value,
            "options": options,
            "yes": catalog(locale)["yes"].as_str().unwrap_or("Yes"),
            "no": catalog(locale)["no"].as_str().unwrap_or("No")
        }));
    }
    for filter in registry
        .filters()
        .iter()
        .filter(|filter| filter.model_key == meta.key)
    {
        let raw = params
            .get(&format!("filter.{}", filter.name))
            .cloned()
            .unwrap_or_default();
        let (kind, options) = match &filter.kind {
            crate::filters::FilterKind::Choice { options, .. } => (
                "choice",
                options
                    .iter()
                    .map(|option| {
                        json!({
                            "id": option.value,
                            "label": if locale == "de" { option.label_de } else { option.label_en }
                        })
                    })
                    .collect::<Vec<_>>(),
            ),
            crate::filters::FilterKind::Equals { .. } => ("text", Vec::new()),
        };
        filters.push(json!({
            "name": filter.name,
            "label": filter.label(locale),
            "kind": kind,
            "value": raw,
            "options": options,
            "yes": catalog(locale)["yes"].as_str().unwrap_or("Yes"),
            "no": catalog(locale)["no"].as_str().unwrap_or("No")
        }));
    }
    filters
}

#[allow(clippy::too_many_arguments)]
fn present_cell(
    locale: &str,
    currency: &str,
    columns: &[ColumnSpec],
    name: &str,
    value: &Value,
    relations: &HashMap<String, RelationLabels>,
    superuser: bool,
    lead_taken: &mut bool,
) -> Value {
    let mut text = display_cell(locale, currency, name, value);
    if let Some(column) = columns
        .iter()
        .find(|column| column.name == name && column.kind == ColumnKind::Choice)
    {
        if !text.is_empty() && text != "—" {
            text = ferrioxid_core::choice_label(column.choices, locale, &text);
        }
    }
    let foreign = columns
        .iter()
        .find(|column| column.name == name)
        .and_then(|column| column.foreign_table);
    if let Some(table) = foreign {
        if text.is_empty() || text == "—" {
            return json!({ "text": "—", "short": "", "uuid": false, "lead": false, "relation": false, "ref": "", "href": "" });
        }
        let entry = relations.get(table);
        let resolved = entry.and_then(|entry| entry.labels.get(&text));
        let shown = if let Some(label) = resolved {
            label.clone()
        } else if superuser {
            catalog(locale)["missing_relation"]
                .as_str()
                .unwrap_or("Missing")
                .to_owned()
        } else {
            catalog(locale)["hidden_relation"]
                .as_str()
                .unwrap_or("Not visible")
                .to_owned()
        };
        let href = match (entry, resolved) {
            (Some(entry), Some(_)) if !entry.model_key.is_empty() => {
                format!("/admin/models/{}/{text}", entry.model_key)
            }
            _ => String::new(),
        };
        let lead = !*lead_taken;
        *lead_taken = true;
        return json!({
            "text": shown,
            "short": shorten_uuid(&text),
            "uuid": false,
            "lead": lead,
            "relation": true,
            "ref": text,
            "href": href
        });
    }
    let uuid = is_uuid(&text);
    let lead = !uuid && !*lead_taken;
    if lead {
        *lead_taken = true;
    }
    json!({
        "text": text,
        "short": if uuid { shorten_uuid(&text) } else { text.clone() },
        "uuid": uuid,
        "lead": lead,
        "relation": false,
        "ref": "",
        "href": ""
    })
}

struct RelationLabels {
    model_key: &'static str,
    labels: HashMap<String, String>,
}

async fn relation_names(
    registry: &AdminRegistry,
    state: &AppState,
    user: &AuthUser,
    columns: &[ColumnSpec],
    rows: &[Value],
) -> Result<HashMap<String, RelationLabels>, FerrioxidError> {
    let mut grouped: HashMap<&str, Vec<Uuid>> = HashMap::new();
    for column in columns
        .iter()
        .filter(|column| column.foreign_table.is_some())
    {
        let Some(table) = column.foreign_table else {
            continue;
        };
        for row in rows {
            let Some(text) = row.get(column.name).and_then(Value::as_str) else {
                continue;
            };
            if let Ok(id) = Uuid::parse_str(text) {
                grouped.entry(table).or_default().push(id);
            }
        }
    }
    let mut resolved = HashMap::new();
    for (table, ids) in grouped {
        let Some(target) = registry.model_for_table(table) else {
            resolved.insert(
                table.to_owned(),
                RelationLabels {
                    model_key: "",
                    labels: HashMap::new(),
                },
            );
            continue;
        };
        if list_blocked(target.as_ref())
            || permit(state.auth.as_ref().unwrap(), user, target.meta(), "read")
                .await
                .is_err()
        {
            resolved.insert(
                table.to_owned(),
                RelationLabels {
                    model_key: "",
                    labels: HashMap::new(),
                },
            );
            continue;
        }
        let Some(label) = display_column(target.meta(), target.columns()) else {
            resolved.insert(
                table.to_owned(),
                RelationLabels {
                    model_key: "",
                    labels: HashMap::new(),
                },
            );
            continue;
        };
        let scope = access_scope(target.as_ref(), user, false);
        let pairs = query_labels(
            &state.db,
            target.descriptor().table,
            target.columns(),
            label,
            &ids,
            &scope,
        )
        .await?;
        resolved.insert(
            table.to_owned(),
            RelationLabels {
                model_key: target.meta().key,
                labels: pairs.into_iter().collect(),
            },
        );
    }
    Ok(resolved)
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

fn confirmed(body: &Bytes) -> Result<(), &'static str> {
    let values = FormValues::parse_urlencoded(&String::from_utf8_lossy(body))
        .map_err(|_| "confirm_invalid")?;
    match require_confirmation(&values) {
        Ok(()) => Ok(()),
        Err(errors) if errors.for_field("confirm") == Some("validation.invalid") => {
            Err("confirm_invalid")
        }
        Err(_) => Err("confirm_required"),
    }
}

async fn role_names(auth: &ferrioxid_auth::AuthService) -> Vec<String> {
    let mut names: Vec<String> = auth
        .list_roles()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    names.sort();
    names
}

async fn user_new(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
) -> Response {
    user_form(&state, &headers, &registry, None, None, "", "").await
}

async fn user_create(
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
    let actor = match require_directory_post(&state, &headers, &registry, &body).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(code) = confirmed(&body) {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            true,
            &actor.email,
        )
        .await;
    }
    let values = match FormValues::parse_urlencoded(&String::from_utf8_lossy(&body)) {
        Ok(values) => values,
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "confirm_invalid",
                true,
                &actor.email,
            )
            .await
        }
    };
    if reject_unknown(
        &values,
        &[
            "confirm",
            "email",
            "password",
            "password_confirm",
            "active",
            "staff",
            "roles",
        ],
    )
    .is_err()
    {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &actor.email,
        )
        .await;
    }
    let email = required_text(
        &values,
        TextSpec {
            name: "email",
            required: true,
            min_length: None,
            max_length: Some(254),
            email: true,
            url: false,
        },
    );
    let password = values.get("password").unwrap_or("");
    let password_confirm = values.get("password_confirm").unwrap_or("");
    let active = required_bool(&values, "active");
    let staff = required_bool(&values, "staff");
    let roles = repeated(&body, "roles");
    if email.is_err() || active.is_err() || staff.is_err() || password != password_confirm {
        let copy = catalog(&locale);
        let message = if password != password_confirm {
            copy["password_mismatch"]
                .as_str()
                .unwrap_or("The passwords do not match.")
        } else if email.is_err() {
            copy["invalid_credentials"]
                .as_str()
                .unwrap_or("Invalid email or password.")
        } else {
            copy["confirm_invalid"]
                .as_str()
                .unwrap_or("The confirmation is invalid.")
        };
        let mut draft = user_draft(&values, Some(roles));
        if email.is_err() {
            draft.email_error = message.to_owned();
        }
        if password != password_confirm {
            draft.password_error = message.to_owned();
        }
        return user_form(&state, &headers, &registry, None, Some(draft), message, "").await;
    }
    let auth = state.auth.as_ref().unwrap();
    let known = role_names(auth).await;
    if roles
        .iter()
        .any(|role| !known.iter().any(|known| known == role))
    {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "unknown_role",
            true,
            &actor.email,
        )
        .await;
    }
    let created = crate::directory::create_user(
        auth,
        actor.id,
        email.as_ref().unwrap(),
        password,
        active.unwrap(),
        staff.unwrap(),
        &roles,
    )
    .await;
    let created = match created {
        Ok(user) => user,
        Err(ferrioxid_core::FerrioxidError::Validation(message))
            if message.contains("password") =>
        {
            let text = catalog(&locale)["password_invalid"]
                .as_str()
                .unwrap_or("The password must contain 12 to 128 characters.")
                .to_owned();
            let mut draft = user_draft(&values, Some(roles.clone()));
            draft.password_error = text.clone();
            return user_form(&state, &headers, &registry, None, Some(draft), &text, "").await;
        }
        Err(ferrioxid_core::FerrioxidError::Validation(_)) => {
            let text = catalog(&locale)["email_taken"]
                .as_str()
                .unwrap_or("That email is already registered.")
                .to_owned();
            let mut draft = user_draft(&values, Some(roles.clone()));
            draft.email_error = text.clone();
            return user_form(&state, &headers, &registry, None, Some(draft), &text, "").await;
        }
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &actor.email,
            )
            .await
        }
    };
    redirect(&format!(
        "/admin/directory/users/{}?notice=created",
        created.id
    ))
}

async fn user_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let actor = match require_directory(&state, &headers, &registry, "/admin/directory/users").await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let auth = state.auth.as_ref().unwrap();
    let Some(target) = auth.get_user(id).await.ok().flatten() else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &actor.email,
        )
        .await;
    };
    if !target.is_superuser {
        return user_form(
            &state,
            &headers,
            &registry,
            Some(id),
            None,
            "",
            &flash(
                &locale,
                params.get("notice").map(String::as_str).unwrap_or(""),
            ),
        )
        .await;
    }
    let roles = auth
        .list_user_role_names()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(user_id, name)| (user_id == id).then_some(name))
        .collect::<Vec<_>>();
    let permissions = auth.list_user_permissions(id).await.unwrap_or_default();
    let csrf = session_csrf(auth, &headers).await;
    page_as(
        &state,
        &actor,
        &registry,
        "user_detail.html",
        &locale,
        theme_of(&headers),
        &target.email,
        true,
        true,
        &actor.email,
        &csrf,
        "/admin/directory/users",
        &flash(
            &locale,
            params.get("notice").map(String::as_str).unwrap_or(""),
        ),
        "",
        json!({
            "id": id,
            "email": target.email,
            "is_active": target.is_active,
            "is_staff": target.is_staff,
            "is_superuser": target.is_superuser,
            "roles": roles,
            "permissions": permissions,
            "can_edit": !target.is_superuser,
            "history_href": history_href("_ferrioxid_users", &id.to_string()),
            "history": record_history(&state, &actor, "_ferrioxid_users", &id.to_string()).await
        }),
    )
    .await
}

async fn user_edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(id): Path<Uuid>,
) -> Response {
    user_form(&state, &headers, &registry, Some(id), None, "", "").await
}

async fn user_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(id): Path<Uuid>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let actor = match require_directory_post(&state, &headers, &registry, &body).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let auth = state.auth.as_ref().unwrap();
    let Some(target) = auth.get_user(id).await.ok().flatten() else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &actor.email,
        )
        .await;
    };
    if target.is_superuser {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &actor.email,
        )
        .await;
    }
    if let Err(code) = confirmed(&body) {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            true,
            &actor.email,
        )
        .await;
    }
    let values = match FormValues::parse_urlencoded(&String::from_utf8_lossy(&body)) {
        Ok(values) => values,
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "confirm_invalid",
                true,
                &actor.email,
            )
            .await
        }
    };
    if reject_unknown(&values, &["confirm", "email", "active", "staff", "save"]).is_err() {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &actor.email,
        )
        .await;
    }
    let email = required_text(
        &values,
        TextSpec {
            name: "email",
            required: true,
            min_length: None,
            max_length: Some(254),
            email: true,
            url: false,
        },
    );
    let active = required_bool(&values, "active");
    let staff = required_bool(&values, "staff");
    let email_invalid = email.is_err();
    let (Ok(email), Ok(active), Ok(staff)) = (email, active, staff) else {
        let message = if email_invalid {
            catalog(&locale)["invalid_credentials"]
                .as_str()
                .unwrap_or("Invalid email or password.")
                .to_owned()
        } else {
            catalog(&locale)["confirm_invalid"]
                .as_str()
                .unwrap_or("The confirmation is invalid.")
                .to_owned()
        };
        let mut draft = user_draft(&values, None);
        if email_invalid {
            draft.email_error = message.clone();
        }
        return user_form(
            &state,
            &headers,
            &registry,
            Some(id),
            Some(draft),
            &message,
            "",
        )
        .await;
    };
    if actor.id == id && (!active || !staff && target.is_staff) {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &actor.email,
        )
        .await;
    }
    match crate::directory::update_user(auth, actor.id, id, &email, active, staff).await {
        Ok(_) => redirect(&if values.get("save") == Some("list") {
            "/admin/directory/users?notice=updated".to_owned()
        } else {
            format!("/admin/directory/users/{id}?notice=updated")
        }),
        Err(ferrioxid_core::FerrioxidError::Forbidden) => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::FORBIDDEN,
                "forbidden",
                true,
                &actor.email,
            )
            .await
        }
        Err(ferrioxid_core::FerrioxidError::Validation(message)) if message.contains("email") => {
            let text = catalog(&locale)["email_taken"]
                .as_str()
                .unwrap_or("That email is already registered.")
                .to_owned();
            let mut draft = user_draft(&values, None);
            draft.email_error = text.clone();
            user_form(
                &state,
                &headers,
                &registry,
                Some(id),
                Some(draft),
                &text,
                "",
            )
            .await
        }
        Err(_) => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &actor.email,
            )
            .await
        }
    }
}

async fn user_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(id): Path<Uuid>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let actor = match require_directory_post(&state, &headers, &registry, &body).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let auth = state.auth.as_ref().unwrap();
    let Some(target) = auth.get_user(id).await.ok().flatten() else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &actor.email,
        )
        .await;
    };
    if target.is_superuser {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &actor.email,
        )
        .await;
    }
    if let Err(code) = confirmed(&body) {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            true,
            &actor.email,
        )
        .await;
    }
    let values = FormValues::parse_urlencoded(&String::from_utf8_lossy(&body)).unwrap_or_default();
    if reject_unknown(&values, &["confirm", "password", "password_confirm"]).is_err() {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &actor.email,
        )
        .await;
    }
    let password = values.get("password").unwrap_or("");
    let password_confirm = values.get("password_confirm").unwrap_or("");
    if password != password_confirm {
        let text = catalog(&locale)["password_mismatch"]
            .as_str()
            .unwrap_or("The passwords do not match.")
            .to_owned();
        return user_form(
            &state,
            &headers,
            &registry,
            Some(id),
            Some(UserDraft {
                email: None,
                active: None,
                staff: None,
                roles: None,
                email_error: String::new(),
                password_error: text.clone(),
            }),
            &text,
            "",
        )
        .await;
    }
    match crate::directory::reset_password(auth, actor.id, id, password).await {
        Ok(()) => {
            if actor.id == id {
                redirect("/admin/login")
            } else {
                redirect(&format!("/admin/directory/users/{id}?notice=updated"))
            }
        }
        Err(ferrioxid_core::FerrioxidError::Forbidden) => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::FORBIDDEN,
                "forbidden",
                true,
                &actor.email,
            )
            .await
        }
        Err(ferrioxid_core::FerrioxidError::Validation(_)) => {
            let text = catalog(&locale)["password_invalid"]
                .as_str()
                .unwrap_or("The password must contain 12 to 128 characters.")
                .to_owned();
            user_form(
                &state,
                &headers,
                &registry,
                Some(id),
                Some(UserDraft {
                    email: None,
                    active: None,
                    staff: None,
                    roles: None,
                    email_error: String::new(),
                    password_error: text.clone(),
                }),
                &text,
                "",
            )
            .await
        }
        Err(_) => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &actor.email,
            )
            .await
        }
    }
}

struct UserDraft {
    email: Option<String>,
    active: Option<bool>,
    staff: Option<bool>,
    roles: Option<Vec<String>>,
    email_error: String,
    password_error: String,
}

fn user_draft(values: &FormValues, roles: Option<Vec<String>>) -> UserDraft {
    let flag = |name: &str| match values.get(name) {
        Some("true") | Some("1") | Some("on") => Some(true),
        Some("false") | Some("0") => Some(false),
        _ => None,
    };
    UserDraft {
        email: Some(values.get("email").unwrap_or("").to_owned()),
        active: flag("active"),
        staff: flag("staff"),
        roles,
        email_error: String::new(),
        password_error: String::new(),
    }
}

async fn user_form(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    id: Option<Uuid>,
    draft: Option<UserDraft>,
    alert: &str,
    notice: &str,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let actor = match require_directory(state, headers, registry, "/admin/directory/users").await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let auth = state.auth.as_ref().unwrap();
    let target = if let Some(id) = id {
        match auth.get_user(id).await.ok().flatten() {
            Some(user) if !user.is_superuser => Some(user),
            Some(_) => {
                return error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::FORBIDDEN,
                    "forbidden",
                    true,
                    &actor.email,
                )
                .await
            }
            None => {
                return error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::NOT_FOUND,
                    "not_found",
                    true,
                    &actor.email,
                )
                .await
            }
        }
    } else {
        None
    };
    let assigned = if let Some(id) = id {
        auth.list_user_role_names()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(user_id, name)| (user_id == id).then_some(name))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let email = draft
        .as_ref()
        .and_then(|item| item.email.clone())
        .or_else(|| target.as_ref().map(|user| user.email.clone()))
        .unwrap_or_default();
    let is_active = draft
        .as_ref()
        .and_then(|item| item.active)
        .or_else(|| target.as_ref().map(|user| user.is_active))
        .unwrap_or(true);
    let is_staff = draft
        .as_ref()
        .and_then(|item| item.staff)
        .or_else(|| target.as_ref().map(|user| user.is_staff))
        .unwrap_or(false);
    let known_roles = role_names(auth).await;
    let selected = draft
        .as_ref()
        .and_then(|item| item.roles.clone())
        .unwrap_or_default();
    let role_checks: Vec<Value> = known_roles
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "checked": selected.iter().any(|role| role == name)
            })
        })
        .collect();
    let email_error = draft
        .as_ref()
        .map(|item| item.email_error.clone())
        .unwrap_or_default();
    let password_error = draft
        .as_ref()
        .map(|item| item.password_error.clone())
        .unwrap_or_default();
    let csrf = session_csrf(auth, headers).await;
    let history = if let Some(record_id) = id {
        record_history(state, &actor, "_ferrioxid_users", &record_id.to_string()).await
    } else {
        Vec::new()
    };
    let record_history_href = id
        .map(|record_id| history_href("_ferrioxid_users", &record_id.to_string()))
        .unwrap_or_default();
    let action = match id {
        Some(id) => format!("/admin/directory/users/{id}"),
        None => "/admin/directory/users/new".to_owned(),
    };
    let mut response = page_as(
        state,
        &actor,
        registry,
        "user_form.html",
        &locale,
        theme_of(headers),
        catalog(&locale)[if id.is_some() { "edit" } else { "add_user" }]
            .as_str()
            .unwrap_or("User"),
        true,
        true,
        &actor.email,
        &csrf,
        "/admin/directory/users",
        notice,
        alert,
        json!({
            "id": id.map(|value| value.to_string()).unwrap_or_default(),
            "creating": id.is_none(),
            "action": action,
            "email": email,
            "email_error": email_error,
            "password_error": password_error,
            "is_active": is_active,
            "is_staff": is_staff,
            "self_edit": id == Some(actor.id),
            "roles": known_roles,
            "role_checks": role_checks,
            "assigned": assigned,
            "permissions": if let Some(id) = id { auth.list_user_permissions(id).await.unwrap_or_default() } else { Vec::new() },
            "history_href": record_history_href,
            "history": history
        }),
    )
    .await;
    if !alert.is_empty() {
        *response.status_mut() = StatusCode::UNPROCESSABLE_ENTITY;
    }
    response
}

fn repeated(body: &Bytes, name: &str) -> Vec<String> {
    let mut values = Vec::new();
    for pair in String::from_utf8_lossy(body).split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if decode(key) == name {
            let value = decode(value);
            if !value.is_empty() {
                values.push(value);
            }
        }
    }
    values
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
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if target.is_superuser
        || (user.id == id
            && form_flag(&form, if active_flag { "active" } else { "staff" }) == Some(false))
    {
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
    if let Err(code) = confirmed(body) {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            true,
            &user.email,
        )
        .await;
    }
    let result = if active_flag {
        crate::directory::set_active(
            auth,
            user.id,
            id,
            form_flag(&form, "active").unwrap_or(false),
        )
        .await
    } else {
        crate::directory::set_staff(
            auth,
            user.id,
            id,
            form_flag(&form, "staff").unwrap_or(false),
        )
        .await
    };
    if result.is_err() {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    redirect("/admin/directory/users?notice=updated")
}

#[allow(clippy::result_large_err)]
pub(crate) async fn require_get(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
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
        Ok(user) => {
            let locale = locale_of(
                headers,
                &registry.branding().default_locale,
                &registry.branding().fallback_locale,
            );
            Err(error_page(
                state,
                headers,
                registry,
                &locale,
                StatusCode::FORBIDDEN,
                "staff_required",
                true,
                &user.email,
            )
            .await)
        }
        Err(AuthError::Unauthenticated) => Err(redirect(&format!(
            "/admin/login?next={}",
            url_encode(safe_next(next).unwrap_or("/admin"))
        ))),
        Err(_) => Err(plain(StatusCode::FORBIDDEN, "authentication failed")),
    }
}

pub(crate) async fn require_post(
    state: &AppState,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<AuthUser, ()> {
    let Some(auth) = state.auth.as_ref() else {
        return Err(());
    };
    let token = csrf_token_from(headers, body);
    let user = auth
        .authenticate_csrf(headers, &Method::POST, &token)
        .await
        .map_err(|_| ())?;
    user.is_staff.then_some(user).ok_or(())
}

#[allow(clippy::result_large_err)]
async fn require_directory(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    next: &str,
) -> Result<AuthUser, Response> {
    let user = require_get(state, headers, registry, next).await?;
    if user.is_superuser {
        Ok(user)
    } else {
        let locale = locale_of(
            headers,
            &registry.branding().default_locale,
            &registry.branding().fallback_locale,
        );
        Err(error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        )
        .await)
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
            state,
            headers,
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        )
        .await),
        Err(_) => Err(error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::FORBIDDEN,
            "csrf",
            false,
            "",
        )
        .await),
    }
}

#[allow(clippy::too_many_arguments)]
async fn model_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
    body: Bytes,
) -> Response {
    keep_draft(&state, &headers, &registry, &model_key, None, &body).await
}

async fn model_record_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
    body: Bytes,
) -> Response {
    keep_draft(&state, &headers, &registry, &model_key, Some(id), &body).await
}

async fn keep_draft(
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
        Err(_) => {
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
            .await
        }
    };
    let Some(model) = registry.find(model_key) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let action_name = if id.is_some() { "update" } else { "create" };
    if gate(state, &user, model.as_ref(), action_name)
        .await
        .is_err()
    {
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
    let form = parse_form(body);
    let Some(field) = form.get("pick_field").map(String::as_str) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    };
    let allowed = model.meta().fields.iter().any(|item| {
        item.name == field
            && !item.hidden
            && !item.readonly
            && !item.sensitive
            && !sensitive(item.name)
    }) && model
        .columns()
        .iter()
        .any(|column| column.name == field && column.foreign_table.is_some());
    if !allowed {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let values = draft_values(model.meta(), model.columns(), &form);
    let record = id.map(|value| value.to_string());
    let draft = drafts::insert(
        &state.db,
        user.session_id,
        user.id,
        model_key,
        record.as_deref(),
        "form",
        &values,
    )
    .await;
    let Ok(draft) = draft else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    };
    let target = match id {
        Some(id) => format!("/admin/models/{model_key}/{id}/pick/{field}?draft={draft}"),
        None => format!("/admin/models/{model_key}/pick/{field}?draft={draft}"),
    };
    redirect(&target)
}

async fn inline_new(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id, inline)): Path<(String, Uuid, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let user = match require_get(
        &state,
        &headers,
        &registry,
        &format!("/admin/models/{model_key}/{id}"),
    )
    .await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(parent) = registry.find(&model_key) else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale_of(
                &headers,
                &registry.branding().default_locale,
                &registry.branding().fallback_locale,
            ),
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(child) = child_inline(&registry, parent.as_ref(), &inline) else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale_of(
                &headers,
                &registry.branding().default_locale,
                &registry.branding().fallback_locale,
            ),
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if read_visible(&state, &user, parent.as_ref(), id, "read")
        .await
        .is_none()
        || gate(&state, &user, child.as_ref(), "create").await.is_err()
    {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale_of(
                &headers,
                &registry.branding().default_locale,
                &registry.branding().fallback_locale,
            ),
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let spec = parent
        .meta()
        .inlines
        .iter()
        .find(|item| item.name == inline)
        .expect("inline");
    let mut values = Map::new();
    values.insert(spec.column.to_owned(), json!(id.to_string()));
    values.insert("_lock".to_owned(), json!(spec.column));
    values.insert(
        "_action".to_owned(),
        json!(format!(
            "/admin/models/{model_key}/{id}/inline/{inline}/new"
        )),
    );
    values.insert(
        "_cancel".to_owned(),
        json!(format!("/admin/models/{model_key}/{id}")),
    );
    if query.contains_key("draft") {
        values.insert("_draft".to_owned(), json!(query.get("draft")));
        values.insert("_use_draft".to_owned(), json!(true));
    }
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    form_page(
        &state,
        &headers,
        &registry,
        child.meta().key,
        None,
        values,
        FormFeedback {
            notice: &flash(&locale, ""),
            alert: "",
        },
    )
    .await
}

async fn inline_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id, inline)): Path<(String, Uuid, String)>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_post(&state, &headers, &body).await {
        Ok(user) => user,
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::FORBIDDEN,
                "csrf",
                true,
                "",
            )
            .await
        }
    };
    let Some(parent) = registry.find(&model_key) else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(child) = child_inline(&registry, parent.as_ref(), &inline) else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(spec) = parent
        .meta()
        .inlines
        .iter()
        .find(|item| item.name == inline)
    else {
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    if read_visible(&state, &user, parent.as_ref(), id, "read")
        .await
        .is_none()
        || gate(&state, &user, child.as_ref(), "create").await.is_err()
    {
        let _ = audit::insert(
            &state.db,
            &Event {
                actor_id: user.id,
                model_key: child.meta().key.to_owned(),
                record_id: String::new(),
                action: "create".to_owned(),
                outcome: "denied".to_owned(),
                summary: String::new(),
                operation_id: None,
            },
        )
        .await;
        return error_page(
            &state,
            &headers,
            &registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    }
    let form = parse_form(&body);
    let (mut object, mut raw) = payload(child.meta(), child.columns(), &form);
    object.insert(spec.column.to_owned(), json!(id.to_string()));
    raw.insert(spec.column.to_owned(), json!(id.to_string()));
    raw.insert("_lock".to_owned(), json!(spec.column));
    raw.insert(
        "_action".to_owned(),
        json!(format!(
            "/admin/models/{model_key}/{id}/inline/{inline}/new"
        )),
    );
    raw.insert(
        "_cancel".to_owned(),
        json!(format!("/admin/models/{model_key}/{id}")),
    );
    if let Err(message) = reject_locked(child.meta(), &object, true) {
        let text = validation_text(&locale, &message);
        raw.insert(
            "__error".into(),
            json!({ "field": message.split_whitespace().next().unwrap_or(""), "text": text }),
        );
        return form_page(
            &state,
            &headers,
            &registry,
            child.meta().key,
            None,
            raw,
            FormFeedback {
                notice: "",
                alert: &text,
            },
        )
        .await;
    }
    if let Err(field) = ensure_relations(&state, &registry, &user, child.columns(), &object).await {
        let text = catalog(&locale)
            .get("invalid_relation")
            .and_then(Value::as_str)
            .unwrap_or("Choose a visible record.")
            .to_owned();
        raw.insert("__error".into(), json!({ "field": field, "text": text }));
        return form_page(
            &state,
            &headers,
            &registry,
            child.meta().key,
            None,
            raw,
            FormFeedback {
                notice: "",
                alert: &text,
            },
        )
        .await;
    }
    match child.create(&state.db, Value::Object(object), &user).await {
        Ok(_) => redirect(&format!("/admin/models/{model_key}/{id}?notice=created")),
        Err(FerrioxidError::Validation(message)) => {
            let text = validation_text(&locale, &message);
            let field = message.split_whitespace().next().unwrap_or("");
            raw.insert("__error".into(), json!({ "field": field, "text": text }));
            form_page(
                &state,
                &headers,
                &registry,
                child.meta().key,
                None,
                raw,
                FormFeedback {
                    notice: "",
                    alert: &text,
                },
            )
            .await
        }
        Err(FerrioxidError::Denied(_)) => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::FORBIDDEN,
                "forbidden",
                true,
                &user.email,
            )
            .await
        }
        Err(_) => {
            error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &user.email,
            )
            .await
        }
    }
}

async fn model_bulk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
    body: Bytes,
) -> Response {
    stage_bulk(&state, &headers, &registry, &model_key, &body).await
}

async fn bulk_confirm(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, draft)): Path<(String, Uuid)>,
) -> Response {
    show_bulk(&state, &headers, &registry, &model_key, draft).await
}

async fn bulk_apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, draft)): Path<(String, Uuid)>,
    body: Bytes,
) -> Response {
    let locale = locale_of(
        &headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_post(&state, &headers, &body).await {
        Ok(user) => user,
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::FORBIDDEN,
                "csrf",
                true,
                "",
            )
            .await
        }
    };
    if confirmed(&body).is_err() {
        return show_bulk(&state, &headers, &registry, &model_key, draft).await;
    }
    apply_bulk(
        &state, &headers, &registry, &user, &locale, &model_key, draft,
    )
    .await
}

async fn audit_page(
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
    let user = match require_get(&state, &headers, &registry, "/admin/audit").await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let page_number = params
        .get("page")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    let actor = (!user.is_superuser).then_some(user.id);
    let model_filter = params.get("model").and_then(|value| audit_key(value));
    let record_filter = params.get("record").and_then(|value| audit_key(value));
    let listed = if model_filter.is_none() && record_filter.is_none() {
        audit::page(
            &state.db,
            actor,
            u64::from(page_number - 1) * audit::PAGE_SIZE,
        )
        .await
    } else {
        audit::matching(
            &state.db,
            actor,
            model_filter.as_deref(),
            record_filter.as_deref(),
            u64::from(page_number - 1) * audit::PAGE_SIZE,
        )
        .await
    };
    let (events, has_more) = match listed {
        Ok(value) => value,
        Err(_) => {
            return error_page(
                &state,
                &headers,
                &registry,
                &locale,
                StatusCode::INTERNAL_SERVER_ERROR,
                "forbidden",
                true,
                &user.email,
            )
            .await
        }
    };
    let rows: Vec<Value> = events
        .into_iter()
        .map(|event| {
            let actor_label = if event.actor_id == user.id {
                user.email.clone()
            } else {
                event.actor_id.to_string()
            };
            let formatted = audit::format_unix_utc(event.created_at);
            let record_href = (registry.find(&event.model_key).is_some()
                && Uuid::parse_str(&event.record_id).is_ok())
            .then(|| format!("/admin/models/{}/{}", event.model_key, event.record_id));
            json!({
                "when": formatted.as_ref().map(|(label, _)| label.clone()).unwrap_or_default(),
                "when_iso": formatted.as_ref().map(|(_, iso)| iso.clone()).unwrap_or_default(),
                "actor": actor_label,
                "action": event.action,
                "record": event.record_id,
                "record_short": event.record_id.chars().take(8).collect::<String>(),
                "record_href": record_href,
                "ok": event.outcome == "ok",
                "operation": event
                    .operation_id
                    .map(|id| id.to_string().chars().take(8).collect::<String>())
                    .unwrap_or_default(),
                "summary": event.summary
            })
        })
        .collect();
    let csrf = session_csrf(state.auth.as_ref().unwrap(), &headers).await;
    page_as(
        &state,
        &user,
        &registry,
        "audit.html",
        &locale,
        theme_of(&headers),
        catalog(&locale)
            .get("history")
            .and_then(Value::as_str)
            .unwrap_or("History"),
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        "/admin/audit",
        "",
        "",
        json!({
            "rows": rows,
            "filtered": model_filter.is_some() || record_filter.is_some(),
            "filter_label": match (&model_filter, &record_filter) {
                (Some(model), Some(record)) => format!("{model} · {record}"),
                (Some(model), None) => model.clone(),
                (None, Some(record)) => record.clone(),
                (None, None) => String::new(),
            },
            "previous": (page_number > 1).then(|| audit_page_href(page_number - 1, model_filter.as_deref(), record_filter.as_deref())),
            "next_page": has_more.then(|| audit_page_href(page_number + 1, model_filter.as_deref(), record_filter.as_deref()))
        }),
    )
    .await
}

fn child_inline<'a>(
    registry: &'a AdminRegistry,
    parent: &dyn ManagedModel,
    name: &str,
) -> Option<&'a std::sync::Arc<dyn ManagedModel>> {
    let spec = parent
        .meta()
        .inlines
        .iter()
        .find(|item| item.name == name)?;
    let child = registry.find(spec.child_table)?;
    child.columns().iter().find(|column| {
        column.name == spec.column && crate::registry::references_parent(column, parent)
    })?;
    Some(child)
}

fn draft_values(
    meta: &ferrioxid_core::AdminModelMeta,
    columns: &[ColumnSpec],
    form: &HashMap<String, String>,
) -> Map<String, Value> {
    let mut values = Map::new();
    for field in meta.fields {
        if field.hidden
            || field.readonly
            || field.sensitive
            || sensitive(field.name)
            || field.name == "id"
        {
            continue;
        }
        let Some(value) = form.get(field.name) else {
            continue;
        };
        if value.chars().count() > 2_000 {
            continue;
        }
        if columns.iter().any(|column| column.name == field.name) {
            values.insert(field.name.to_owned(), json!(value));
        }
    }
    for (key, value) in form {
        if (key.starts_with("inline_") || key.starts_with("extra_") || key == "basis")
            && value.chars().count() <= 2_000
            && !key.contains("password")
        {
            values.insert(key.clone(), json!(value));
        }
    }
    values
}

async fn take_draft(
    state: &AppState,
    user: &AuthUser,
    model_key: &str,
    id: Option<Uuid>,
    values: &mut Map<String, Value>,
) -> String {
    let picks = match values.remove("_picks") {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    let use_draft = values
        .remove("_use_draft")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let token = values
        .get("_draft")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let mut shown = String::new();
    if use_draft {
        if let Some(token) = token {
            if let Ok(Some(draft)) = drafts::load(
                &state.db,
                token,
                user.session_id,
                user.id,
                model_key,
                "form",
            )
            .await
            {
                let record = draft.record_id.unwrap_or_default();
                let matches = match id {
                    Some(id) => record == id.to_string(),
                    None => record.is_empty(),
                };
                if matches {
                    for (key, value) in draft.values {
                        values.entry(key).or_insert(value);
                    }
                    shown = token.to_string();
                }
            }
        }
    } else if let Some(token) = token {
        shown = token.to_string();
    }
    for (key, value) in picks {
        values.insert(key, value);
    }
    if shown.is_empty() {
        values.remove("_draft");
    } else {
        values.insert("_draft".to_owned(), json!(shown));
    }
    shown
}

fn choice_href(back: &str, field: &str, id: &str, draft: Option<&str>) -> String {
    match draft.filter(|value| Uuid::parse_str(value).is_ok()) {
        Some(draft) => format!("{back}?draft={}&{field}={id}", url_encode(draft)),
        None => format!("{back}?{field}={id}"),
    }
}

#[allow(clippy::too_many_arguments)]
async fn inline_sections(
    state: &AppState,
    registry: &AdminRegistry,
    user: &AuthUser,
    parent: &dyn ManagedModel,
    parent_id: Uuid,
    query: &HashMap<String, String>,
    editing: bool,
    locale: &str,
) -> Vec<Value> {
    let mut sections = Vec::new();
    for spec in parent.meta().inlines {
        if editing && spec.form {
            continue;
        }
        let Some(child) = child_inline(registry, parent, spec.name) else {
            continue;
        };
        if list_blocked(child.as_ref())
            || permit(state.auth.as_ref().unwrap(), user, child.meta(), "read")
                .await
                .is_err()
        {
            continue;
        }
        let page_number = query
            .get(&format!("inline.{}", spec.name))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1)
            .clamp(1, 10_000);
        let limit = u64::from(child.meta().per_page.clamp(1, 25));
        let scope = access_scope(child.as_ref(), user, false);
        let filters = [(spec.column, DbValue::from(parent_id))];
        let Ok((rows, has_more, _)) = child
            .page(
                &state.db,
                &[],
                None,
                "id",
                false,
                &filters,
                &[],
                u64::from(page_number - 1) * limit,
                limit,
                &scope,
            )
            .await
        else {
            continue;
        };
        let label_field = child
            .meta()
            .fields
            .iter()
            .find(|field| {
                field.list_display
                    && !field.hidden
                    && !field.sensitive
                    && field.name != spec.column
                    && field.name != "id"
            })
            .map(|field| field.name)
            .unwrap_or("id");
        let foreign_label = child
            .columns()
            .iter()
            .any(|column| column.name == label_field && column.foreign_table.is_some());
        let relations = if foreign_label {
            let label_columns: Vec<ColumnSpec> = child
                .columns()
                .iter()
                .copied()
                .filter(|column| column.name == label_field)
                .collect();
            match relation_names(registry, state, user, &label_columns, &rows).await {
                Ok(relations) => relations,
                Err(_) => continue,
            }
        } else {
            HashMap::new()
        };
        let rendered = rows
            .iter()
            .filter_map(|row| {
                let id = row.get("id")?.as_str()?;
                let (text, relation_href) = if foreign_label {
                    let mut lead = false;
                    let cell = present_cell(
                        locale,
                        registry.branding().currency.as_str(),
                        child.columns(),
                        label_field,
                        row.get(label_field).unwrap_or(&Value::Null),
                        &relations,
                        user.is_superuser,
                        &mut lead,
                    );
                    (
                        cell["text"].as_str().unwrap_or(id).to_owned(),
                        cell["href"].as_str().unwrap_or("").to_owned(),
                    )
                } else {
                    let text = row
                        .get(label_field)
                        .map(|value| match value {
                            Value::String(text) => text.clone(),
                            Value::Number(number) => number.to_string(),
                            Value::Bool(flag) => flag.to_string(),
                            _ => id.to_owned(),
                        })
                        .unwrap_or_else(|| id.to_owned());
                    (text, String::new())
                };
                Some(json!({
                    "text": text,
                    "href": format!("/admin/models/{}/{}", child.meta().key, id),
                    "relation_href": relation_href
                }))
            })
            .collect::<Vec<_>>();
        let can_create = permit(state.auth.as_ref().unwrap(), user, child.meta(), "create")
            .await
            .is_ok();
        sections.push(json!({
            "label": child.meta().label,
            "rows": rendered,
            "add": (can_create && !spec.form).then(|| format!("/admin/models/{}/{parent_id}/inline/{}/new", parent.meta().key, spec.name)),
            "next": has_more.then(|| format!("/admin/models/{}/{parent_id}?inline.{}={}", parent.meta().key, spec.name, page_number + 1))
        }));
    }
    sections
}

async fn stage_bulk(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    model_key: &str,
    body: &Bytes,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_post(state, headers, body).await {
        Ok(user) => user,
        Err(_) => {
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
            .await
        }
    };
    let Some(model) = registry.find(model_key) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let form = parse_form(body);
    let Some(name) = form.get("action").map(String::as_str) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    };
    let Some(action) = registry
        .actions_for(model.meta(), model.columns())
        .into_iter()
        .find(|action| action.name == name)
    else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    };
    if permit(
        state.auth.as_ref().unwrap(),
        &user,
        model.meta(),
        action.permission,
    )
    .await
    .is_err()
    {
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
    let ids = if form.get("scope").map(String::as_str) == Some("page") {
        let (sort, descending) = match resolved_sort(model.meta(), &form) {
            Ok(value) => value,
            Err(()) => {
                return error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "forbidden",
                    true,
                    &user.email,
                )
                .await
            }
        };
        let filters = match filters_from(
            &String::from_utf8_lossy(body),
            model.columns(),
            model.meta(),
            registry.filters(),
        ) {
            Ok(filters) => filters,
            Err(_) => {
                return error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "forbidden",
                    true,
                    &user.email,
                )
                .await
            }
        };
        let page_number = form
            .get("page")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1)
            .max(1);
        let page_size = u64::from(model.meta().per_page.clamp(1, 100));
        let search: Vec<&str> = model
            .meta()
            .fields
            .iter()
            .filter(|field| field.searchable && !field.hidden && !sensitive(field.name))
            .map(|field| field.name)
            .collect();
        let query = form.get("q").map(String::as_str).unwrap_or("");
        let listed = model
            .page(
                &state.db,
                &search,
                (!query.is_empty()).then_some(query),
                &sort,
                descending,
                &filters,
                &[],
                u64::from(page_number - 1) * page_size,
                page_size,
                &access_scope(model.as_ref(), &user, true),
            )
            .await;
        match listed {
            Ok((rows, _, _)) => rows
                .iter()
                .filter_map(|row| {
                    row.get("id")
                        .and_then(Value::as_str)
                        .and_then(|value| Uuid::parse_str(value).ok())
                })
                .collect::<Vec<_>>(),
            Err(_) => {
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
                .await
            }
        }
    } else {
        let mut ids = Vec::new();
        for value in repeated(body, "ids") {
            let Ok(id) = Uuid::parse_str(&value) else {
                return error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "forbidden",
                    true,
                    &user.email,
                )
                .await;
            };
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    };
    if ids.is_empty() {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "bulk_empty",
            true,
            &user.email,
        )
        .await;
    }
    if ids.len() > action.max_ids {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    let verb = if action.permission == "delete" {
        "delete"
    } else {
        "update"
    };
    let scope = access_scope(model.as_ref(), &user, true);
    for id in &ids {
        match model.read(&state.db, *id, &scope).await {
            Ok(Some(row)) if model.object_allows(&user, verb, &row) => {}
            Ok(_) => {
                return error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::NOT_FOUND,
                    "bulk_denied",
                    true,
                    &user.email,
                )
                .await
            }
            Err(_) => {
                return error_page(
                    state,
                    headers,
                    registry,
                    &locale,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "bulk_failed",
                    true,
                    &user.email,
                )
                .await
            }
        }
    }
    let mut payload = Map::new();
    payload.insert("action".to_owned(), json!(action.name));
    payload.insert(
        "ids".to_owned(),
        json!(ids.iter().map(ToString::to_string).collect::<Vec<_>>()),
    );
    let draft = drafts::insert(
        &state.db,
        user.session_id,
        user.id,
        model_key,
        None,
        "bulk",
        &payload,
    )
    .await;
    let Ok(draft) = draft else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    };
    redirect(&format!("/admin/models/{model_key}/bulk/{draft}"))
}

async fn show_bulk(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    model_key: &str,
    draft: Uuid,
) -> Response {
    let locale = locale_of(
        headers,
        &registry.branding().default_locale,
        &registry.branding().fallback_locale,
    );
    let user = match require_get(
        state,
        headers,
        registry,
        &format!("/admin/models/{model_key}"),
    )
    .await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(model_key) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(stored) = drafts::load(
        &state.db,
        draft,
        user.session_id,
        user.id,
        model_key,
        "bulk",
    )
    .await
    .ok()
    .flatten() else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(name) = stored.values.get("action").and_then(Value::as_str) else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(action) = registry
        .actions_for(model.meta(), model.columns())
        .into_iter()
        .find(|item| item.name == name)
    else {
        return error_page(
            state,
            headers,
            registry,
            &locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let count = stored
        .values
        .get("ids")
        .and_then(Value::as_array)
        .map(|ids| ids.len())
        .unwrap_or(0);
    let csrf = session_csrf(state.auth.as_ref().unwrap(), headers).await;
    page_as(
        state,
        &user,
        registry,
        "bulk.html",
        &locale,
        theme_of(headers),
        catalog(&locale)
            .get("bulk_confirm")
            .and_then(Value::as_str)
            .unwrap_or("Confirm"),
        true,
        user.is_superuser,
        &user.email,
        &csrf,
        &format!("/admin/models/{model_key}"),
        "",
        "",
        json!({
            "model_label": model.meta().label,
            "list_path": format!("/admin/models/{model_key}"),
            "action": format!("/admin/models/{model_key}/bulk/{draft}"),
            "action_label": if locale == "de" { action.label_de } else { action.label_en },
            "count": count
        }),
    )
    .await
}

async fn apply_bulk(
    state: &AppState,
    headers: &HeaderMap,
    registry: &AdminRegistry,
    user: &AuthUser,
    locale: &str,
    model_key: &str,
    draft: Uuid,
) -> Response {
    let Some(model) = registry.find(model_key) else {
        return error_page(
            state,
            headers,
            registry,
            locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(stored) = drafts::load(
        &state.db,
        draft,
        user.session_id,
        user.id,
        model_key,
        "bulk",
    )
    .await
    .ok()
    .flatten() else {
        return error_page(
            state,
            headers,
            registry,
            locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(name) = stored.values.get("action").and_then(Value::as_str) else {
        return error_page(
            state,
            headers,
            registry,
            locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let Some(action) = registry
        .actions_for(model.meta(), model.columns())
        .into_iter()
        .find(|item| item.name == name)
    else {
        return error_page(
            state,
            headers,
            registry,
            locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    };
    if permit(
        state.auth.as_ref().unwrap(),
        user,
        model.meta(),
        action.permission,
    )
    .await
    .is_err()
    {
        let _ = audit::insert(
            &state.db,
            &Event {
                actor_id: user.id,
                model_key: model.meta().key.to_owned(),
                record_id: String::new(),
                action: if action.permission == "delete" {
                    "bulk_delete"
                } else {
                    "bulk_update"
                }
                .to_owned(),
                outcome: "denied".to_owned(),
                summary: String::new(),
                operation_id: None,
            },
        )
        .await;
        return error_page(
            state,
            headers,
            registry,
            locale,
            StatusCode::FORBIDDEN,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    let Some(raw_ids) = stored.values.get("ids").and_then(Value::as_array) else {
        return error_page(
            state,
            headers,
            registry,
            locale,
            StatusCode::NOT_FOUND,
            "not_found",
            true,
            &user.email,
        )
        .await;
    };
    let mut ids = Vec::new();
    for value in raw_ids {
        let Some(id) = value.as_str().and_then(|text| Uuid::parse_str(text).ok()) else {
            return error_page(
                state,
                headers,
                registry,
                locale,
                StatusCode::UNPROCESSABLE_ENTITY,
                "forbidden",
                true,
                &user.email,
            )
            .await;
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() || ids.len() > action.max_ids {
        return error_page(
            state,
            headers,
            registry,
            locale,
            StatusCode::UNPROCESSABLE_ENTITY,
            "forbidden",
            true,
            &user.email,
        )
        .await;
    }
    let verb = if action.permission == "delete" {
        "delete"
    } else {
        "update"
    };
    let scope = access_scope(model.as_ref(), user, true);
    let applied = {
        let txn = match state.db.begin().await {
            Ok(txn) => txn,
            Err(_) => {
                return error_page(
                    state,
                    headers,
                    registry,
                    locale,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "bulk_failed",
                    true,
                    &user.email,
                )
                .await;
            }
        };
        async {
            let mut rows = Vec::new();
            for id in &ids {
                match get_record(&txn, model.descriptor().table, model.columns(), *id, &scope).await
                {
                    Ok(Some(row)) if model.object_allows(user, verb, &row) => rows.push((*id, row)),
                    Ok(_) => return Err((StatusCode::NOT_FOUND, "bulk_denied")),
                    Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed")),
                }
            }
            let audit_action = if matches!(action.operation, ActionOperation::Delete) {
                "bulk_delete"
            } else {
                "bulk_update"
            };
            if let ActionOperation::Custom(custom) = &action.operation {
                let records: Vec<Value> = rows.iter().map(|(_, row)| row.clone()).collect();
                let mut context = crate::actions::ActionContext::prepare(
                    custom.columns,
                    model.columns(),
                    model.meta(),
                    &records,
                );
                if custom.handler.apply(&mut context).await.is_err() {
                    return Err((StatusCode::UNPROCESSABLE_ENTITY, "bulk_failed"));
                }
                for write in context.planned() {
                    let body = match &write.value {
                        crate::actions::ActionValue::Text(text) => json!({ write.column: text }),
                        crate::actions::ActionValue::Integer(number) => {
                            json!({ write.column: number })
                        }
                        crate::actions::ActionValue::Boolean(flag) => json!({ write.column: flag }),
                    };
                    let before = rows
                        .iter()
                        .find(|(id, _)| *id == write.id)
                        .map(|(_, row)| row);
                    let after = match model.update_in(&txn, write.id, body, &scope, user).await {
                        Ok(row) => row,
                        Err(FerrioxidError::Validation(_))
                        | Err(FerrioxidError::Denied(_))
                        | Err(FerrioxidError::NotFound) => {
                            return Err((StatusCode::UNPROCESSABLE_ENTITY, "bulk_failed"));
                        }
                        Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed")),
                    };
                    let summary = audit::change_summary(model.meta(), before, Some(&after));
                    if audit::insert(
                        &txn,
                        &Event {
                            actor_id: user.id,
                            model_key: model.meta().key.to_owned(),
                            record_id: write.id.to_string(),
                            action: audit_action.to_owned(),
                            outcome: "ok".to_owned(),
                            summary,
                            operation_id: None,
                        },
                    )
                    .await
                    .is_err()
                    {
                        return Err((StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed"));
                    }
                }
                let count = rows.len();
                txn.commit()
                    .await
                    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed"))?;
                return Ok(count);
            }
            for (id, before) in &rows {
                let after = match &action.operation {
                    ActionOperation::Delete => {
                        match delete_record(
                            &txn,
                            model.descriptor().table,
                            *id,
                            &scope,
                            model.columns(),
                        )
                        .await
                        {
                            Ok(1) => None,
                            Ok(_) => return Err((StatusCode::NOT_FOUND, "bulk_denied")),
                            Err(_) => {
                                return Err((StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed"))
                            }
                        }
                    }
                    ActionOperation::SetBool { column, value } => {
                        match update_record(
                            &txn,
                            model.descriptor().table,
                            model.columns(),
                            *id,
                            vec![(*column, DbValue::from(*value))],
                            &scope,
                        )
                        .await
                        {
                            Ok(row) => Some(row),
                            Err(_) => {
                                return Err((StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed"))
                            }
                        }
                    }
                    ActionOperation::Custom(_) => {
                        return Err((StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed"))
                    }
                };
                let summary = audit::change_summary(model.meta(), Some(before), after.as_ref());
                if audit::insert(
                    &txn,
                    &Event {
                        actor_id: user.id,
                        model_key: model.meta().key.to_owned(),
                        record_id: id.to_string(),
                        action: audit_action.to_owned(),
                        outcome: "ok".to_owned(),
                        summary,
                        operation_id: None,
                    },
                )
                .await
                .is_err()
                {
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed"));
                }
            }
            let count = rows.len();
            txn.commit()
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "bulk_failed"))?;
            Ok(count)
        }
        .await
    };
    let count = match applied {
        Ok(count) => count,
        Err((status, key)) => {
            return error_page(
                state,
                headers,
                registry,
                locale,
                status,
                key,
                true,
                &user.email,
            )
            .await;
        }
    };
    let _ = drafts::delete(&state.db, draft, user.session_id, user.id).await;
    redirect(&format!(
        "/admin/models/{model_key}?notice=changed&changed={count}"
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn page_as(
    state: &AppState,
    user: &AuthUser,
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
    mut extra: Value,
) -> Response {
    let navigation = crate::nav::navigation(state, user, registry, locale, path).await;
    if let Some(object) = extra.as_object_mut() {
        object.insert("nav_sections".to_owned(), navigation.sections);
        object.insert("model_groups".to_owned(), navigation.groups);
    }
    page(
        registry,
        template,
        locale,
        theme,
        title,
        signed_in,
        superuser,
        email,
        csrf,
        path,
        notice_text,
        alert,
        extra,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn page(
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
        "primary_color": brand_for_ui(&branding.primary_color),
        "nav_dashboard": template != "error.html" && (path == "/admin" || path == "/admin/"),
        "nav_models": path.starts_with("/admin/models"),
        "nav_audit": path.starts_with("/admin/audit"),
        "nav_users": path.starts_with("/admin/directory/users"),
        "nav_roles": path.starts_with("/admin/directory/roles") && !path.starts_with("/admin/directory/permissions"),
        "nav_permissions": path.starts_with("/admin/directory/permissions"),
        "i18n": catalog(locale),
        "signed_in": signed_in,
        "nav": signed_in,
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
    if value.get("nav_custom").is_none() {
        if let Some(target) = value.as_object_mut() {
            target.insert("nav_custom".to_owned(), Value::Array(Vec::new()));
        }
    }
    if value.get("nav_sections").is_none() {
        if let Some(target) = value.as_object_mut() {
            target.insert("nav_sections".to_owned(), Value::Array(Vec::new()));
        }
    }
    if value.get("model_groups").is_none() {
        if let Some(target) = value.as_object_mut() {
            target.insert("model_groups".to_owned(), Value::Array(Vec::new()));
        }
    }
    match render(template, value, registry.templates()) {
        Ok(body) => Html(body).into_response(),
        Err(error) => {
            tracing::error!(%error, "admin template failed");
            plain(StatusCode::INTERNAL_SERVER_ERROR, "Request rejected")
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn error_page(
    state: &AppState,
    headers: &HeaderMap,
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
    let title = catalog(locale)
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("Error")
        .to_owned();
    let csrf = if signed_in {
        match state.auth.as_ref() {
            Some(auth) => session_csrf(auth, headers).await,
            None => String::new(),
        }
    } else {
        String::new()
    };
    let signed_in = signed_in && !csrf.is_empty();
    let actor = match state.auth.as_ref() {
        Some(auth) if signed_in => auth.authenticate(headers, &Method::GET).await.ok(),
        _ => None,
    };
    let show_nav = actor.is_some() && key != "staff_required";
    let superuser = actor.as_ref().is_some_and(|user| user.is_superuser);
    let mut response = if let Some(user) = actor.as_ref() {
        page_as(
            state,
            user,
            registry,
            "error.html",
            locale,
            theme_of(headers),
            &title,
            true,
            superuser,
            email,
            &csrf,
            "/admin",
            "",
            "",
            json!({ "message": message, "nav": show_nav }),
        )
        .await
    } else {
        page(
            registry,
            "error.html",
            locale,
            theme_of(headers),
            &title,
            false,
            false,
            email,
            &csrf,
            "/admin",
            "",
            "",
            json!({ "message": message, "nav": false }),
        )
    };
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

pub(crate) fn locale_of(
    headers: &HeaderMap,
    default_locale: &str,
    fallback_locale: &str,
) -> String {
    let requested = cookie(headers, "ferrioxid_locale")
        .or_else(|| accept_language(headers))
        .unwrap_or_default();
    normalize(&requested, default_locale, fallback_locale)
}

pub(crate) fn theme_of(headers: &HeaderMap) -> String {
    match cookie(headers, "ferrioxid_theme").as_deref() {
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

pub(crate) async fn session_csrf(
    auth: &ferrioxid_auth::AuthService,
    headers: &HeaderMap,
) -> String {
    auth.csrf_from_request(headers).await.unwrap_or_default()
}

fn login_token(headers: &HeaderMap) -> String {
    cookie(headers, "ferrioxid_login_csrf")
        .filter(|value| valid_login_csrf(value))
        .unwrap_or_else(random_token)
}

fn login_gate(
    auth: &ferrioxid_auth::AuthService,
    headers: &HeaderMap,
    supplied: &str,
) -> Result<(), &'static str> {
    if auth.origin_allowed(headers, &Method::POST).is_err() {
        return Err("origin_rejected");
    }
    if supplied.is_empty() {
        return Err("form_token_missing");
    }
    let Some(expected) = cookie(headers, "ferrioxid_login_csrf") else {
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
        "ferrioxid_login_csrf={token}; HttpOnly; Path=/; SameSite={same}; Max-Age=600{}",
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

pub(crate) fn csrf_token_from(headers: &HeaderMap, body: &Bytes) -> String {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("multipart/form-data")
    {
        return ferrioxid_storage::multipart_text_field(content_type, body, "csrf_token")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default();
    }
    parse_form(body)
        .get("csrf_token")
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn parse_form(body: &Bytes) -> HashMap<String, String> {
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

fn sort_query(params: &HashMap<String, String>, sort: &str, dir: &str) -> String {
    let mut pairs = vec![
        format!(
            "q={}",
            url_encode(params.get("q").map(String::as_str).unwrap_or(""))
        ),
        format!("sort={sort}"),
        format!("dir={dir}"),
    ];
    let mut keys: Vec<&String> = params
        .keys()
        .filter(|key| key.starts_with("filter."))
        .collect();
    keys.sort();
    for key in keys {
        pairs.push(format!(
            "{}={}",
            url_encode(key),
            url_encode(params.get(key).map(String::as_str).unwrap_or(""))
        ));
    }
    pairs.join("&")
}

fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 8 | 13 | 18 | 23) || byte.is_ascii_hexdigit())
}

fn shorten_uuid(value: &str) -> String {
    format!("{}…{}", &value[..8], &value[value.len() - 4..])
}

/// Projektfarbe nur, wenn weiße Schrift darauf mindestens 4,5:1 erreicht.
fn brand_for_ui(hex: &str) -> String {
    if contrast_ratio(hex, "#ffffff") >= 4.5 {
        hex.to_owned()
    } else {
        "#0f172a".to_owned()
    }
}

fn contrast_ratio(left: &str, right: &str) -> f64 {
    let Some(left) = relative_luminance(left) else {
        return 1.0;
    };
    let Some(right) = relative_luminance(right) else {
        return 1.0;
    };
    let (hi, lo) = if left > right {
        (left, right)
    } else {
        (right, left)
    };
    (hi + 0.05) / (lo + 0.05)
}

fn relative_luminance(hex: &str) -> Option<f64> {
    let (red, green, blue) = rgb(hex)?;
    Some(0.2126 * channel_lin(red) + 0.7152 * channel_lin(green) + 0.0722 * channel_lin(blue))
}

fn rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#')?;
    let expanded;
    let hex = if hex.len() == 3 {
        expanded = hex.chars().fold(String::new(), |mut output, char| {
            output.push(char);
            output.push(char);
            output
        });
        expanded.as_str()
    } else {
        hex
    };
    if hex.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ))
}

fn channel_lin(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

const RECORD_HISTORY_LIMIT: usize = 8;

async fn record_history(
    state: &AppState,
    user: &AuthUser,
    model_key: &str,
    record_id: &str,
) -> Vec<Value> {
    if model_key.is_empty() || record_id.is_empty() {
        return Vec::new();
    }
    let actor = (!user.is_superuser).then_some(user.id);
    let Ok((events, _)) =
        audit::matching(&state.db, actor, Some(model_key), Some(record_id), 0).await
    else {
        return Vec::new();
    };
    events
        .into_iter()
        .take(RECORD_HISTORY_LIMIT)
        .map(|event| {
            let actor_label = if event.actor_id == user.id {
                user.email.clone()
            } else {
                event.actor_id.to_string()
            };
            let formatted = audit::format_unix_utc(event.created_at);
            json!({
                "when": formatted.as_ref().map(|(label, _)| label.clone()).unwrap_or_default(),
                "when_iso": formatted.as_ref().map(|(_, iso)| iso.clone()).unwrap_or_default(),
                "actor": actor_label,
                "action": event.action,
                "outcome": event.outcome,
                "summary": event.summary,
            })
        })
        .collect()
}

fn history_href(model: &str, record: &str) -> String {
    format!(
        "/admin/audit?model={}&record={}",
        url_encode(model),
        url_encode(record)
    )
}

fn audit_key(value: &str) -> Option<String> {
    let ok = !value.is_empty()
        && value.len() <= 80
        && value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-' | '.'));
    ok.then(|| value.to_owned())
}

fn audit_page_href(page: u32, model: Option<&str>, record: Option<&str>) -> String {
    let mut href = format!("/admin/audit?page={page}");
    if let Some(model) = model {
        href.push_str("&model=");
        href.push_str(&url_encode(model));
    }
    if let Some(record) = record {
        href.push_str("&record=");
        href.push_str(&url_encode(record));
    }
    href
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

struct DateNavigation {
    bounds: Vec<(&'static str, bool, DbValue)>,
    view: Value,
}

#[allow(clippy::too_many_arguments)]
async fn date_navigation(
    state: &AppState,
    model: &dyn ManagedModel,
    locale: &str,
    params: &HashMap<String, String>,
    search_columns: &[&str],
    search: Option<&str>,
    filters: &[(&str, DbValue)],
    scope: &ferrioxid_core::QueryScope,
    list_path: &str,
) -> Result<DateNavigation, ()> {
    let Some(column) = model.meta().date_hierarchy else {
        return Ok(DateNavigation {
            bounds: Vec::new(),
            view: Value::Null,
        });
    };
    let kind = model
        .columns()
        .iter()
        .find(|item| item.name == column)
        .map(|item| item.kind)
        .ok_or(())?;
    let year = optional_number(params, "year")?;
    let month = optional_number(params, "month")?;
    let day = optional_number(params, "day")?;
    if month.is_some() && year.is_none() || day.is_some() && month.is_none() {
        return Err(());
    }
    let mut bounds = Vec::new();
    if let Some(year) = year {
        if !(1..=9999).contains(&year) {
            return Err(());
        }
        let bound = ferrioxid_core::hierarchy_bound(year as i32, month, day).map_err(|_| ())?;
        let (start, end) = match kind {
            ColumnKind::Date => (DbValue::from(bound.start), DbValue::from(bound.end)),
            ColumnKind::DateTime => (
                DbValue::from(bound.start.and_hms_opt(0, 0, 0).ok_or(())?.and_utc()),
                DbValue::from(bound.end.and_hms_opt(0, 0, 0).ok_or(())?.and_utc()),
            ),
            _ => return Err(()),
        };
        bounds.push((column, true, start));
        bounds.push((column, false, end));
    }
    let width = if year.is_none() {
        4
    } else if month.is_none() {
        7
    } else if day.is_none() {
        10
    } else {
        0
    };
    let buckets = if width == 0 {
        Vec::new()
    } else {
        date_buckets(
            &state.db,
            model.descriptor().table,
            model.columns(),
            column,
            width,
            search_columns,
            search,
            filters,
            &bounds,
            scope,
        )
        .await
        .map_err(|_| ())?
    };
    let root = hierarchy_href(list_path, params, None, None, None);
    let mut crumbs = Vec::new();
    if let Some(year) = year {
        crumbs.push(json!({
            "label": year.to_string(),
            "href": hierarchy_href(list_path, params, Some(year), None, None)
        }));
    }
    if let (Some(year), Some(month)) = (year, month) {
        crumbs.push(json!({
            "label": month_name(locale, month),
            "href": hierarchy_href(list_path, params, Some(year), Some(month), None)
        }));
    }
    let items: Vec<Value> = buckets
        .into_iter()
        .filter_map(|(label, count)| {
            let (text, href) = match width {
                4 => {
                    let value = label.parse::<u32>().ok()?;
                    (
                        label,
                        hierarchy_href(list_path, params, Some(value), None, None),
                    )
                }
                7 => {
                    let month = label.get(5..7)?.parse::<u32>().ok()?;
                    (
                        month_name(locale, month),
                        hierarchy_href(list_path, params, year, Some(month), None),
                    )
                }
                _ => {
                    let day = label.get(8..10)?.parse::<u32>().ok()?;
                    (
                        day.to_string(),
                        hierarchy_href(list_path, params, year, month, Some(day)),
                    )
                }
            };
            Some(json!({ "label": text, "href": href, "count": count }))
        })
        .collect();
    let field = model
        .meta()
        .fields
        .iter()
        .find(|field| field.name == column)
        .map(|field| field.label)
        .unwrap_or(column);
    Ok(DateNavigation {
        bounds,
        view: json!({
            "field": field,
            "root": root,
            "crumbs": crumbs,
            "items": items
        }),
    })
}

fn optional_number(params: &HashMap<String, String>, key: &str) -> Result<Option<u32>, ()> {
    match params.get(key).map(String::as_str) {
        None | Some("") => Ok(None),
        Some(value) => value.parse::<u32>().map(Some).map_err(|_| ()),
    }
}

fn month_name(locale: &str, month: u32) -> String {
    catalog(locale)
        .get(format!("month_{month}"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

fn hierarchy_href(
    list_path: &str,
    params: &HashMap<String, String>,
    year: Option<u32>,
    month: Option<u32>,
    day: Option<u32>,
) -> String {
    let mut pairs = Vec::new();
    for (key, value) in params {
        if matches!(
            key.as_str(),
            "page" | "notice" | "changed" | "year" | "month" | "day"
        ) {
            continue;
        }
        pairs.push(format!("{}={}", url_encode(key), url_encode(value)));
    }
    if let Some(year) = year {
        pairs.push(format!("year={year}"));
    }
    if let Some(month) = month {
        pairs.push(format!("month={month}"));
    }
    if let Some(day) = day {
        pairs.push(format!("day={day}"));
    }
    if pairs.is_empty() {
        list_path.to_owned()
    } else {
        format!("{list_path}?{}", pairs.join("&"))
    }
}

fn normalize_temporal(
    columns: &[ColumnSpec],
    object: &mut Map<String, Value>,
) -> Result<(), (String, &'static str)> {
    for column in columns {
        let Some(value) = object.get(column.name).cloned() else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let Some(text) = value.as_str() else {
            continue;
        };
        if text.is_empty() {
            if column.nullable {
                object.insert(column.name.to_owned(), Value::Null);
            }
            continue;
        }
        let canonical = match column.kind {
            ColumnKind::Date => ferrioxid_core::parse_date(text)
                .map(ferrioxid_core::temporal::format_date)
                .map_err(|code| (column.name.to_owned(), code))?,
            ColumnKind::Time => ferrioxid_core::parse_time(text)
                .map(ferrioxid_core::temporal::format_time)
                .map_err(|code| (column.name.to_owned(), code))?,
            ColumnKind::DateTime => ferrioxid_core::parse_datetime_local(text)
                .or_else(|_| ferrioxid_core::parse_datetime_utc(text))
                .map(ferrioxid_core::temporal::format_datetime)
                .map_err(|code| (column.name.to_owned(), code))?,
            _ => continue,
        };
        object.insert(column.name.to_owned(), json!(canonical));
    }
    Ok(())
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

#[cfg(test)]
mod contrast_tests {
    use super::{brand_for_ui, contrast_ratio, is_uuid};

    #[test]
    fn pale_brand_falls_back_to_slate() {
        assert_eq!(brand_for_ui("#ffffff"), "#0f172a");
        assert_eq!(brand_for_ui("#f4f7fb"), "#0f172a");
    }

    #[test]
    fn readable_brand_keeps_its_hex() {
        assert_eq!(brand_for_ui("#0f172a"), "#0f172a");
        assert!(contrast_ratio("#ffffff", "#9f1239") >= 4.5);
        assert_eq!(brand_for_ui("#9f1239"), "#9f1239");
    }

    #[test]
    fn uuids_are_not_ordinary_labels() {
        assert!(is_uuid("11111111-1111-1111-1111-111111111111"));
        assert!(!is_uuid("Apfel"));
    }
}

#[cfg(test)]
mod relation_label_tests {
    use super::{present_cell, RelationLabels};
    use ferrioxid_core::{ColumnKind, ColumnSpec, ReferentialAction};
    use serde_json::json;
    use std::collections::HashMap;

    fn product_column() -> ColumnSpec {
        ColumnSpec {
            name: "product_id",
            kind: ColumnKind::Uuid,
            nullable: false,
            primary_key: false,
            default_bool: None,
            unique: false,
            indexed: true,
            foreign_table: Some("products"),
            foreign_column: Some("id"),
            on_delete: Some(ReferentialAction::Restrict),
            precision: 0,
            scale: 0,
            choices: &[],
        }
    }

    fn relations(label: Option<&str>) -> HashMap<String, RelationLabels> {
        let id = "11111111-1111-1111-1111-111111111111";
        let mut labels = HashMap::new();
        if let Some(label) = label {
            labels.insert(id.to_owned(), label.to_owned());
        }
        let mut relations = HashMap::new();
        relations.insert(
            "products".to_owned(),
            RelationLabels {
                model_key: if label.is_some() { "products" } else { "" },
                labels,
            },
        );
        relations
    }

    #[test]
    fn visible_target_uses_the_name_and_admin_link() {
        let id = "11111111-1111-1111-1111-111111111111";
        let mut lead = false;
        let cell = present_cell(
            "de",
            "EUR",
            &[product_column()],
            "product_id",
            &json!(id),
            &relations(Some("Apfel")),
            false,
            &mut lead,
        );
        assert_eq!(cell["text"], "Apfel");
        assert_eq!(cell["href"], format!("/admin/models/products/{id}"));
        assert_ne!(cell["text"].as_str(), Some(id));
    }

    #[test]
    fn missing_and_hidden_targets_share_one_label_without_a_link() {
        let id = "11111111-1111-1111-1111-111111111111";
        let columns = [product_column()];
        let hidden = relations(None);
        let mut readable = relations(Some("Apfel"));
        readable.get_mut("products").unwrap().labels.remove(id);
        let mut lead = false;
        let hidden_cell = present_cell(
            "de",
            "EUR",
            &columns,
            "product_id",
            &json!(id),
            &hidden,
            false,
            &mut lead,
        );
        lead = false;
        let missing_cell = present_cell(
            "de",
            "EUR",
            &columns,
            "product_id",
            &json!(id),
            &readable,
            false,
            &mut lead,
        );
        assert_eq!(hidden_cell["text"], "Nicht sichtbar");
        assert_eq!(missing_cell["text"], hidden_cell["text"]);
        assert_eq!(hidden_cell["href"], "");
        assert_eq!(missing_cell["href"], "");
        assert_ne!(hidden_cell["text"].as_str(), Some(id));
    }
}
