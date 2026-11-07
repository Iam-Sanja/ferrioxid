//! Admin-API und serverseitige HTML-Oberfläche.
//!
//! Datenzugriff, Validierung und Sitzungen bleiben in den vorhandenen Ferrum-Crates.

mod actions;
mod assets;
mod audit;
mod drafts;
mod html;
mod http;
mod localization;
mod registry;
mod rendering;

pub use actions::{ActionOperation, AdminAction};
pub use registry::{AdminBranding, AdminRegistry};

pub fn mount(
    router: axum::Router<ferrum_auth::AppState>,
    registry: AdminRegistry,
) -> axum::Router<ferrum_auth::AppState> {
    http::mount(router, registry)
}
