//! Statische Dateien und Standard-Middleware über die Projekteinstellungen.

use axum::Router;
use fexu_middleware::{apply, Standard};
use fexu_migrations::ProjectConfig;
use fexu_static::{mount, StaticOptions};

pub fn prepare(router: Router, settings: &ProjectConfig) -> Router {
    let router = if settings.static_enabled {
        let mut options = StaticOptions::new(settings.static_root());
        options.url_prefix = settings.static_url_prefix.clone();
        options.cache_seconds = settings.static_cache_seconds;
        mount(router, options)
    } else {
        router
    };
    let limit = usize::try_from(settings.max_upload_bytes()).unwrap_or(2 * 1024 * 1024);
    apply(
        router,
        Standard {
            body_limit: limit.max(1024),
            ..Standard::default()
        },
    )
}
