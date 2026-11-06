//! Explicit model registration -> generated list, read, create, update and delete routes.
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use ferrum_core::{ColumnSpec, FerrumError};
use ferrum_db::{kind_of, render_schema, DeleteOutcome, ModelRepository};
use ferrum_migrations::history_exists;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde_json::json;

pub struct App {
    db: DatabaseConnection,
    routes: Router<DatabaseConnection>,
    schemas: Vec<(&'static str, &'static [ColumnSpec])>,
}

impl App {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            routes: Router::new(),
            schemas: Vec::new(),
        }
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
        } else {
            // No auth implementation exists: deny access by default on all routes.
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
        self.routes
            .route("/healthz", get(|| async { Json(json!({"status": "ok"})) }))
            .with_state(self.db)
    }
}

struct ApiError(FerrumError);

impl From<FerrumError> for ApiError {
    fn from(error: FerrumError) -> Self {
        Self(error)
    }
}

async fn denied() -> ApiError {
    ApiError(FerrumError::Forbidden)
}

async fn list<M: ModelRepository>(
    State(db): State<DatabaseConnection>,
) -> Result<Json<ferrum_db::ListPage<M>>, ApiError> {
    Ok(Json(M::list(&db).await?))
}

async fn get_one<M: ModelRepository>(
    State(db): State<DatabaseConnection>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<M>, ApiError> {
    let model = M::get(&db, id).await?.ok_or(FerrumError::NotFound)?;
    Ok(Json(model))
}

async fn create<M: ModelRepository>(
    State(db): State<DatabaseConnection>,
    Json(input): Json<M::Create>,
) -> Result<(StatusCode, Json<M>), ApiError> {
    Ok((StatusCode::CREATED, Json(M::create(&db, input).await?)))
}

async fn update<M: ModelRepository>(
    State(db): State<DatabaseConnection>,
    Path(id): Path<uuid::Uuid>,
    Json(input): Json<M::Update>,
) -> Result<Json<M>, ApiError> {
    Ok(Json(M::update(&db, id, input).await?))
}

async fn delete_one<M: ModelRepository>(
    State(db): State<DatabaseConnection>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, ApiError> {
    match M::delete(&db, id).await? {
        DeleteOutcome::Deleted => Ok(StatusCode::NO_CONTENT),
        DeleteOutcome::Missing => Err(FerrumError::NotFound.into()),
    }
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
        (status, Json(json!({"type":"about:blank","title":title,"status":status.as_u16(),"detail":detail}))).into_response()
    }
}
