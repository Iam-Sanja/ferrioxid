//! Registrierte Admin-Seiten. Der Pfad kommt aus der Registry, nie aus einem freien URL-Teil.

use crate::registry::AdminRegistry;
use fexu_auth::AuthUser;
use sea_orm::DatabaseConnection;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub const RESERVED_KEYS: &[&str] = &[
    "admin",
    "api",
    "assets",
    "audit",
    "directory",
    "login",
    "logout",
    "models",
    "pages",
    "preferences",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionError {
    pub code: &'static str,
    pub detail: &'static str,
}

impl ExtensionError {
    pub const fn new(code: &'static str, detail: &'static str) -> Self {
        Self { code, detail }
    }
}

impl std::fmt::Display for ExtensionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ExtensionError {}

/// A file field from a multipart admin form. The bytes are not stored by the framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedFile {
    pub field: String,
    pub filename: String,
    /// Browser-supplied Content-Type. Do not use it as proof of the file contents.
    pub declared_type: Option<String>,
    pub bytes: Vec<u8>,
}

impl UploadedFile {
    pub fn size(&self) -> usize {
        self.bytes.len()
    }
}

pub struct ViewRequest<'a> {
    pub user: &'a AuthUser,
    pub db: &'a DatabaseConnection,
    pub locale: &'a str,
    pub form: &'a HashMap<String, String>,
    /// Empty for GET and for URL-encoded forms.
    pub uploads: &'a [UploadedFile],
}

#[derive(Debug, Clone)]
pub struct ViewPage {
    pub data: Map<String, Value>,
    pub notice: String,
    pub alert: String,
}

impl ViewPage {
    pub fn new(data: Map<String, Value>) -> Self {
        Self {
            data,
            notice: String::new(),
            alert: String::new(),
        }
    }
}

#[async_trait::async_trait]
pub trait ViewHandler: Send + Sync {
    async fn handle(&self, request: &ViewRequest<'_>) -> Result<ViewPage, ExtensionError>;
}

#[derive(Clone)]
pub struct AdminView {
    key: &'static str,
    title_de: &'static str,
    title_en: &'static str,
    permission: &'static str,
    navigation: bool,
    template: &'static str,
    get: Option<Arc<dyn ViewHandler>>,
    post: Option<Arc<dyn ViewHandler>>,
}

impl AdminView {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            title_de: key,
            title_en: key,
            permission: "",
            navigation: false,
            template: "custom.html",
            get: None,
            post: None,
        }
    }

    pub fn titles(mut self, german: &'static str, english: &'static str) -> Self {
        self.title_de = german;
        self.title_en = english;
        self
    }

    /// Vollständiger Code, zum Beispiel `orders.read`. Fexu prüft ihn vor dem Handler.
    pub fn permission(mut self, code: &'static str) -> Self {
        self.permission = code;
        self
    }

    pub fn navigation(mut self, enabled: bool) -> Self {
        self.navigation = enabled;
        self
    }

    /// Name einer registrierten Vorlage. Kein Dateipfad.
    pub fn template(mut self, name: &'static str) -> Self {
        self.template = name;
        self
    }

    pub fn on_get(mut self, handler: impl ViewHandler + 'static) -> Self {
        self.get = Some(Arc::new(handler));
        self
    }

    pub fn on_post(mut self, handler: impl ViewHandler + 'static) -> Self {
        self.post = Some(Arc::new(handler));
        self
    }

    pub(crate) fn key(&self) -> &'static str {
        self.key
    }

    pub(crate) fn title(&self, locale: &str) -> &'static str {
        if locale == "de" {
            self.title_de
        } else {
            self.title_en
        }
    }

    pub(crate) fn permission_code(&self) -> &'static str {
        self.permission
    }

    pub(crate) fn in_navigation(&self) -> bool {
        self.navigation
    }

    pub(crate) fn template_name(&self) -> &'static str {
        self.template
    }

    pub(crate) fn get_handler(&self) -> Option<&Arc<dyn ViewHandler>> {
        self.get.as_ref()
    }

    pub(crate) fn post_handler(&self) -> Option<&Arc<dyn ViewHandler>> {
        self.post.as_ref()
    }
}

pub(crate) fn valid_key(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && value.len() <= 40
}

pub(crate) fn valid_permission(value: &str) -> bool {
    let Some((resource, action)) = value.split_once('.') else {
        return false;
    };
    valid_key(resource) && valid_key(action) && !value.contains("..")
}

pub(crate) fn valid_label(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 80
        && !trimmed
            .chars()
            .any(|char| char.is_control() || matches!(char, '<' | '>' | '"' | '\''))
}

pub(crate) fn check_view(view: &AdminView, templates: &[&str]) -> Result<(), ExtensionError> {
    if !valid_key(view.key) || RESERVED_KEYS.contains(&view.key) {
        return Err(ExtensionError::new(
            "extension.route",
            "Der Seitenname ist leer, reserviert oder enthält unerlaubte Zeichen.",
        ));
    }
    if !valid_permission(view.permission) {
        return Err(ExtensionError::new(
            "extension.permission",
            "Eine eigene Seite braucht einen Berechtigungscode resource.action.",
        ));
    }
    if !valid_label(view.title_de) || !valid_label(view.title_en) {
        return Err(ExtensionError::new(
            "extension.route",
            "Der Seitentitel ist leer oder enthält Steuerzeichen.",
        ));
    }
    if view.get.is_none() {
        return Err(ExtensionError::new(
            "extension.route",
            "Eine eigene Seite braucht einen GET-Handler.",
        ));
    }
    if !templates.contains(&view.template) && view.template != "custom.html" {
        return Err(ExtensionError::new(
            "extension.template",
            "Die Seitenvorlage ist nicht registriert.",
        ));
    }
    Ok(())
}

impl AdminRegistry {
    pub(crate) fn view_by_key(&self, key: &str) -> Option<&AdminView> {
        self.views().iter().find(|view| view.key() == key)
    }
}

#[cfg(test)]
mod tests {
    use super::{AdminView, ExtensionError};
    use crate::registry::AdminRegistry;

    #[test]
    fn reserved_routes_and_unsafe_templates_fail() {
        let reserved = AdminRegistry::new().view(AdminView::new("models"));
        assert!(matches!(
            reserved,
            Err(ExtensionError {
                code: "extension.route",
                ..
            })
        ));
        let traversal =
            AdminRegistry::new().template("../base.html", "{% extends \"base.html\" %}");
        assert!(traversal.is_err());
        let unescaped = AdminRegistry::new().template(
            "reports.html",
            "{% extends \"base.html\" %}{{ body | safe }}",
        );
        assert!(unescaped.is_err());
    }
}
