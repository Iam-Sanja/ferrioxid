//! Explizite Modellregistrierung erzeugt Listen-, Lese-, Anlage-, Änderungs- und Löschrouten.
//!
//! `allow_anonymous` bleibt die einzige öffentliche Freigabe. Jede andere Route prüft die
//! Sitzung und die am Modell hinterlegte Berechtigung. Gebundene Objektregeln filtern über `QueryScope`.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use ferrum_admin::AdminRegistry;
use ferrum_auth::{mount, AppState, AuthError, AuthService, AuthUser, ClientIp, ObjectPolicy};
use ferrum_core::{problem_json, ColumnSpec, FerrumError, QueryScope};
use ferrum_db::{kind_of, render_schema, DeleteOutcome, ModelRepository};
use ferrum_migrations::{history_exists, ProjectConfig};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde_json::json;

pub mod pages;
pub mod urls;
pub mod views;
pub mod web;

pub struct App {
    db: DatabaseConnection,
    auth: Option<AuthService>,
    mail: Option<ferrum_mail::MailService>,
    admin: Option<AdminRegistry>,
    settings: Option<ProjectConfig>,
    names: urls::RouteMap,
    routes: Router<AppState>,
    schemas: Vec<(&'static str, &'static [ColumnSpec])>,
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
        }
    }

    pub fn with_auth(mut self, auth: AuthService) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn with_mail(mut self, mail: ferrum_mail::MailService) -> Self {
        self.mail = Some(mail);
        self
    }

    pub fn with_settings(mut self, settings: ProjectConfig) -> Self {
        self.settings = Some(settings);
        self
    }

    /// Hängt benannte Anwendungsrouten an. Reservierte Framework-Pfade werden abgelehnt.
    pub fn urls(mut self, set: urls::UrlSet) -> Result<Self, urls::UrlError> {
        let (router, names) = set.into_parts();
        self.names.merge(names)?;
        self.routes = self.routes.merge(router);
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
        let descriptor = M::descriptor();
        let detail_path = format!("{}/{{id}}", descriptor.api_path.trim_end_matches('/'));
        if descriptor.allow_anonymous {
            self.routes = self
                .routes
                .route(descriptor.api_path, get(list::<M>).post(create::<M>));
            self.routes = self.routes.route(
                &detail_path,
                get(get_one::<M>).patch(update::<M>).delete(delete_one::<M>),
            );
        } else if descriptor.requires_object_policy {
            self.routes = self.routes.route(
                descriptor.api_path,
                get(policy_unbound).post(policy_unbound),
            );
            self.routes = self.routes.route(
                &detail_path,
                get(policy_unbound)
                    .patch(policy_unbound)
                    .delete(policy_unbound),
            );
        } else if descriptor.permission.is_some() {
            self.routes = self.routes.route(
                descriptor.api_path,
                get(guarded_list::<M>).post(guarded_create::<M>),
            );
            self.routes = self.routes.route(
                &detail_path,
                get(guarded_get::<M>)
                    .patch(guarded_update::<M>)
                    .delete(guarded_delete::<M>),
            );
        } else {
            self.routes = self
                .routes
                .route(descriptor.api_path, get(denied).post(denied));
            self.routes = self
                .routes
                .route(&detail_path, get(denied).patch(denied).delete(denied));
        }
        self.schemas.push((descriptor.table, M::columns()));
        self
    }

    /// Bindet eine Objektregel. Listen und Schreibzugriffe verwenden denselben Query-Scope.
    ///
    /// `register_policy` ersetzt `register` für dasselbe Modell. Beide Aufrufe erzeugen doppelte Routen.
    pub fn register_policy<M, P>(mut self) -> Self
    where
        M: ModelRepository,
        P: ObjectPolicy<M> + 'static,
    {
        let descriptor = M::descriptor();
        let detail_path = format!("{}/{{id}}", descriptor.api_path.trim_end_matches('/'));
        self.routes = self.routes.route(
            descriptor.api_path,
            get(policy_list::<M, P>).post(policy_create::<M, P>),
        );
        self.routes = self.routes.route(
            &detail_path,
            get(policy_get::<M, P>)
                .patch(policy_update::<M, P>)
                .delete(policy_delete::<M, P>),
        );
        self.schemas.push((descriptor.table, M::columns()));
        self
    }

    /// Legacy-Helfer für leere Testdatenbanken. Keine Migrationshistorie und kein Schema-Diff.
    ///
    /// Existiert `_ferrum_migrations`, wird nichts verändert.
    pub async fn bootstrap_schema(&self) -> Result<(), FerrumError> {
        if history_exists(&self.db)
            .await
            .map_err(FerrumError::database)?
        {
            return Err(FerrumError::Validation(
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
                .map_err(FerrumError::database)?;
        }
        Ok(())
    }

    pub fn into_router(self) -> Router {
        let state = AppState {
            db: self.db,
            auth: self.auth,
            mail: self.mail,
        };
        let router = {
            let mut routes = mount(self.routes);
            if let Some(registry) = self.admin {
                routes = ferrum_admin::mount(routes, registry);
            }
            routes
        }
        .route("/healthz", get(|| async { Json(json!({"status": "ok"})) }))
        .with_state(state);
        let router = router.layer(axum::Extension(std::sync::Arc::new(self.names)));
        if let Some(settings) = self.settings {
            router.layer(axum::Extension(std::sync::Arc::new(settings)))
        } else {
            router
        }
    }
}

struct ApiError(FerrumError);

impl From<FerrumError> for ApiError {
    fn from(error: FerrumError) -> Self {
        Self(error)
    }
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self(FerrumError::from(error))
    }
}

async fn denied() -> ApiError {
    ApiError(FerrumError::Forbidden)
}

async fn policy_unbound() -> ApiError {
    ApiError(FerrumError::Denied(
        "object policy is not bound; call register_policy".into(),
    ))
}

async fn list<M: ModelRepository>(
    State(state): State<AppState>,
) -> Result<Json<ferrum_db::ListPage<M>>, ApiError> {
    Ok(Json(M::list(&state.db).await?))
}

async fn get_one<M: ModelRepository>(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<M>, ApiError> {
    let model = M::get(&state.db, id).await?.ok_or(FerrumError::NotFound)?;
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
        DeleteOutcome::Missing => Err(FerrumError::NotFound.into()),
    }
}

async fn guarded_list<M: ModelRepository>(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: ClientIp,
) -> Result<Json<ferrum_db::ListPage<M>>, ApiError> {
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
    let model = M::get(&state.db, id).await?.ok_or(FerrumError::NotFound)?;
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
        DeleteOutcome::Missing => Err(FerrumError::NotFound.into()),
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
        return Err(FerrumError::Denied("create policy rejected the record".into()).into());
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
) -> Result<Json<ferrum_db::ListPage<M>>, ApiError>
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
        .ok_or(FerrumError::NotFound)?;
    if !allows(user.is_superuser, P::can_read(&user, &model)) {
        return Err(FerrumError::NotFound.into());
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
        .ok_or(FerrumError::NotFound)?;
    if !allows(user.is_superuser, P::can_update(&user, &current)) {
        return Err(FerrumError::NotFound.into());
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
        .ok_or(FerrumError::NotFound)?;
    if !allows(user.is_superuser, P::can_delete(&user, &current)) {
        return Err(FerrumError::NotFound.into());
    }
    match M::delete_scoped(&state.db, id, &scope).await? {
        DeleteOutcome::Deleted => Ok(StatusCode::NO_CONTENT),
        DeleteOutcome::Missing => Err(FerrumError::NotFound.into()),
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
        .ok_or_else(|| FerrumError::Denied("authentication is not configured".into()))?;
    let user = auth.authenticate(headers, method).await?;
    auth.authorize(&user, M::descriptor().permission, action)
        .await?;
    Ok(user)
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, title, detail) = match &self.0 {
            FerrumError::Validation(message) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Validation failed",
                message.as_str(),
            ),
            FerrumError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden", "Access denied"),
            FerrumError::Denied(message) => (StatusCode::FORBIDDEN, "Forbidden", message.as_str()),
            FerrumError::Unauthorized(message) => {
                (StatusCode::UNAUTHORIZED, "Unauthorized", message.as_str())
            }
            FerrumError::NotFound => (StatusCode::NOT_FOUND, "Not found", "Resource not found"),
            FerrumError::Database(error) => {
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
