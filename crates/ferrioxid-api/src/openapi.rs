//! OpenAPI 3.1 built from registered model metadata.
//!
//! The document lists generated model routes, the health check, the auth routes
//! that `mount` always installs, and operations added with [`super::App::document`].
//! Admin routes are not part of this registry and do not appear.

use std::collections::{BTreeMap, BTreeSet};

use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use ferrioxid_core::{
    AdminFieldMeta, AdminModelMeta, ColumnKind, ColumnSpec, FerrioxidModel, FieldContract,
    ModelDescriptor, NumericBound, ReferentialAction,
};
use ferrioxid_db::PAGE_LIMIT;
use serde_json::{json, Map, Value};

use crate::App;

const AUTH_PATHS: &[&str] = &[
    "/auth/login",
    "/auth/logout",
    "/auth/me",
    "/auth/csrf",
    "/auth/anonymous",
    "/auth/change-password",
    "/auth/password-reset",
    "/auth/password-reset/confirm",
    "/auth/password-reset/choose",
];

#[derive(Clone, Debug)]
pub struct OpenApiOptions {
    pub enabled: bool,
    pub json_path: String,
    pub ui_path: Option<String>,
    pub title: String,
}

impl Default for OpenApiOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            json_path: "/openapi.json".to_owned(),
            ui_path: Some("/docs".to_owned()),
            title: "Ferrioxid".to_owned(),
        }
    }
}

impl OpenApiOptions {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

/// One extra operation for a route the application already registered.
#[derive(Clone, Debug)]
pub struct OpenApiOperation {
    pub method: String,
    pub path: String,
    pub operation_id: String,
    pub summary: String,
    pub tag: String,
    pub request_schema: Option<Value>,
    pub response_status: u16,
    pub response_schema: Option<Value>,
    /// Session cookie is required.
    pub session: bool,
    /// Cookie writes also require the CSRF header or form field.
    pub csrf: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelAccess {
    Anonymous,
    Permission(&'static str),
    Denied,
    Policy,
    PolicyUnbound,
}

#[derive(Clone, Copy)]
pub(crate) struct DocumentedModel {
    pub descriptor: &'static ModelDescriptor,
    pub columns: &'static [ColumnSpec],
    pub bounds: &'static [NumericBound],
    pub contracts: &'static [FieldContract],
    pub admin: Option<&'static AdminModelMeta>,
    pub access: ModelAccess,
}

impl DocumentedModel {
    pub(crate) fn from_model<M: FerrioxidModel>(access: ModelAccess) -> Self {
        Self {
            descriptor: M::descriptor(),
            columns: M::columns(),
            bounds: M::numeric_bounds(),
            contracts: M::field_contracts(),
            admin: M::admin_meta(),
            access,
        }
    }
}

pub(crate) fn check(app: &App) -> Result<(), String> {
    if !app.openapi.enabled {
        return Ok(());
    }
    validate_path(&app.openapi.json_path)?;
    if let Some(ui) = &app.openapi.ui_path {
        validate_path(ui)?;
        if ui == &app.openapi.json_path {
            return Err("The OpenAPI document and the explorer need different paths.".into());
        }
        if app.paths.contains(ui) {
            return Err(format!(
                "OpenAPI explorer path `{ui}` is already registered."
            ));
        }
    }
    if app.paths.contains(&app.openapi.json_path) {
        return Err(format!(
            "OpenAPI path `{}` is already registered.",
            app.openapi.json_path
        ));
    }
    let mut ids = BTreeSet::new();
    for operation in &app.openapi_operations {
        validate_operation(app, operation, &mut ids)?;
    }
    Ok(())
}

pub(crate) fn specification(app: &App) -> Value {
    let mut paths: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut schemas: BTreeMap<String, Value> = BTreeMap::new();
    schemas.insert("Problem".to_owned(), problem_schema());
    insert_path(&mut paths, "/healthz", "get", health_operation());
    for path in AUTH_PATHS {
        for (method, operation) in auth_operations(path) {
            insert_path(&mut paths, path, method, operation);
        }
    }
    let mut models = app.documented.clone();
    models.sort_by_key(|model| model.descriptor.api_path);
    let mut seen_paths = BTreeSet::new();
    for model in models.iter().rev() {
        if !seen_paths.insert(model.descriptor.api_path) {
            continue;
        }
        add_model(app, model, &mut paths, &mut schemas);
    }
    for operation in &app.openapi_operations {
        let method = operation.method.to_ascii_lowercase();
        insert_path(
            &mut paths,
            &operation.path,
            &method,
            custom_operation(operation),
        );
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": app.openapi.title,
            "version": "experimental"
        },
        "paths": path_value(paths),
        "components": {
            "schemas": schema_value(schemas),
            "securitySchemes": security_schemes()
        }
    })
}

pub(crate) fn mount(router: Router, specification: Value, options: &OpenApiOptions) -> Router {
    let body = std::sync::Arc::new(specification);
    let json_body = body.clone();
    let mut router = router.route(
        &options.json_path,
        get(move || {
            let body = json_body.clone();
            async move { Json(body.as_ref().clone()) }
        }),
    );
    if let Some(ui_path) = options.ui_path.clone() {
        let page = explorer_page(&options.title, &options.json_path);
        router = router.route(
            &ui_path,
            get(move || {
                let page = page.clone();
                async move { Html(page) }
            }),
        );
    }
    router
}

fn add_model(
    app: &App,
    model: &DocumentedModel,
    paths: &mut BTreeMap<String, Map<String, Value>>,
    schemas: &mut BTreeMap<String, Value>,
) {
    let name = model.descriptor.name;
    let response = name.to_owned();
    let page = format!("{name}Page");
    let create = format!("{name}Create");
    let update = format!("{name}Update");
    schemas.insert(response.clone(), object_schema(model, SchemaRole::Response));
    schemas.insert(page.clone(), page_schema(&response));
    schemas.insert(create.clone(), object_schema(model, SchemaRole::Create));
    schemas.insert(update.clone(), object_schema(model, SchemaRole::Update));
    let collection = model.descriptor.api_path.trim_end_matches('/');
    let detail = format!("{collection}/{{id}}");
    let tag = name.to_owned();
    match model.access {
        ModelAccess::Denied | ModelAccess::PolicyUnbound => {
            for method in ["get", "post"] {
                insert_path(
                    paths,
                    collection,
                    method,
                    denied_operation(app, &tag, model.access, method, false),
                );
            }
            for method in ["get", "patch", "delete"] {
                insert_path(
                    paths,
                    &detail,
                    method,
                    denied_operation(app, &tag, model.access, method, true),
                );
            }
        }
        access => {
            insert_path(
                paths,
                collection,
                "get",
                read_operation(app, &tag, access, "get", Some(&page), 200, false),
            );
            insert_path(
                paths,
                collection,
                "post",
                write_operation(
                    app,
                    &tag,
                    access,
                    "post",
                    Some(&create),
                    Some(&response),
                    201,
                ),
            );
            insert_path(
                paths,
                &detail,
                "get",
                read_operation(app, &tag, access, "get", Some(&response), 200, true),
            );
            insert_path(
                paths,
                &detail,
                "patch",
                write_operation(
                    app,
                    &tag,
                    access,
                    "patch",
                    Some(&update),
                    Some(&response),
                    200,
                ),
            );
            insert_path(
                paths,
                &detail,
                "delete",
                write_operation(app, &tag, access, "delete", None, None, 204),
            );
        }
    }
}

#[derive(Clone, Copy)]
enum SchemaRole {
    Response,
    Create,
    Update,
}

fn object_schema(model: &DocumentedModel, role: SchemaRole) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for column in model.columns {
        let contract = contract_for(model, column.name);
        let include = match role {
            SchemaRole::Response => true,
            SchemaRole::Create => contract
                .map(|item| item.in_create)
                .unwrap_or(!column.primary_key),
            SchemaRole::Update => contract
                .map(|item| item.in_update)
                .unwrap_or(!column.primary_key),
        };
        if !include {
            continue;
        }
        if matches!(role, SchemaRole::Response) && !column.nullable {
            required.push(column.name);
        }
        if matches!(role, SchemaRole::Create)
            && contract
                .map(|item| item.create_required)
                .unwrap_or(!column.nullable && column.default_bool.is_none() && !column.primary_key)
        {
            required.push(column.name);
        }
        properties.insert(
            column.name.to_owned(),
            column_schema(model, column, contract, matches!(role, SchemaRole::Update)),
        );
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": Value::Object(properties)
    })
}

fn column_schema(
    model: &DocumentedModel,
    column: &ColumnSpec,
    contract: Option<&FieldContract>,
    patch: bool,
) -> Value {
    let nullable = column.nullable || patch;
    let mut schema = match column.kind {
        ColumnKind::Uuid => typed("string", nullable, Some("uuid")),
        ColumnKind::Text => typed("string", nullable, None),
        ColumnKind::BigInt => typed("integer", nullable, None),
        ColumnKind::Boolean => typed("boolean", nullable, None),
        ColumnKind::Date => typed("string", nullable, Some("date")),
        ColumnKind::Time => typed("string", nullable, Some("time")),
        ColumnKind::DateTime => typed("string", nullable, Some("date-time")),
        ColumnKind::Decimal => {
            let mut schema = typed("string", nullable, None);
            schema["pattern"] = json!(r"^-?\d+(\.\d+)?$");
            schema["x-ferrioxid-decimal"] = json!({
                "precision": column.precision,
                "scale": column.scale
            });
            schema
        }
        ColumnKind::Json => {
            let mut schema = typed("string", nullable, None);
            schema["contentMediaType"] = json!("application/json");
            schema["description"] = json!(
                "Canonical JSON text. PostgreSQL stores jsonb and SQLite stores text. JSON path operators are not part of this schema."
            );
            schema
        }
        ColumnKind::Choice => {
            let mut values: Vec<Value> = column
                .choices
                .iter()
                .map(|choice| Value::String(choice.value.to_owned()))
                .collect();
            if nullable {
                values.push(Value::Null);
            }
            let mut schema = typed("string", nullable, None);
            schema["enum"] = Value::Array(values);
            schema
        }
    };
    if let Some(contract) = contract {
        if let Some(min_length) = contract.min_length {
            schema["minLength"] = json!(min_length);
        }
        if let Some(max_length) = contract.max_length {
            schema["maxLength"] = json!(max_length);
        }
        if contract.format == Some("email") {
            schema["format"] = json!("email");
        } else if contract.format == Some("slug") {
            schema["x-ferrioxid-format"] = json!("slug");
        }
    }
    if let Some(bound) = model
        .bounds
        .iter()
        .find(|bound| bound.column == column.name)
    {
        if column.kind == ColumnKind::BigInt {
            if let Some(min) = bound.min {
                schema["minimum"] = json!(min);
            }
            if let Some(max) = bound.max {
                schema["maximum"] = json!(max);
            }
        } else if column.kind == ColumnKind::Decimal {
            if let Some(min) = bound.min {
                schema["x-ferrioxid-minimum"] = json!(min);
            }
            if let Some(max) = bound.max {
                schema["x-ferrioxid-maximum"] = json!(max);
            }
        }
    }
    if let Some(default) = column.default_bool {
        schema["default"] = json!(default);
    }
    if let (Some(table), Some(referenced)) = (column.foreign_table, column.foreign_column) {
        schema["x-ferrioxid-relation"] = json!({
            "table": table,
            "column": referenced,
            "onDelete": on_delete_name(column.on_delete)
        });
    }
    if let Some(field) = admin_field(model, column.name) {
        if field.filterable {
            schema["x-ferrioxid-filterable"] = json!(true);
            schema["description"] = json!(
                "Filterable in the admin metadata. The generated list route does not accept this name as a query parameter."
            );
        }
        if field.sortable {
            schema["x-ferrioxid-sortable"] = json!(true);
        }
        if field.readonly {
            schema["readOnly"] = json!(
                matches!(
                    contract,
                    Some(item) if !item.in_update
                ) || contract.is_none() && column.primary_key
            );
        }
    }
    schema
}

fn page_schema(item_ref: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["data", "limit", "truncated"],
        "properties": {
            "data": {
                "type": "array",
                "items": { "$ref": format!("#/components/schemas/{item_ref}") }
            },
            "limit": { "type": "integer", "const": PAGE_LIMIT },
            "truncated": { "type": "boolean" }
        },
        "description": "The generated list keeps at most 100 rows. It does not accept offset, cursor, filter, or sort query parameters."
    })
}

fn problem_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "code", "title", "status", "detail", "params"],
        "properties": {
            "type": { "type": "string" },
            "code": { "type": "string" },
            "title": { "type": "string" },
            "status": { "type": "integer" },
            "detail": { "type": "string" },
            "params": { "type": "object", "additionalProperties": { "type": "string" } },
            "field": { "type": "string" }
        }
    })
}

fn read_operation(
    app: &App,
    tag: &str,
    access: ModelAccess,
    method: &str,
    schema_name: Option<&str>,
    status: u16,
    by_id: bool,
) -> Value {
    let mut operation = base_operation(app, tag, method, access, false, by_id);
    if by_id {
        operation["parameters"] = json!([id_parameter()]);
    }
    operation["responses"] = responses(app, access, false, status, schema_name, by_id);
    operation
}

fn write_operation(
    app: &App,
    tag: &str,
    access: ModelAccess,
    method: &str,
    request: Option<&str>,
    response: Option<&str>,
    status: u16,
) -> Value {
    let mut operation = base_operation(app, tag, method, access, true, method != "post");
    if method != "post" {
        operation["parameters"] = json!([id_parameter()]);
    }
    if let Some(request) = request {
        operation["requestBody"] = json!({
            "required": true,
            "content": {
                "application/json": {
                    "schema": { "$ref": format!("#/components/schemas/{request}") }
                }
            }
        });
    }
    operation["responses"] = responses(app, access, true, status, response, method != "post");
    operation
}

fn denied_operation(app: &App, tag: &str, access: ModelAccess, method: &str, by_id: bool) -> Value {
    let mut operation = base_operation(app, tag, method, access, false, by_id);
    if by_id {
        operation["parameters"] = json!([id_parameter()]);
    }
    operation["responses"] = json!({
        "403": problem_response()
    });
    operation
}

fn base_operation(
    app: &App,
    tag: &str,
    method: &str,
    access: ModelAccess,
    write: bool,
    by_id: bool,
) -> Value {
    let operation_id = if by_id {
        format!("{method}_{tag}_by_id")
    } else {
        format!("{method}_{tag}")
    };
    let summary = match access {
        ModelAccess::Anonymous => format!("{method} {tag}"),
        ModelAccess::Permission(name) => {
            format!(
                "{method} {tag}; permission {name}.{}",
                method_action(method)
            )
        }
        ModelAccess::Policy => format!("{method} {tag}; object policy and session"),
        ModelAccess::Denied => format!("{method} {tag}; model is registered without access"),
        ModelAccess::PolicyUnbound => format!("{method} {tag}; object policy is not bound"),
    };
    let mut operation = json!({
        "operationId": operation_id,
        "tags": [tag],
        "summary": summary,
        "security": security_for(app, access, write)
    });
    if write && !matches!(access, ModelAccess::Denied | ModelAccess::PolicyUnbound) {
        operation["description"] = json!(
            "A request that sends ferrioxid_session or ferrioxid_anonymous must also send x-csrf-token or the form field csrf_token. Bearer authorization is not a session."
        );
    }
    operation
}

fn responses(
    app: &App,
    access: ModelAccess,
    write: bool,
    success: u16,
    schema_name: Option<&str>,
    missing_is_404: bool,
) -> Value {
    let mut body = Map::new();
    let success_key = success.to_string();
    body.insert(success_key, success_response(success, schema_name));
    if !matches!(access, ModelAccess::Anonymous) {
        body.insert("401".to_owned(), problem_response());
    }
    body.insert("403".to_owned(), problem_response());
    if missing_is_404 && !matches!(access, ModelAccess::Denied | ModelAccess::PolicyUnbound) {
        body.insert("404".to_owned(), problem_response());
    }
    if write || missing_is_404 {
        body.insert("422".to_owned(), problem_response());
    }
    body.insert("500".to_owned(), problem_response());
    if app.browser_required && write && matches!(access, ModelAccess::Anonymous) {
        body.insert("403".to_owned(), problem_response());
    }
    Value::Object(body)
}

fn success_response(status: u16, schema_name: Option<&str>) -> Value {
    if status == 204 || schema_name.is_none() {
        return json!({ "description": "No response body." });
    }
    json!({
        "description": "JSON document.",
        "content": {
            "application/json": {
                "schema": { "$ref": format!("#/components/schemas/{}", schema_name.unwrap_or_default()) }
            }
        }
    })
}

fn problem_response() -> Value {
    json!({
        "description": "Stable problem JSON. The status, code, title and detail fields are always present.",
        "content": {
            "application/problem+json": {
                "schema": { "$ref": "#/components/schemas/Problem" }
            }
        }
    })
}

fn id_parameter() -> Value {
    json!({
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string", "format": "uuid" }
    })
}

fn security_for(app: &App, access: ModelAccess, write: bool) -> Value {
    match access {
        ModelAccess::Anonymous if write && app.browser_required => json!([
            { "sessionCookie": [], "csrfHeader": [] },
            { "anonymousCookie": [], "csrfHeader": [] }
        ]),
        ModelAccess::Anonymous if write => json!([
            {},
            { "sessionCookie": [], "csrfHeader": [] },
            { "anonymousCookie": [], "csrfHeader": [] }
        ]),
        ModelAccess::Anonymous => json!([{}]),
        ModelAccess::Permission(_) | ModelAccess::Policy if write => {
            json!([{ "sessionCookie": [], "csrfHeader": [] }])
        }
        ModelAccess::Permission(_) | ModelAccess::Policy => json!([{ "sessionCookie": [] }]),
        ModelAccess::Denied | ModelAccess::PolicyUnbound => json!([]),
    }
}

fn health_operation() -> Value {
    json!({
        "operationId": "get_healthz",
        "tags": ["health"],
        "summary": "Process is running. This does not check the database.",
        "responses": {
            "200": {
                "description": "Process is running.",
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["status"],
                            "properties": { "status": { "const": "ok" } }
                        }
                    }
                }
            }
        }
    })
}

fn auth_operations(path: &str) -> Vec<(&'static str, Value)> {
    match path {
        "/auth/login" => vec![(
            "post",
            auth_json("post_auth_login", "Create a session", true, 200, true),
        )],
        "/auth/logout" => vec![(
            "post",
            auth_json("post_auth_logout", "Revoke the session", false, 204, true),
        )],
        "/auth/me" => vec![(
            "get",
            auth_json("get_auth_me", "Read the signed-in user", false, 200, false),
        )],
        "/auth/csrf" => vec![(
            "get",
            auth_json(
                "get_auth_csrf",
                "Read the session CSRF token",
                false,
                200,
                false,
            ),
        )],
        "/auth/anonymous" => vec![(
            "get",
            auth_json(
                "get_auth_anonymous",
                "Open an anonymous session",
                false,
                200,
                false,
            ),
        )],
        "/auth/change-password" => vec![(
            "post",
            auth_json(
                "post_auth_change_password",
                "Replace the password",
                true,
                204,
                true,
            ),
        )],
        "/auth/password-reset" => vec![
            (
                "get",
                auth_html("get_auth_password_reset", "Show the reset request form"),
            ),
            (
                "post",
                auth_html("post_auth_password_reset", "Submit the reset request form"),
            ),
        ],
        "/auth/password-reset/confirm" => {
            vec![(
                "get",
                auth_html("get_auth_password_reset_confirm", "Confirm a reset token"),
            )]
        }
        "/auth/password-reset/choose" => vec![
            (
                "get",
                auth_html(
                    "get_auth_password_reset_choose",
                    "Show the new-password form",
                ),
            ),
            (
                "post",
                auth_html("post_auth_password_reset_choose", "Store the new password"),
            ),
        ],
        _ => Vec::new(),
    }
}

fn auth_json(id: &str, summary: &str, body: bool, status: u16, csrf: bool) -> Value {
    let mut operation = json!({
        "operationId": id,
        "tags": ["auth"],
        "summary": summary,
        "responses": {
            status.to_string(): if status == 204 {
                json!({ "description": "No response body." })
            } else {
                json!({ "description": "JSON document." })
            },
            "401": problem_response(),
            "403": problem_response(),
            "422": problem_response()
        }
    });
    if body {
        operation["requestBody"] = json!({
            "required": true,
            "content": { "application/json": { "schema": { "type": "object" } } }
        });
    }
    if csrf {
        operation["description"] = json!(
            "Cookie sessions need their CSRF proof. Login uses ferrioxid_login_csrf when an anonymous cookie is also present. Password reset uses ferrioxid_reset_csrf."
        );
        operation["security"] = json!([
            {},
            { "sessionCookie": [], "csrfHeader": [] },
            { "anonymousCookie": [], "csrfHeader": [] }
        ]);
    }
    operation
}

fn auth_html(id: &str, summary: &str) -> Value {
    json!({
        "operationId": id,
        "tags": ["auth"],
        "summary": summary,
        "responses": {
            "200": {
                "description": "HTML form.",
                "content": { "text/html": { "schema": { "type": "string" } } }
            },
            "403": problem_response()
        }
    })
}

fn custom_operation(operation: &OpenApiOperation) -> Value {
    let mut body = json!({
        "operationId": operation.operation_id,
        "tags": [operation.tag],
        "summary": operation.summary,
        "responses": {
            operation.response_status.to_string(): success_response(operation.response_status, None)
        }
    });
    if let Some(schema) = &operation.response_schema {
        body["responses"][operation.response_status.to_string()] = json!({
            "description": "Documented response.",
            "content": { "application/json": { "schema": schema } }
        });
    }
    if let Some(schema) = &operation.request_schema {
        body["requestBody"] = json!({
            "required": true,
            "content": { "application/json": { "schema": schema } }
        });
    }
    body["security"] = if operation.session && operation.csrf {
        json!([{ "sessionCookie": [], "csrfHeader": [] }])
    } else if operation.session {
        json!([{ "sessionCookie": [] }])
    } else if operation.csrf {
        json!([{ "csrfHeader": [] }, {}])
    } else {
        json!([{}])
    };
    body
}

fn security_schemes() -> Value {
    json!({
        "sessionCookie": {
            "type": "apiKey",
            "in": "cookie",
            "name": "ferrioxid_session",
            "description": "Signed browser session. It is not a Bearer token."
        },
        "anonymousCookie": {
            "type": "apiKey",
            "in": "cookie",
            "name": "ferrioxid_anonymous",
            "description": "Anonymous browser session. It does not prove an admin login."
        },
        "csrfHeader": {
            "type": "apiKey",
            "in": "header",
            "name": "x-csrf-token",
            "description": "Required for cookie writes. Form posts may send csrf_token instead."
        }
    })
}

fn contract_for<'a>(model: &'a DocumentedModel, column: &str) -> Option<&'a FieldContract> {
    model
        .contracts
        .iter()
        .find(|contract| contract.column == column)
}

fn admin_field<'a>(model: &'a DocumentedModel, column: &str) -> Option<&'a AdminFieldMeta> {
    model
        .admin
        .and_then(|admin| admin.fields.iter().find(|field| field.name == column))
}

fn on_delete_name(action: Option<ReferentialAction>) -> &'static str {
    match action {
        Some(ReferentialAction::Cascade) => "cascade",
        Some(ReferentialAction::SetNull) => "set_null",
        Some(ReferentialAction::Restrict) | None => "restrict",
    }
}

fn typed(kind: &str, nullable: bool, format: Option<&str>) -> Value {
    let mut schema = if nullable {
        json!({ "type": [kind, "null"] })
    } else {
        json!({ "type": kind })
    };
    if let Some(format) = format {
        schema["format"] = json!(format);
    }
    schema
}

fn method_action(method: &str) -> &'static str {
    match method {
        "post" => "create",
        "patch" | "put" => "update",
        "delete" => "delete",
        _ => "read",
    }
}

fn insert_path(
    paths: &mut BTreeMap<String, Map<String, Value>>,
    path: &str,
    method: &str,
    operation: Value,
) {
    let entry = paths.entry(path.to_owned()).or_default();
    entry.insert(method.to_owned(), operation);
}

fn path_value(paths: BTreeMap<String, Map<String, Value>>) -> Value {
    let mut map = Map::new();
    for (path, methods) in paths {
        map.insert(path, Value::Object(methods));
    }
    Value::Object(map)
}

fn schema_value(schemas: BTreeMap<String, Value>) -> Value {
    let mut map = Map::new();
    for (name, schema) in schemas {
        map.insert(name, schema);
    }
    Value::Object(map)
}

fn validate_operation(
    app: &App,
    operation: &OpenApiOperation,
    ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    let method = operation.method.to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
        return Err(format!(
            "OpenAPI method `{}` is not generated.",
            operation.method
        ));
    }
    validate_path(&operation.path)?;
    if operation.path.starts_with("/admin") {
        return Err("Admin routes are not part of the public OpenAPI document.".into());
    }
    let known = app.paths.contains(&operation.path)
        || operation.path == "/healthz"
        || AUTH_PATHS.contains(&operation.path.as_str());
    if !known {
        return Err(format!(
            "OpenAPI path `{}` is not a registered route.",
            operation.path
        ));
    }
    if operation.operation_id.is_empty()
        || !operation
            .operation_id
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_')
    {
        return Err(format!(
            "OpenAPI operation id `{}` must be an identifier.",
            operation.operation_id
        ));
    }
    if !ids.insert(operation.operation_id.clone()) {
        return Err(format!(
            "OpenAPI operation id `{}` is duplicated.",
            operation.operation_id
        ));
    }
    if !(100..600).contains(&operation.response_status) {
        return Err("OpenAPI response status is outside 100..599.".into());
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') || path.contains("//") || path.contains('?') || path.contains('#') {
        return Err(format!(
            "OpenAPI path `{path}` is not a stable absolute path."
        ));
    }
    if path.split('/').skip(1).any(|segment| {
        segment.is_empty()
            || segment.contains("..")
            || segment.contains('\\')
            || segment.contains(' ')
            || !segment.chars().all(|char| {
                char.is_ascii_alphanumeric()
                    || char == '_'
                    || char == '-'
                    || char == '.'
                    || char == '{'
                    || char == '}'
            })
    }) {
        return Err(format!(
            "OpenAPI path `{path}` contains an unsupported segment."
        ));
    }
    Ok(())
}

fn explorer_page(title: &str, json_path: &str) -> String {
    let visible = escape_html(json_path);
    let title = escape_html(title);
    let script_path = serde_json::to_string(json_path).expect("a path string encodes as JSON");
    include_str!("explorer.html")
        .replace("/*TITLE*/", &title)
        .replace("/*SPEC*/", &visible)
        .replace("/*SPEC_JS*/", &script_path)
}

fn escape_html(value: &str) -> String {
    value
        .chars()
        .map(|char| match char {
            '&' => "&amp;".to_owned(),
            '<' => "&lt;".to_owned(),
            '>' => "&gt;".to_owned(),
            '"' => "&quot;".to_owned(),
            _ => char.to_string(),
        })
        .collect()
}
