//! Explizite Modellregistrierung erzeugt Listen-, Lese-, Anlage-, Änderungs- und Löschrouten.
//!
//! `allow_anonymous` bleibt die einzige öffentliche Freigabe. Jede andere Route prüft die
//! Sitzung und die am Modell hinterlegte Berechtigung. Cookie-Schreibzugriffe prüft zusätzlich
//! die Schicht in `cookie_guard`, bevor ein Handler läuft. Gebundene Objektregeln filtern über `QueryScope`.

use std::collections::BTreeSet;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use ferrioxid_admin::AdminRegistry;
use ferrioxid_auth::{mount, AppState, AuthError, AuthService, AuthUser, ClientIp, ObjectPolicy};
use ferrioxid_core::{problem_json, ColumnSpec, FerrioxidError, QueryScope};
use ferrioxid_db::{kind_of, render_schema, DeleteOutcome, ModelRepository};
use ferrioxid_migrations::{history_exists, ProjectConfig};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde_json::json;

mod cookie_guard;
pub mod openapi;
pub mod testing;

pub use cookie_guard::require_browser;
pub use openapi::{OpenApiOperation, OpenApiOptions};

pub mod apps;
pub mod crud;
pub mod flash;
pub mod pages;
pub mod urls;
pub mod views;
pub mod web;

pub struct App {
    db: DatabaseConnection,
    auth: Option<AuthService>,
    mail: Option<ferrioxid_mail::MailService>,
    admin: Option<AdminRegistry>,
    settings: Option<ProjectConfig>,
    names: urls::RouteMap,
    routes: Router<AppState>,
    schemas: Vec<(&'static str, &'static [ColumnSpec])>,
    installed: Vec<String>,
    versions: Vec<(String, String)>,
    sealed: bool,
    paths: BTreeSet<String>,
    tables: BTreeSet<String>,
    migrations: Vec<ferrioxid_migrations::PackageMigration>,
    catalog: apps::TemplateCatalog,
    statics: Vec<apps::StaticMount>,
    browser_required: bool,
    openapi: OpenApiOptions,
    openapi_operations: Vec<OpenApiOperation>,
    documented: Vec<openapi::DocumentedModel>,
    strict_migrations: bool,
}

impl App {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            auth: None,
            mail: None,
            admin: None,
            settings: None,
            names: urls::RouteMap::default(),
            routes: Router::new(),
            schemas: Vec::new(),
            installed: Vec::new(),
            versions: Vec::new(),
            sealed: false,
            paths: BTreeSet::new(),
            tables: BTreeSet::new(),
            migrations: Vec::new(),
            catalog: apps::TemplateCatalog::default(),
            statics: Vec::new(),
            browser_required: false,
            openapi: OpenApiOptions::default(),
            openapi_operations: Vec::new(),
            documented: Vec::new(),
            strict_migrations: false,
        }
    }

    /// Requires [`App::try_into_router_strict`] before the router is built.
    ///
    /// The check only reads migration status. It does not apply or roll back migrations.
    /// [`App::try_into_router`] keeps the previous behavior unless this flag is set.
    pub fn strict_migrations(mut self) -> Self {
        self.strict_migrations = true;
        self
    }

    /// Replaces the OpenAPI JSON path, the optional explorer, or turns both off.
    pub fn openapi(mut self, options: OpenApiOptions) -> Self {
        self.openapi = options;
        self
    }

    pub fn without_openapi(mut self) -> Self {
        self.openapi.enabled = false;
        self
    }

    /// Documents one route that `urls` or `merge` already registered.
    ///
    /// A path that is not registered, including admin paths, is rejected when the
    /// router is built.
    pub fn document(mut self, operation: OpenApiOperation) -> Self {
        self.openapi_operations.push(operation);
        self
    }

    /// POST, PUT, PATCH and DELETE require a browser session cookie and its CSRF proof.
    pub fn require_browser_writes(mut self) -> Self {
        self.browser_required = true;
        self
    }

    pub fn with_auth(mut self, auth: AuthService) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn with_mail(mut self, mail: ferrioxid_mail::MailService) -> Self {
        self.mail = Some(mail);
        self
    }

    pub fn with_settings(mut self, settings: ProjectConfig) -> Self {
        self.settings = Some(settings);
        self
    }

    pub fn route_names(&self) -> &urls::RouteMap {
        &self.names
    }

    /// Hängt benannte Anwendungsrouten an. Reservierte Framework-Pfade werden abgelehnt.
    pub fn urls(mut self, set: urls::UrlSet) -> Result<Self, urls::UrlError> {
        self.accept_urls(set)?;
        Ok(self)
    }

    pub fn with_admin(mut self, registry: AdminRegistry) -> Self {
        self.admin = Some(registry);
        self
    }

    /// Hängt zusätzliche Routen an, bevor der Zustand gebunden wird.
    pub fn merge(mut self, router: Router<AppState>) -> Self {
        self.routes = self.routes.merge(router);
        self
    }

    pub fn register<M: ModelRepository>(mut self) -> Self {
        self.register_in::<M>();
        self
    }

    pub(crate) fn register_in<M: ModelRepository>(&mut self) {
        let descriptor = M::descriptor();
        let detail_path = format!("{}/{{id}}", descriptor.api_path.trim_end_matches('/'));
        if descriptor.allow_anonymous {
            let routes = std::mem::replace(&mut self.routes, Router::new());
            self.routes = routes.route(descriptor.api_path, get(list::<M>).post(create::<M>));
            let routes = std::mem::replace(&mut self.routes, Router::new());
            self.routes = routes.route(
                &detail_path,
                get(get_one::<M>).patch(update::<M>).delete(delete_one::<M>),
            );
        } else if descriptor.requires_object_policy {
            let routes = std::mem::replace(&mut self.routes, Router::new());
            self.routes = routes.route(
                descriptor.api_path,
                get(policy_unbound).post(policy_unbound),
            );
            let routes = std::mem::replace(&mut self.routes, Router::new());
            self.routes = routes.route(
                &detail_path,
                get(policy_unbound)
                    .patch(policy_unbound)
                    .delete(policy_unbound),
            );
        } else if descriptor.permission.is_some() {
            let routes = std::mem::replace(&mut self.routes, Router::new());
            self.routes = routes.route(
                descriptor.api_path,
                get(guarded_list::<M>).post(guarded_create::<M>),
            );
            let routes = std::mem::replace(&mut self.routes, Router::new());
            self.routes = routes.route(
                &detail_path,
                get(guarded_get::<M>)
                    .patch(guarded_update::<M>)
                    .delete(guarded_delete::<M>),
            );
        } else {
            let routes = std::mem::replace(&mut self.routes, Router::new());
            self.routes = routes.route(descriptor.api_path, get(denied).post(denied));
            let routes = std::mem::replace(&mut self.routes, Router::new());
            self.routes = routes.route(&detail_path, get(denied).patch(denied).delete(denied));
        }
        self.remember(descriptor.table, descriptor.api_path, &detail_path);
        self.schemas.push((descriptor.table, M::columns()));
        let access = if descriptor.allow_anonymous {
            openapi::ModelAccess::Anonymous
        } else if descriptor.requires_object_policy {
            openapi::ModelAccess::PolicyUnbound
        } else if let Some(permission) = descriptor.permission {
            openapi::ModelAccess::Permission(permission)
        } else {
            openapi::ModelAccess::Denied
        };
        self.documented
            .push(openapi::DocumentedModel::from_model::<M>(access));
    }

    /// Bindet eine Objektregel. Listen und Schreibzugriffe verwenden denselben Query-Scope.
    ///
    /// `register_policy` ersetzt `register` für dasselbe Modell. Beide Aufrufe erzeugen doppelte Routen.
    pub fn register_policy<M, P>(mut self) -> Self
    where
        M: ModelRepository,
        P: ObjectPolicy<M> + 'static,
    {
        self.register_policy_in::<M, P>();
        self
    }

    pub(crate) fn register_policy_in<M, P>(&mut self)
    where
        M: ModelRepository,
        P: ObjectPolicy<M> + 'static,
    {
        let descriptor = M::descriptor();
        let detail_path = format!("{}/{{id}}", descriptor.api_path.trim_end_matches('/'));
        let routes = std::mem::replace(&mut self.routes, Router::new());
        self.routes = routes.route(
            descriptor.api_path,
            get(policy_list::<M, P>).post(policy_create::<M, P>),
        );
        let routes = std::mem::replace(&mut self.routes, Router::new());
        self.routes = routes.route(
            &detail_path,
            get(policy_get::<M, P>)
                .patch(policy_update::<M, P>)
                .delete(policy_delete::<M, P>),
        );
        self.remember(descriptor.table, descriptor.api_path, &detail_path);
        self.schemas.push((descriptor.table, M::columns()));
        self.documented
            .push(openapi::DocumentedModel::from_model::<M>(
                openapi::ModelAccess::Policy,
            ));
    }

    fn remember(&mut self, table: &str, path: &str, detail: &str) {
        self.tables.insert(table.to_owned());
        self.paths.insert(path.to_owned());
        self.paths.insert(detail.to_owned());
    }

    fn accept_urls(&mut self, set: urls::UrlSet) -> Result<(), urls::UrlError> {
        let (router, names) = set.into_parts();
        for (_name, path) in names.iter() {
            if self.paths.contains(path) {
                return Err(urls::UrlError {
                    code: "url.duplicate",
                    detail: format!("Der Pfad `{path}` ist bereits vergeben."),
                });
            }
        }
        self.names.merge(names)?;
        for (_name, path) in self.names.iter() {
            self.paths.insert(path.to_owned());
        }
        let routes = std::mem::replace(&mut self.routes, Router::new());
        self.routes = routes.merge(router);
        Ok(())
    }

    /// Legacy-Helfer für leere Testdatenbanken. Keine Migrationshistorie und kein Schema-Diff.
    ///
    /// Existiert `_ferrioxid_migrations`, wird nichts verändert.
    pub async fn bootstrap_schema(&self) -> Result<(), FerrioxidError> {
        if history_exists(&self.db)
            .await
            .map_err(FerrioxidError::database)?
        {
            return Err(FerrioxidError::Validation(
                "migration history exists; bootstrap will not create or relabel tables".into(),
            ));
        }
        let backend = self.db.get_database_backend();
        let kind = kind_of(backend)?;
        for (table, columns) in &self.schemas {
            let sql = render_schema(kind, table, columns);
            self.db
                .execute(Statement::from_string(backend, sql))
                .await
                .map_err(FerrioxidError::database)?;
        }
        Ok(())
    }

    pub fn try_into_router(self) -> Result<Router, crate::apps::AppError> {
        if self.strict_migrations {
            return Err(crate::apps::AppError::new(
                "migration.strict",
                "strict_migrations() was set. Call try_into_router_strict().await. Nothing was migrated.",
            ));
        }
        self.build_router()
    }

    /// Reads migration status, then builds the router. A pending, unknown or conflicting
    /// migration rejects startup before the first request. The database is not changed.
    pub async fn try_into_router_strict(self) -> Result<Router, crate::apps::AppError> {
        self.migration_readiness().await?;
        self.build_router()
    }

    fn build_router(self) -> Result<Router, crate::apps::AppError> {
        let needs_validation = !self.sealed
            && (!self.installed.is_empty()
                || self
                    .settings
                    .as_ref()
                    .is_some_and(|settings| !settings.apps.is_empty()));
        let app = if needs_validation { self.seal()? } else { self };
        openapi::check(&app)
            .map_err(|detail| crate::apps::AppError::new("openapi.path", detail))?;
        Ok(app.finish_router())
    }

    pub fn into_router(self) -> Router {
        match self.try_into_router() {
            Ok(router) => router,
            Err(error) => panic!(
                "Ferrioxid-App-Registry ist ungültig ({}). {}",
                error.code, error.detail
            ),
        }
    }

    async fn migration_readiness(&self) -> Result<(), crate::apps::AppError> {
        let app_dir = self
            .settings
            .as_ref()
            .map(|settings| settings.migrations_dir.clone())
            .unwrap_or_else(|| std::path::PathBuf::from("ferrioxid-no-app-migrations"));
        let report = ferrioxid_migrations::inspect_migrations_with_packages(
            &self.db,
            &ferrioxid_auth::migrations_dir(),
            &app_dir,
            None,
            &self.migrations,
        )
        .await
        .map_err(|error| crate::apps::AppError::new("migration.strict", error.0))?;
        let mut problems = Vec::new();
        collect_migration_problems("auth", &report.auth, &mut problems);
        collect_migration_problems("app", &report.app, &mut problems);
        for package in &report.packages {
            collect_migration_problems(&package.name, &package.status, &mut problems);
        }
        for diagnostic in &report.diagnostics {
            if diagnostic.level == ferrioxid_migrations::Level::Error
                || diagnostic.description.contains("differs")
                || diagnostic.description.contains("neither")
            {
                problems.push(format!(
                    "{} {}: {}",
                    diagnostic.code, diagnostic.migration_id, diagnostic.description
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(crate::apps::AppError::new(
                "migration.strict",
                format!(
                    "Startup refused because migrations are not ready. Apply the listed migrations manually. This check did not change the database.\n{}",
                    problems.join("\n")
                ),
            ))
        }
    }

    fn finish_router(self) -> Router {
        let specification = self.openapi.enabled.then(|| openapi::specification(&self));
        let openapi_options = self.openapi.clone();
        let state = AppState {
            db: self.db,
            auth: self.auth,
            mail: self.mail,
        };
        let router = {
            let mut routes = mount(self.routes);
            if let Some(registry) = self.admin {
                routes = ferrioxid_admin::mount(routes, registry);
            }
            routes
        }
        .route("/healthz", get(|| async { Json(json!({"status": "ok"})) }))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            cookie_guard::enforce,
        ));
        let router = if self.browser_required {
            router.layer(axum::middleware::from_fn_with_state(
                state.clone(),
                cookie_guard::require_browser,
            ))
        } else {
            router
        };
        let router = router.with_state(state);
        let mut router = router;
        for mount in self.statics {
            let mut options = ferrioxid_static::StaticOptions::new(mount.directory);
            options.url_prefix = mount.prefix;
            router = ferrioxid_static::mount(router, options);
        }
        let mut catalog = self.catalog;
        catalog.set_routes(self.names.clone());
        let router = if let Some(specification) = specification {
            openapi::mount(router, specification, &openapi_options)
        } else {
            router
        };
        let router = router.layer(axum::Extension(std::sync::Arc::new(catalog)));
        let router = router.layer(axum::Extension(std::sync::Arc::new(self.names)));
        if let Some(settings) = self.settings {
            router.layer(axum::Extension(std::sync::Arc::new(settings)))
        } else {
            router
        }
    }
}

fn collect_migration_problems(
    app: &str,
    status: &ferrioxid_migrations::StatusReport,
    problems: &mut Vec<String>,
) {
    for line in &status.lines {
        if line.state == "ausstehend" {
            problems.push(format!("app `{app}` migration `{}` is pending", line.id));
        }
    }
    for conflict in &status.conflicts {
        problems.push(format!("app `{app}`: {conflict}"));
    }
    if status.schema_note.contains("differs") || status.schema_note.contains("neither") {
        problems.push(format!("app `{app}`: {}", status.schema_note));
    }
}

struct ApiError(FerrioxidError);

impl From<FerrioxidError> for ApiError {
    fn from(error: FerrioxidError) -> Self {
        Self(error)
    }
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self(FerrioxidError::from(error))
    }
}

async fn denied() -> ApiError {
    ApiError(FerrioxidError::Forbidden)
}

async fn policy_unbound() -> ApiError {
    ApiError(FerrioxidError::Denied(
        "object policy is not bound; call register_policy".into(),
    ))
}

async fn list<M: ModelRepository>(
    State(state): State<AppState>,
) -> Result<Json<ferrioxid_db::ListPage<M>>, ApiError> {
    Ok(Json(M::list(&state.db).await?))
}

async fn get_one<M: ModelRepository>(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<M>, ApiError> {
    let model = M::get(&state.db, id)
        .await?
        .ok_or(FerrioxidError::NotFound)?;
    Ok(Json(model))
}

async fn create<M: ModelRepository>(
    State(state): State<AppState>,
    Json(input): Json<M::Create>,
) -> Result<(StatusCode, Json<M>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(M::create(&state.db, input).await?),
    ))
}

async fn update<M: ModelRepository>(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(input): Json<M::Update>,
) -> Result<Json<M>, ApiError> {
    Ok(Json(M::update(&state.db, id, input).await?))
}

async fn delete_one<M: ModelRepository>(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, ApiError> {
    match M::delete(&state.db, id).await? {
        DeleteOutcome::Deleted => Ok(StatusCode::NO_CONTENT),
        DeleteOutcome::Missing => Err(FerrioxidError::NotFound.into()),
    }
}

async fn guarded_list<M: ModelRepository>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
) -> Result<Json<ferrioxid_db::ListPage<M>>, ApiError> {
    ensure::<M>(&state, &headers, &Method::GET, &peer.0, "read").await?;
    Ok(Json(M::list(&state.db).await?))
}

async fn guarded_get<M: ModelRepository>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<M>, ApiError> {
    ensure::<M>(&state, &headers, &Method::GET, &peer.0, "read").await?;
    let model = M::get(&state.db, id)
        .await?
        .ok_or(FerrioxidError::NotFound)?;
    Ok(Json(model))
}

async fn guarded_create<M: ModelRepository>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(input): Json<M::Create>,
) -> Result<(StatusCode, Json<M>), ApiError> {
    ensure::<M>(&state, &headers, &Method::POST, &peer.0, "create").await?;
    Ok((
        StatusCode::CREATED,
        Json(M::create(&state.db, input).await?),
    ))
}

async fn guarded_update<M: ModelRepository>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Path(id): Path<uuid::Uuid>,
    Json(input): Json<M::Update>,
) -> Result<Json<M>, ApiError> {
    ensure::<M>(&state, &headers, &Method::PATCH, &peer.0, "update").await?;
    Ok(Json(M::update(&state.db, id, input).await?))
}

async fn guarded_delete<M: ModelRepository>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure::<M>(&state, &headers, &Method::DELETE, &peer.0, "delete").await?;
    match M::delete(&state.db, id).await? {
        DeleteOutcome::Deleted => Ok(StatusCode::NO_CONTENT),
        DeleteOutcome::Missing => Err(FerrioxidError::NotFound.into()),
    }
}

async fn policy_create<M, P>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Json(input): Json<M::Create>,
) -> Result<(StatusCode, Json<M>), ApiError>
where
    M: ModelRepository,
    P: ObjectPolicy<M>,
{
    let user = ensure::<M>(&state, &headers, &Method::POST, &peer.0, "create").await?;
    if !allows(user.is_superuser, P::can_create(&user, &input)) {
        return Err(FerrioxidError::Denied("create policy rejected the record".into()).into());
    }
    Ok((
        StatusCode::CREATED,
        Json(M::create(&state.db, input).await?),
    ))
}

async fn policy_list<M, P>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
) -> Result<Json<ferrioxid_db::ListPage<M>>, ApiError>
where
    M: ModelRepository,
    P: ObjectPolicy<M>,
{
    let user = ensure::<M>(&state, &headers, &Method::GET, &peer.0, "read").await?;
    let scope = actor_scope::<M, P>(&user, false);
    Ok(Json(M::list_scoped(&state.db, &scope).await?))
}

async fn policy_get<M, P>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<M>, ApiError>
where
    M: ModelRepository,
    P: ObjectPolicy<M>,
{
    let user = ensure::<M>(&state, &headers, &Method::GET, &peer.0, "read").await?;
    let scope = actor_scope::<M, P>(&user, false);
    let model = M::get_scoped(&state.db, id, &scope)
        .await?
        .ok_or(FerrioxidError::NotFound)?;
    if !allows(user.is_superuser, P::can_read(&user, &model)) {
        return Err(FerrioxidError::NotFound.into());
    }
    Ok(Json(model))
}

async fn policy_update<M, P>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Path(id): Path<uuid::Uuid>,
    Json(input): Json<M::Update>,
) -> Result<Json<M>, ApiError>
where
    M: ModelRepository,
    P: ObjectPolicy<M>,
{
    let user = ensure::<M>(&state, &headers, &Method::PATCH, &peer.0, "update").await?;
    let scope = actor_scope::<M, P>(&user, true);
    let current = M::get_scoped(&state.db, id, &scope)
        .await?
        .ok_or(FerrioxidError::NotFound)?;
    if !allows(user.is_superuser, P::can_update(&user, &current)) {
        return Err(FerrioxidError::NotFound.into());
    }
    Ok(Json(M::update_scoped(&state.db, id, input, &scope).await?))
}

async fn policy_delete<M, P>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, ApiError>
where
    M: ModelRepository,
    P: ObjectPolicy<M>,
{
    let user = ensure::<M>(&state, &headers, &Method::DELETE, &peer.0, "delete").await?;
    let scope = actor_scope::<M, P>(&user, true);
    let current = M::get_scoped(&state.db, id, &scope)
        .await?
        .ok_or(FerrioxidError::NotFound)?;
    if !allows(user.is_superuser, P::can_delete(&user, &current)) {
        return Err(FerrioxidError::NotFound.into());
    }
    match M::delete_scoped(&state.db, id, &scope).await? {
        DeleteOutcome::Deleted => Ok(StatusCode::NO_CONTENT),
        DeleteOutcome::Missing => Err(FerrioxidError::NotFound.into()),
    }
}

fn actor_scope<M, P>(user: &AuthUser, write: bool) -> QueryScope
where
    M: ModelRepository,
    P: ObjectPolicy<M>,
{
    if user.is_superuser {
        QueryScope::All
    } else if write {
        P::write_scope(user)
    } else {
        P::read_scope(user)
    }
}

fn allows(superuser: bool, permitted: bool) -> bool {
    superuser || permitted
}

async fn ensure<M: ModelRepository>(
    state: &AppState,
    headers: &HeaderMap,
    method: &Method,
    _peer: &str,
    action: &str,
) -> Result<AuthUser, ApiError> {
    let auth = state
        .auth
        .as_ref()
        .ok_or_else(|| FerrioxidError::Denied("authentication is not configured".into()))?;
    let user = auth.authenticate(headers, method).await?;
    auth.authorize(&user, M::descriptor().permission, action)
        .await?;
    Ok(user)
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, title, detail) = match &self.0 {
            FerrioxidError::Validation(message) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Validation failed",
                message.as_str(),
            ),
            FerrioxidError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden", "Access denied"),
            FerrioxidError::Denied(message) => {
                (StatusCode::FORBIDDEN, "Forbidden", message.as_str())
            }
            FerrioxidError::Unauthorized(message) => {
                (StatusCode::UNAUTHORIZED, "Unauthorized", message.as_str())
            }
            FerrioxidError::NotFound => (StatusCode::NOT_FOUND, "Not found", "Resource not found"),
            FerrioxidError::Database(error) => {
                tracing::error!(error = %error, "database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error",
                    "Database operation failed",
                )
            }
        };
        (status, Json(problem_json(status.as_u16(), title, detail))).into_response()
    }
}
