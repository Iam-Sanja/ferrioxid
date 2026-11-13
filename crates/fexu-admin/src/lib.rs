//! Admin-API und serverseitige HTML-Oberfläche.
//!
//! Datenzugriff, Validierung und Sitzungen bleiben in den vorhandenen Fexu-Crates.

mod actions;
mod assets;
mod audit;
mod compose;
mod configure;
mod directory;
mod drafts;
mod filters;
mod html;
mod http;
mod localization;
mod nav;
mod pages;
mod registry;
mod relations;
mod rendering;
mod views;
mod widgets;

pub use actions::{ActionContext, ActionError, ActionHandler, ActionOperation, AdminAction};
#[doc(hidden)]
pub use audit::fail_next_insert;
pub use compose::{groups as inline_form_groups, save as save_with_children, SaveResult};
pub use configure::{CheckedModelAdmin, ConfigError, ManyLink, ModelAdmin};
pub use filters::{AdminFilter, FilterKind, FilterOption};
pub use registry::{AdminBranding, AdminRegistry};
pub use relations::{
    bulk_targets, current_members, lookup_page, posted_members, replace_links, scoped_membership,
    shape_of, LookupPage, RelationShape,
};
pub use views::{AdminView, ExtensionError, UploadedFile, ViewHandler, ViewPage, ViewRequest};
pub use widgets::{AdminWidget, WidgetKind};

pub fn mount(
    router: axum::Router<fexu_auth::AppState>,
    registry: AdminRegistry,
) -> axum::Router<fexu_auth::AppState> {
    http::mount(router, registry)
}
