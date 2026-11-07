//! Admin-API und serverseitige HTML-Oberfläche.
//!
//! Datenzugriff, Validierung und Sitzungen bleiben in den vorhandenen Ferrum-Crates.

mod actions;
mod assets;
mod audit;
mod compose;
mod directory;
mod drafts;
mod filters;
mod html;
mod http;
mod localization;
mod nav;
mod pages;
mod registry;
mod rendering;
mod views;
mod widgets;

pub use actions::{ActionContext, ActionError, ActionHandler, ActionOperation, AdminAction};
#[doc(hidden)]
pub use audit::fail_next_insert;
pub use compose::{groups as inline_form_groups, save as save_with_children, SaveResult};
pub use filters::{AdminFilter, FilterKind, FilterOption};
pub use registry::{AdminBranding, AdminRegistry};
pub use views::{AdminView, ExtensionError, ViewHandler, ViewPage, ViewRequest};
pub use widgets::{AdminWidget, WidgetKind};

pub fn mount(
    router: axum::Router<ferrum_auth::AppState>,
    registry: AdminRegistry,
) -> axum::Router<ferrum_auth::AppState> {
    http::mount(router, registry)
}
