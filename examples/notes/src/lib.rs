//! Wiederverwendbare Notiz-App. Sie hängt nur an der öffentlichen Fexu-API.

pub mod jobs;

use axum::routing::get;
use fexu::apps::{AppConfig, AppError, FexuApplication, Registration};
use fexu::urls::{UrlError, UrlSet};

pub struct NotesApp;

impl FexuApplication for NotesApp {
    fn config(&self) -> AppConfig {
        AppConfig::new("notes").label("Notizen").prefix("/notes")
    }

    fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), AppError> {
        registry.routes(routes()?)
    }

    fn register_jobs(&self, registry: &mut fexu::jobs::JobRegistry) -> Result<(), AppError> {
        jobs::register(registry, jobs::NotePulseHandler::new())
            .map_err(|error| AppError::new("job.handler", error.detail().to_owned()))?;
        jobs::register(registry, jobs::NoteRetryHandler)
            .map_err(|error| AppError::new("job.handler", error.detail().to_owned()))
    }
}

pub fn routes() -> Result<UrlSet, UrlError> {
    UrlSet::new().route("notes.list", "/items", get(list))
}

async fn list() -> axum::response::Response {
    fexu::views::json(&fexu::serde_json::json!({"app": "notes"}))
}
