//! Admin-HTTP. Sitzung, Staff-Flag, Modellrecht und CSRF kommen aus ferrioxid-auth.

use crate::assets;
use crate::registry::{redact, sensitive, AdminRegistry, ManagedModel};
use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use ferrioxid_auth::{AppState, AuthError, AuthService, AuthUser};
use ferrioxid_core::{
    problem_json, AdminFieldMeta, AdminModelMeta, ColumnKind, ColumnSpec, FerrioxidError,
    QueryScope,
};
use ferrioxid_db::DeleteOutcome;
use sea_orm::Value as DbValue;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub fn mount(router: Router<AppState>, registry: AdminRegistry) -> Router<AppState> {
    let router = crate::html::routes(router, registry.clone());
    let api = Router::new()
        .route("/config", get(public_config))
        .route("/me", get(me))
        .route("/models", get(models))
        .route("/models/{model}/schema", get(schema))
        .route("/models/{model}/records", get(records).post(create_record))
        .route(
            "/models/{model}/records/{id}",
            get(read_record).patch(update_record).delete(delete_record),
        )
        .route(
            "/models/{model}/fields/{field}/options",
            get(relation_options),
        )
        .route("/directory/users", get(list_users))
        .route("/directory/users/{id}/active", post(set_active))
        .route("/directory/users/{id}/staff", post(set_staff_flag))
        .route("/directory/roles", get(list_roles).post(create_role))
        .route("/directory/roles/{role}/permissions", get(role_permissions))
        .route(
            "/directory/roles/{role}/permissions/{code}",
            post(grant).delete(revoke),
        )
        .route(
            "/directory/assignments",
            post(assign_role).delete(revoke_role),
        )
        .layer(Extension(registry));
    router
        .nest("/admin/api", api)
        .route("/admin/assets/{*path}", get(admin_asset))
}

async fn admin_asset(Path(path): Path<String>) -> axum::response::Response {
    assets::asset(path).await
}

#[derive(Deserialize)]
struct ListParams {
    page: Option<u32>,
    page_size: Option<u32>,
    sort: Option<String>,
    dir: Option<String>,
    q: Option<String>,
}

async fn public_config(Extension(registry): Extension<AdminRegistry>) -> Json<Value> {
    let branding = registry.branding();
    Json(json!({
        "site_title": branding.site_title,
        "site_header": branding.site_header,
        "app_name": branding.app_name,
        "primary_color": branding.primary_color,
        "default_locale": branding.default_locale,
        "fallback_locale": branding.fallback_locale,
        "supported_locales": ["de", "en"],
        "currency": branding.currency
    }))
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> axum::response::Response {
    match staff(&state, &headers, &Method::GET).await {
        Ok(user) => Json(public_user(&user)).into_response(),
        Err(response) => response,
    }
}

async fn models(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::GET).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    let mut visible = Vec::new();
    for model in registry.models() {
        if permit(auth, &user, model.meta(), "read").await.is_ok() {
            visible.push(json!({
                "key": model.meta().key,
                "label": model.meta().label,
                "list_blocked": list_blocked(model.as_ref()),
            }));
        }
    }
    Json(json!({ "models": visible })).into_response()
}

async fn schema(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::GET).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return failure(StatusCode::NOT_FOUND, "model was not found");
    };
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    if let Err(error) = permit(auth, &user, model.meta(), "read").await {
        return ferrioxid_failure(error);
    }
    let mut actions = Vec::new();
    for action in ["read", "create", "update", "delete"] {
        if permit(auth, &user, model.meta(), action).await.is_err() {
            continue;
        }
        if let Some(code) = registry.action_permission(model.meta().key, action) {
            match auth.holds(&user, code).await {
                Ok(true) => actions.push(action),
                Ok(false) => {}
                Err(_) => {
                    return failure(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "permission lookup failed",
                    )
                }
            }
        } else {
            actions.push(action);
        }
    }
    Json(schema_json(
        model.meta().key,
        model.meta().label,
        list_blocked(model.as_ref()),
        registry.field_slice(model.meta()),
        model.columns(),
        &actions,
    ))
    .into_response()
}

async fn records(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
    Query(params): Query<ListParams>,
    axum::extract::RawQuery(raw): axum::extract::RawQuery,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::GET).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return failure(StatusCode::NOT_FOUND, "model was not found");
    };
    if let Err(error) = gate(&state, &registry, &user, model.as_ref(), "read").await {
        return ferrioxid_failure(error);
    }
    if list_blocked(model.as_ref()) {
        return failure(
            StatusCode::FORBIDDEN,
            "object policy is not bound; call add_policy",
        );
    }
    let scope = access_scope(model.as_ref(), &user, false);
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(25).clamp(1, 100);
    let fields = registry.field_slice(model.meta());
    let sort = params.sort.as_deref().unwrap_or("id");
    if !fields
        .iter()
        .any(|field| field.name == sort && (field.sortable || field.name == "id"))
    {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "sort field is not allowed",
        );
    }
    let descending = matches!(params.dir.as_deref(), Some("desc"));
    let search_columns: Vec<&str> = fields
        .iter()
        .filter(|field| field.searchable)
        .map(|field| field.name)
        .collect();
    if params.q.as_ref().is_some_and(|term| !term.is_empty()) && search_columns.is_empty() {
        return failure(StatusCode::UNPROCESSABLE_ENTITY, "search is not available");
    }
    let filters = match filters_from(
        raw.as_deref().unwrap_or(""),
        model.columns(),
        &registry,
        model.meta(),
        registry.filters(),
    ) {
        Ok(filters) => filters,
        Err(message) => return failure(StatusCode::UNPROCESSABLE_ENTITY, &message),
    };
    let filter_refs: Vec<(&str, DbValue)> = filters
        .iter()
        .map(|(name, value)| (*name, value.clone()))
        .collect();
    match model
        .page(
            &state.db,
            &search_columns,
            params.q.as_deref().filter(|term| !term.is_empty()),
            sort,
            descending,
            &filter_refs,
            &[],
            u64::from(page - 1) * u64::from(page_size),
            u64::from(page_size),
            &scope,
        )
        .await
    {
        Ok((mut rows, has_more, total)) => {
            for row in &mut rows {
                redact(model.meta(), row);
            }
            Json(json!({
                "data": rows,
                "page": page,
                "page_size": page_size,
                "has_more": has_more,
                "total": total
            }))
            .into_response()
        }
        Err(error) => ferrioxid_failure(error),
    }
}

async fn read_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::GET).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return failure(StatusCode::NOT_FOUND, "model was not found");
    };
    if let Err(error) = gate(&state, &registry, &user, model.as_ref(), "read").await {
        return ferrioxid_failure(error);
    }
    record_or_hidden(&user, model.as_ref(), &state, id, "read").await
}

async fn create_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path(model_key): Path<String>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::POST).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return failure(StatusCode::NOT_FOUND, "model was not found");
    };
    if let Err(error) = gate(&state, &registry, &user, model.as_ref(), "create").await {
        return ferrioxid_failure(error);
    }
    let Some(object) = body.as_object() else {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "request body must be an object",
        );
    };
    if let Err(message) = reject_locked(registry.field_slice(model.meta()), object, true) {
        return failure(StatusCode::UNPROCESSABLE_ENTITY, &message);
    }
    match model.create(&state.db, body, &user).await {
        Ok(mut created) => {
            redact(model.meta(), &mut created);
            (StatusCode::CREATED, Json(created)).into_response()
        }
        Err(error) => ferrioxid_failure(error),
    }
}

async fn update_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::PATCH).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return failure(StatusCode::NOT_FOUND, "model was not found");
    };
    if let Err(error) = gate(&state, &registry, &user, model.as_ref(), "update").await {
        return ferrioxid_failure(error);
    }
    let Some(object) = body.as_object() else {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "request body must be an object",
        );
    };
    if let Err(message) = reject_locked(registry.field_slice(model.meta()), object, false) {
        return failure(StatusCode::UNPROCESSABLE_ENTITY, &message);
    }
    let scope = access_scope(model.as_ref(), &user, true);
    if needs_object_check(model.as_ref(), &user) {
        let current = match model.read(&state.db, id, &scope).await {
            Ok(Some(value)) => value,
            Ok(None) => return failure(StatusCode::NOT_FOUND, "Resource not found"),
            Err(error) => return ferrioxid_failure(error),
        };
        if !model.object_allows(&user, "update", &current) {
            return failure(StatusCode::NOT_FOUND, "Resource not found");
        }
    }
    match model.update(&state.db, id, body, &scope, &user).await {
        Ok(mut updated) => {
            redact(model.meta(), &mut updated);
            Json(updated).into_response()
        }
        Err(error) => ferrioxid_failure(error),
    }
}

async fn delete_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, id)): Path<(String, Uuid)>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::DELETE).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return failure(StatusCode::NOT_FOUND, "model was not found");
    };
    if let Err(error) = gate(&state, &registry, &user, model.as_ref(), "delete").await {
        return ferrioxid_failure(error);
    }
    let scope = access_scope(model.as_ref(), &user, true);
    if needs_object_check(model.as_ref(), &user) {
        let current = match model.read(&state.db, id, &scope).await {
            Ok(Some(value)) => value,
            Ok(None) => return failure(StatusCode::NOT_FOUND, "Resource not found"),
            Err(error) => return ferrioxid_failure(error),
        };
        if !model.object_allows(&user, "delete", &current) {
            return failure(StatusCode::NOT_FOUND, "Resource not found");
        }
    }
    match model.remove(&state.db, id, &scope, &user).await {
        Ok(DeleteOutcome::Deleted) => StatusCode::NO_CONTENT.into_response(),
        Ok(DeleteOutcome::Missing) => failure(StatusCode::NOT_FOUND, "Resource not found"),
        Err(error) => ferrioxid_failure(error),
    }
}

async fn relation_options(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(registry): Extension<AdminRegistry>,
    Path((model_key, field)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::GET).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let Some(model) = registry.find(&model_key) else {
        return failure(StatusCode::NOT_FOUND, "model was not found");
    };
    if let Err(error) = gate(&state, &registry, &user, model.as_ref(), "read").await {
        return ferrioxid_failure(error);
    }
    let Some(column) = model.columns().iter().find(|column| column.name == field) else {
        return failure(StatusCode::NOT_FOUND, "field was not found");
    };
    if model
        .meta()
        .fields
        .iter()
        .any(|meta| meta.name == field && meta.hidden)
        || sensitive(&field)
    {
        return failure(StatusCode::NOT_FOUND, "field was not found");
    }
    let Some(target_table) = column.foreign_table else {
        return failure(StatusCode::UNPROCESSABLE_ENTITY, "field is not a relation");
    };
    if registry
        .models()
        .iter()
        .filter(|candidate| candidate.descriptor().table == target_table)
        .count()
        > 1
    {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "relation target is ambiguous",
        );
    }
    let Some(target) = registry.model_for_table(target_table) else {
        return failure(
            StatusCode::FORBIDDEN,
            "relation options require a registered admin model without an object policy",
        );
    };
    if list_blocked(target.as_ref()) {
        return failure(
            StatusCode::FORBIDDEN,
            "object policy is not bound; call add_policy",
        );
    }
    let scope = access_scope(target.as_ref(), &user, false);
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    if let Err(error) = permit(auth, &user, target.meta(), "read").await {
        return ferrioxid_failure(error);
    }
    let Some(label) = display_column(registry.field_slice(target.meta()), target.columns()) else {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "relation has no searchable display field",
        );
    };
    let term = params
        .get("q")
        .map(String::as_str)
        .filter(|term| !term.is_empty());
    let search_columns: Vec<&str> = if term.is_some() {
        vec![label]
    } else {
        Vec::new()
    };
    match target
        .page(
            &state.db,
            &search_columns,
            term,
            label,
            false,
            &[],
            &[],
            0,
            25,
            &scope,
        )
        .await
    {
        Ok((rows, _, _)) => {
            let options: Vec<Value> = rows
                .into_iter()
                .filter_map(|row| {
                    let id = row.get("id")?.clone();
                    let text = row.get(label)?.clone();
                    Some(json!({ "id": id, "label": text }))
                })
                .collect();
            Json(json!({ "options": options })).into_response()
        }
        Err(error) => ferrioxid_failure(error),
    }
}

#[derive(Deserialize)]
struct FlagBody {
    active: Option<bool>,
    staff: Option<bool>,
}

async fn list_users(State(state): State<AppState>, headers: HeaderMap) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::GET).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(response) = superuser_only(&user) {
        return response;
    }
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    let users = match auth.list_users().await {
        Ok(users) => users,
        Err(error) => return auth_failure(error),
    };
    let links = match auth.list_user_role_names().await {
        Ok(links) => links,
        Err(error) => return auth_failure(error),
    };
    let mut roles_by_user: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (user_id, name) in links {
        roles_by_user.entry(user_id).or_default().push(name);
    }
    let users: Vec<Value> = users
        .into_iter()
        .map(|user| {
            let mut roles = roles_by_user.remove(&user.id).unwrap_or_default();
            roles.sort();
            json!({
                "id": user.id,
                "email": user.email,
                "is_active": user.is_active,
                "is_staff": user.is_staff,
                "is_superuser": user.is_superuser,
                "roles": roles
            })
        })
        .collect();
    Json(json!({ "users": users })).into_response()
}

async fn set_active(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<FlagBody>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::POST).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(response) = superuser_only(&user) {
        return response;
    }
    let Some(active) = body.active else {
        return failure(StatusCode::UNPROCESSABLE_ENTITY, "active is required");
    };
    if user.id == id && !active {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "you cannot deactivate your own account",
        );
    }
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    let Some(target) = auth
        .list_users()
        .await
        .ok()
        .and_then(|users| users.into_iter().find(|candidate| candidate.id == id))
    else {
        return failure(StatusCode::NOT_FOUND, "user was not found");
    };
    if target.is_superuser {
        return failure(StatusCode::FORBIDDEN, "superuser changes are blocked");
    }
    match crate::directory::set_active(auth, user.id, id, active).await {
        Ok(()) => Json(json!({ "status": "updated" })).into_response(),
        Err(error) => ferrioxid_failure(error),
    }
}

async fn set_staff_flag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<FlagBody>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::POST).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(response) = superuser_only(&user) {
        return response;
    }
    let Some(staff_flag) = body.staff else {
        return failure(StatusCode::UNPROCESSABLE_ENTITY, "staff is required");
    };
    if user.id == id && !staff_flag {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "you cannot remove your own staff access",
        );
    }
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    let Some(target) = auth
        .list_users()
        .await
        .ok()
        .and_then(|users| users.into_iter().find(|candidate| candidate.id == id))
    else {
        return failure(StatusCode::NOT_FOUND, "user was not found");
    };
    if target.is_superuser {
        return failure(StatusCode::FORBIDDEN, "superuser changes are blocked");
    }
    match crate::directory::set_staff(auth, user.id, id, staff_flag).await {
        Ok(()) => Json(json!({ "status": "updated" })).into_response(),
        Err(error) => ferrioxid_failure(error),
    }
}

#[derive(Deserialize)]
struct RoleBody {
    name: Option<String>,
    email: Option<String>,
    role: Option<String>,
}

async fn list_roles(State(state): State<AppState>, headers: HeaderMap) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::GET).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(response) = superuser_only(&user) {
        return response;
    }
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    match auth.list_roles().await {
        Ok(roles) => {
            let roles: Vec<Value> = roles
                .into_iter()
                .map(|(id, name)| json!({ "id": id, "name": name }))
                .collect();
            Json(json!({ "roles": roles })).into_response()
        }
        Err(error) => auth_failure(error),
    }
}

async fn create_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RoleBody>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::POST).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(response) = superuser_only(&user) {
        return response;
    }
    let Some(name) = body.name else {
        return failure(StatusCode::UNPROCESSABLE_ENTITY, "name is required");
    };
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    match auth.ensure_role(&name).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id, "name": name }))).into_response(),
        Err(error) => auth_failure(error),
    }
}

async fn role_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role): Path<String>,
) -> axum::response::Response {
    let user = match staff(&state, &headers, &Method::GET).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(response) = superuser_only(&user) {
        return response;
    }
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    match auth.list_role_permissions(&role).await {
        Ok(codes) => Json(json!({ "permissions": codes })).into_response(),
        Err(error) => auth_failure(error),
    }
}

async fn grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role, code)): Path<(String, String)>,
) -> axum::response::Response {
    mutate_permission(&state, &headers, &Method::POST, &role, &code, true).await
}

async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role, code)): Path<(String, String)>,
) -> axum::response::Response {
    mutate_permission(&state, &headers, &Method::DELETE, &role, &code, false).await
}

async fn mutate_permission(
    state: &AppState,
    headers: &HeaderMap,
    method: &Method,
    role: &str,
    code: &str,
    grant_permission: bool,
) -> axum::response::Response {
    let user = match staff(state, headers, method).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(response) = superuser_only(&user) {
        return response;
    }
    if code.contains("superuser") {
        return failure(StatusCode::FORBIDDEN, "superuser changes are blocked");
    }
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    let result =
        crate::directory::grant_permission(auth, user.id, role, code, grant_permission).await;
    match result {
        Ok(()) => Json(json!({ "status": "updated" })).into_response(),
        Err(error) => ferrioxid_failure(error),
    }
}

async fn assign_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RoleBody>,
) -> axum::response::Response {
    change_assignment(&state, &headers, body, true).await
}

async fn revoke_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RoleBody>,
) -> axum::response::Response {
    change_assignment(&state, &headers, body, false).await
}

async fn change_assignment(
    state: &AppState,
    headers: &HeaderMap,
    body: RoleBody,
    assign: bool,
) -> axum::response::Response {
    let user = match staff(
        state,
        headers,
        if assign {
            &Method::POST
        } else {
            &Method::DELETE
        },
    )
    .await
    {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(response) = superuser_only(&user) {
        return response;
    }
    let (Some(email), Some(role)) = (body.email, body.role) else {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "email and role are required",
        );
    };
    let Some(auth) = state.auth.as_ref() else {
        return failure(StatusCode::FORBIDDEN, "authentication is not configured");
    };
    let result = crate::directory::assign_role(auth, user.id, &email, &role, assign).await;
    match result {
        Ok(()) => Json(json!({ "status": "updated" })).into_response(),
        Err(error) => ferrioxid_failure(error),
    }
}

async fn record_or_hidden(
    user: &AuthUser,
    model: &dyn ManagedModel,
    state: &AppState,
    id: Uuid,
    action: &str,
) -> axum::response::Response {
    if model.descriptor().requires_object_policy && !model.policy_bound() {
        return failure(
            StatusCode::FORBIDDEN,
            "object policy is not bound; call add_policy",
        );
    }
    let scope = access_scope(model, user, action != "read");
    match model.read(&state.db, id, &scope).await {
        Ok(Some(mut value)) => {
            if needs_object_check(model, user) && !model.object_allows(user, action, &value) {
                return failure(StatusCode::NOT_FOUND, "Resource not found");
            }
            redact(model.meta(), &mut value);
            Json(value).into_response()
        }
        Ok(None) => failure(StatusCode::NOT_FOUND, "Resource not found"),
        Err(error) => ferrioxid_failure(error),
    }
}

pub(crate) fn list_blocked(model: &dyn ManagedModel) -> bool {
    model.descriptor().requires_object_policy && !model.policy_bound()
}

pub(crate) fn access_scope(model: &dyn ManagedModel, user: &AuthUser, write: bool) -> QueryScope {
    if !model.policy_bound() || user.is_superuser {
        QueryScope::All
    } else if write {
        model.write_scope(user)
    } else {
        model.read_scope(user)
    }
}

fn needs_object_check(model: &dyn ManagedModel, user: &AuthUser) -> bool {
    model.policy_bound() && model.descriptor().requires_object_policy && !user.is_superuser
}

pub(crate) async fn gate(
    state: &AppState,
    registry: &AdminRegistry,
    user: &AuthUser,
    model: &dyn ManagedModel,
    action: &str,
) -> Result<(), FerrioxidError> {
    if model.descriptor().requires_object_policy && !model.policy_bound() {
        return Err(FerrioxidError::Denied(
            "object policy is not bound; call add_policy".into(),
        ));
    }
    let Some(auth) = state.auth.as_ref() else {
        return Err(FerrioxidError::Denied(
            "authentication is not configured".into(),
        ));
    };
    permit(auth, user, model.meta(), action).await?;
    if let Some(code) = registry.action_permission(model.meta().key, action) {
        let allowed = auth.holds(user, code).await.map_err(FerrioxidError::from)?;
        if !allowed {
            return Err(FerrioxidError::Forbidden);
        }
    }
    Ok(())
}

pub(crate) async fn permit(
    auth: &AuthService,
    user: &AuthUser,
    meta: &AdminModelMeta,
    action: &str,
) -> Result<(), FerrioxidError> {
    if let Some(permission) = meta.permission {
        auth.authorize(user, Some(permission), action)
            .await
            .map_err(FerrioxidError::from)
    } else if user.is_superuser {
        Ok(())
    } else {
        Err(FerrioxidError::Forbidden)
    }
}

#[allow(clippy::result_large_err)]
async fn staff(
    state: &AppState,
    headers: &HeaderMap,
    method: &Method,
) -> Result<AuthUser, axum::response::Response> {
    let Some(auth) = state.auth.as_ref() else {
        return Err(failure(
            StatusCode::FORBIDDEN,
            "authentication is not configured",
        ));
    };
    let user = match auth.authenticate(headers, method).await {
        Ok(user) => user,
        Err(error) => return Err(auth_failure(error)),
    };
    if !user.is_staff {
        return Err(failure(StatusCode::FORBIDDEN, "staff access is required"));
    }
    Ok(user)
}

#[allow(clippy::result_large_err)]
fn superuser_only(user: &AuthUser) -> Result<(), axum::response::Response> {
    if user.is_superuser {
        Ok(())
    } else {
        Err(failure(
            StatusCode::FORBIDDEN,
            "this directory action requires a superuser",
        ))
    }
}

pub(crate) fn reject_locked(
    fields: &[AdminFieldMeta],
    object: &Map<String, Value>,
    creating: bool,
) -> Result<(), String> {
    for key in object.keys() {
        if sensitive(key) {
            return Err(format!("field:{key}: field is not writable"));
        }
        let Some(field) = fields.iter().find(|field| field.name == key) else {
            return Err(format!("field:{key}: field is not writable"));
        };
        if field.hidden || field.readonly || (creating && key == "id") {
            return Err(format!("field:{key}: field is not writable"));
        }
    }
    Ok(())
}

pub(crate) fn filters_from(
    raw: &str,
    columns: &[ColumnSpec],
    registry: &AdminRegistry,
    meta: &AdminModelMeta,
    custom: &[crate::filters::AdminFilter],
) -> Result<Vec<(&'static str, DbValue)>, String> {
    let fields = registry.field_slice(meta);
    let mut filters = Vec::new();
    for pair in raw.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = urlencoding_decode(key);
        let Some(field) = key.strip_prefix("filter.") else {
            continue;
        };
        let field_name = urlencoding_decode(field);
        if let Some(filter) = custom
            .iter()
            .find(|filter| filter.model_key == meta.key && filter.name == field_name)
        {
            let value = urlencoding_decode(value);
            if value.is_empty() {
                continue;
            }
            filters.push(filter.bind(columns, &value)?);
            continue;
        }
        let Some(meta_field) = fields.iter().find(|candidate| candidate.name == field_name) else {
            return Err("filter field is not allowed".into());
        };
        if !meta_field.filterable || meta_field.hidden {
            return Err("filter field is not allowed".into());
        }
        let Some(column) = columns.iter().find(|column| column.name == field_name) else {
            return Err("filter field is not allowed".into());
        };
        let value = urlencoding_decode(value);
        if value.is_empty() {
            continue;
        }
        filters.push((meta_field.name, filter_value(column, &value)?));
    }
    Ok(filters)
}

fn filter_value(column: &ColumnSpec, value: &str) -> Result<DbValue, String> {
    match column.kind {
        ColumnKind::Text => Ok(DbValue::from(value.to_owned())),
        ColumnKind::Json => Err("json columns have no portable filter".into()),
        ColumnKind::BigInt => value
            .parse::<i64>()
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be an integer", column.name)),
        ColumnKind::Boolean => match value {
            "true" | "1" => Ok(DbValue::from(true)),
            "false" | "0" => Ok(DbValue::from(false)),
            _ => Err(format!("filter {} must be true or false", column.name)),
        },
        ColumnKind::Uuid => Uuid::parse_str(value)
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be a uuid", column.name)),
        ColumnKind::Date => ferrioxid_core::parse_date(value)
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be a date", column.name)),
        ColumnKind::Time => ferrioxid_core::parse_time(value)
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be a time", column.name)),
        ColumnKind::DateTime => ferrioxid_core::parse_datetime_utc(value)
            .map(DbValue::from)
            .map_err(|_| format!("filter {} must be a utc datetime", column.name)),
        ColumnKind::Decimal => {
            let parsed = ferrioxid_core::parse_decimal(value, column.precision, column.scale)
                .map_err(|_| format!("filter {} must be a decimal", column.name))?;
            let bytes = ferrioxid_core::encode_decimal(parsed, column.precision, column.scale)
                .map_err(|_| format!("filter {} must be a decimal", column.name))?;
            Ok(DbValue::Bytes(Some(Box::new(bytes.to_vec()))))
        }
        ColumnKind::Choice => {
            if !ferrioxid_core::choice_is_known(column.choices, value) {
                return Err(format!("filter {} is not a known choice", column.name));
            }
            Ok(DbValue::from(value.to_owned()))
        }
    }
}

pub(crate) fn display_column(
    fields: &[AdminFieldMeta],
    columns: &[ColumnSpec],
) -> Option<&'static str> {
    let usable = |field: &AdminFieldMeta| {
        field.name != "id"
            && !field.hidden
            && !sensitive(field.name)
            && columns
                .iter()
                .any(|column| column.name == field.name && column.kind == ColumnKind::Text)
    };
    fields
        .iter()
        .find(|field| field.list_display && usable(field))
        .or_else(|| {
            fields
                .iter()
                .find(|field| field.searchable && usable(field))
        })
        .or_else(|| fields.iter().find(|field| usable(field)))
        .map(|field| field.name)
}

fn schema_json(
    key: &str,
    label: &str,
    list_blocked: bool,
    fields: &[AdminFieldMeta],
    columns: &[ColumnSpec],
    actions: &[&str],
) -> Value {
    let rendered: Vec<Value> = fields
        .iter()
        .filter(|field| !field.hidden && !sensitive(field.name))
        .map(|field| field_json(field, columns))
        .collect();
    json!({
        "model": key,
        "label": label,
        "list_blocked": list_blocked,
        "actions": actions,
        "fields": rendered
    })
}

fn field_json(field: &AdminFieldMeta, columns: &[ColumnSpec]) -> Value {
    let column = columns.iter().find(|column| column.name == field.name);
    json!({
        "name": field.name,
        "label": field.label,
        "type": column.map(column_type).unwrap_or("string"),
        "required": column.is_some_and(|column| !column.nullable && column.default_bool.is_none() && !column.primary_key),
        "readonly": field.readonly || column.is_some_and(|column| column.primary_key),
        "nullable": column.is_some_and(|column| column.nullable),
        "searchable": field.searchable,
        "sortable": field.sortable,
        "filterable": field.filterable,
        "list_display": field.list_display,
        "foreign_key": column.and_then(|column| column.foreign_table),
    })
}

fn column_type(column: &ColumnSpec) -> &'static str {
    match column.kind {
        ColumnKind::Text => "string",
        ColumnKind::BigInt => "integer",
        ColumnKind::Boolean => "boolean",
        ColumnKind::Uuid => "uuid",
        ColumnKind::Date => "date",
        ColumnKind::Time => "time",
        ColumnKind::DateTime => "datetime",
        ColumnKind::Decimal => "decimal",
        ColumnKind::Choice => "choice",
        ColumnKind::Json => "json",
    }
}

fn public_user(user: &AuthUser) -> Value {
    json!({
        "id": user.id,
        "email": user.email,
        "is_staff": user.is_staff,
        "is_superuser": user.is_superuser
    })
}

fn urlencoding_decode(value: &str) -> String {
    let mut output = String::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    output.push(byte as char);
                    index += 3;
                } else {
                    output.push('%');
                    index += 1;
                }
            }
            byte => {
                output.push(byte as char);
                index += 1;
            }
        }
    }
    output
}

fn ferrioxid_failure(failure_error: FerrioxidError) -> axum::response::Response {
    match failure_error {
        FerrioxidError::Validation(message) => failure(StatusCode::UNPROCESSABLE_ENTITY, &message),
        FerrioxidError::Forbidden => failure(StatusCode::FORBIDDEN, "Access denied"),
        FerrioxidError::Denied(message) => failure(StatusCode::FORBIDDEN, &message),
        FerrioxidError::Unauthorized(message) => failure(StatusCode::UNAUTHORIZED, &message),
        FerrioxidError::NotFound => failure(StatusCode::NOT_FOUND, "Resource not found"),
        FerrioxidError::Database(source) => {
            tracing::error!(error = %source, "admin database operation failed");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database operation failed",
            )
        }
    }
}

fn auth_failure(error: AuthError) -> axum::response::Response {
    ferrioxid_failure(FerrioxidError::from(error))
}

fn failure(status: StatusCode, detail: &str) -> axum::response::Response {
    let title = match status {
        StatusCode::BAD_REQUEST => "Bad request",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::FORBIDDEN => "Forbidden",
        StatusCode::NOT_FOUND => "Not found",
        StatusCode::CONFLICT => "Conflict",
        StatusCode::UNPROCESSABLE_ENTITY => "Validation failed",
        StatusCode::TOO_MANY_REQUESTS => "Too many requests",
        _ => "Error",
    };
    (status, Json(problem_json(status.as_u16(), title, detail))).into_response()
}

use axum::response::IntoResponse;
