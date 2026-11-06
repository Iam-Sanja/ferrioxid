//! Admin-API und Auslieferung der gebauten Oberfläche.
//!
//! Datenzugriff, Validierung und Sitzungen bleiben in den vorhandenen Ferrum-Crates.

mod assets;
mod http;
mod registry;

pub use registry::AdminRegistry;

pub fn mount(
    router: axum::Router<ferrum_auth::AppState>,
    registry: AdminRegistry,
) -> axum::Router<ferrum_auth::AppState> {
    http::mount(router, registry)
}
