//! Explizite Admin-Registry. Nur Modelle mit `admin = true` werden aufgenommen.

use crate::actions::{self, AdminAction};
use crate::audit::{self, Event};
use crate::filters::{self, AdminFilter};
use crate::views::{self, AdminView, ExtensionError};
use crate::widgets::{self, AdminWidget};
use ferrioxid_auth::{AuthUser, ObjectPolicy};
use ferrioxid_core::{AdminModelMeta, ColumnSpec, FerrioxidError, ModelDescriptor, QueryScope};
use ferrioxid_db::{query_admin_page, AdminPageRequest, DeleteOutcome, ModelRepository};
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use serde_json::{Map, Value};
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
#[async_trait::async_trait]
pub trait ManagedModel: Send + Sync {
    fn meta(&self) -> &'static AdminModelMeta;
    fn descriptor(&self) -> &'static ModelDescriptor;
    fn columns(&self) -> &'static [ColumnSpec];
    fn policy_bound(&self) -> bool;
    fn read_scope(&self, user: &AuthUser) -> QueryScope;
    fn write_scope(&self, user: &AuthUser) -> QueryScope;
    async fn page(
        &self,
        db: &DatabaseConnection,
        search_columns: &[&str],
        search: Option<&str>,
        sort_column: &str,
        descending: bool,
        filters: &[(&str, sea_orm::Value)],
        bounds: &[(&str, bool, sea_orm::Value)],
        offset: u64,
        limit: u64,
        scope: &QueryScope,
    ) -> Result<(Vec<Value>, bool, i64), FerrioxidError>;
    async fn read(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<Option<Value>, FerrioxidError>;
    async fn create(
        &self,
        db: &DatabaseConnection,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError>;
    async fn update(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError>;
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrioxidError>;
    fn object_allows(&self, user: &AuthUser, action: &str, record: &Value) -> bool;
    async fn read_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<Option<Value>, FerrioxidError>;
    async fn create_in(
        &self,
        tx: &DatabaseTransaction,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError>;
    async fn update_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError>;
    async fn delete_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrioxidError>;
}

#[derive(Clone, Debug)]
pub struct AdminBranding {
    pub site_title: String,
    pub site_header: String,
    pub app_name: String,
    pub primary_color: String,
    pub default_locale: String,
    pub fallback_locale: String,
    pub currency: String,
}

impl Default for AdminBranding {
    fn default() -> Self {
        Self {
            site_title: "Ferrioxid Admin".to_owned(),
            site_header: "Ferrioxid Verwaltung".to_owned(),
            app_name: "Ferrioxid".to_owned(),
            primary_color: "#0f172a".to_owned(),
            default_locale: "de".to_owned(),
            fallback_locale: "en".to_owned(),
            currency: "EUR".to_owned(),
        }
    }
}

impl AdminBranding {
    /// Verwirft Farben, Sprachen und Währungen, die nicht dem öffentlichen Vertrag entsprechen.
    pub fn sanitized(mut self) -> Self {
        let fallback = Self::default();
        if !is_public_label(&self.site_title) {
            self.site_title = fallback.site_title;
        }
        if !is_public_label(&self.site_header) {
            self.site_header = fallback.site_header;
        }
        if !is_public_label(&self.app_name) {
            self.app_name = fallback.app_name;
        }
        if !is_hex_color(&self.primary_color) {
            self.primary_color = fallback.primary_color;
        }
        if self.default_locale != "de" && self.default_locale != "en" {
            self.default_locale = fallback.default_locale;
        }
        if self.fallback_locale != "de" && self.fallback_locale != "en" {
            self.fallback_locale = fallback.fallback_locale;
        }
        if self.currency.len() != 3 || !self.currency.chars().all(|char| char.is_ascii_uppercase())
        {
            self.currency = fallback.currency;
        }
        self
    }
}

pub(crate) fn valid_template_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".html") else {
        return false;
    };
    crate::views::valid_key(stem)
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
}

fn is_public_label(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 80
        && !trimmed
            .chars()
            .any(|char| char.is_control() || char == '<' || char == '>')
}

fn is_hex_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 3 | 6) && hex.chars().all(|char| char.is_ascii_hexdigit())
}

#[derive(Clone, Default)]
pub struct AdminRegistry {
    models: Arc<Vec<Arc<dyn ManagedModel>>>,
    branding: AdminBranding,
    actions: Vec<AdminAction>,
    views: Vec<AdminView>,
    filters: Vec<AdminFilter>,
    widgets: Vec<AdminWidget>,
    templates: Vec<(&'static str, &'static str)>,
    sealed: bool,
    apps: Vec<(String, String)>,
    model_apps: Vec<(String, String)>,
    view_apps: Vec<(String, String)>,
    pending_app: Option<String>,
}

impl AdminRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_branding(mut self, branding: AdminBranding) -> Self {
        self.branding = branding.sanitized();
        self
    }

    pub fn branding(&self) -> &AdminBranding {
        &self.branding
    }

    /// Ordnet genau die nächste Modell- oder Seitenregistrierung dieser App zu.
    /// Für weitere Registrierungen `with_app` erneut aufrufen.
    pub fn with_app(mut self, key: impl Into<String>, label: impl Into<String>) -> Self {
        let key = key.into();
        let label = label.into().trim().to_owned();
        let mut chars = key.chars();
        let key_ok = matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
            && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
            && key.len() <= 40;
        let label_ok =
            !label.is_empty() && label.len() <= 80 && !label.chars().any(|char| char.is_control());
        self.pending_app = None;
        if key_ok && label_ok {
            if !self.apps.iter().any(|(name, _)| name == &key) {
                self.apps.push((key.clone(), label));
            }
            self.pending_app = Some(key);
        }
        self
    }

    fn take_app_scope(&mut self) -> Option<String> {
        self.pending_app.take()
    }

    pub(crate) fn app_catalog(&self) -> &[(String, String)] {
        &self.apps
    }

    pub(crate) fn model_app(&self, key: &str) -> Option<&str> {
        self.model_apps
            .iter()
            .find(|(model, _)| model == key)
            .map(|(_, app)| app.as_str())
    }

    pub(crate) fn view_app(&self, key: &str) -> Option<&str> {
        self.view_apps
            .iter()
            .find(|(view, _)| view == key)
            .map(|(_, app)| app.as_str())
    }

    pub fn add<M>(mut self) -> Self
    where
        M: ModelRepository + 'static,
    {
        if M::admin_meta().is_none() {
            self.pending_app = None;
            return self;
        }
        self.push(Arc::new(Plain::<M>::new()))
    }

    pub fn add_policy<M, P>(mut self) -> Self
    where
        M: ModelRepository + 'static,
        P: ObjectPolicy<M> + Send + Sync + 'static,
    {
        if M::admin_meta().is_none() {
            self.pending_app = None;
            return self;
        }
        self.push(Arc::new(Policed::<M, P>::new()))
    }

    pub fn action(mut self, action: AdminAction) -> Self {
        if !actions::valid_name(action.name)
            || matches!(action.name, "delete" | "activate" | "deactivate")
            || self.actions.iter().any(|existing| {
                existing.model_key == action.model_key && existing.name == action.name
            })
        {
            return self;
        }
        self.sealed = false;
        self.actions.push(action);
        self
    }

    pub fn try_action(mut self, action: AdminAction) -> Result<Self, ExtensionError> {
        actions::check_custom(&action)?;
        if self
            .actions
            .iter()
            .any(|existing| existing.model_key == action.model_key && existing.name == action.name)
        {
            return Err(ExtensionError::new(
                "extension.duplicate",
                "Die Aktion ist für dieses Modell schon registriert.",
            ));
        }
        self.sealed = false;
        self.actions.push(action);
        Ok(self)
    }

    pub fn view(mut self, view: AdminView) -> Result<Self, ExtensionError> {
        if !views::valid_key(view.key()) || views::RESERVED_KEYS.contains(&view.key()) {
            return Err(ExtensionError::new(
                "extension.route",
                "Der Seitenname ist leer, reserviert oder enthält unerlaubte Zeichen.",
            ));
        }
        if self
            .views
            .iter()
            .any(|existing| existing.key() == view.key())
        {
            return Err(ExtensionError::new(
                "extension.duplicate",
                "Die Seite ist schon registriert.",
            ));
        }
        self.sealed = false;
        let key = view.key().to_owned();
        self.views.push(view);
        if let Some(app) = self.take_app_scope() {
            if !self.view_apps.iter().any(|(view, _)| view == &key) {
                self.view_apps.push((key, app));
            }
        }
        Ok(self)
    }

    pub fn filter(mut self, filter: AdminFilter) -> Result<Self, ExtensionError> {
        filters::check_filter(&filter, None, None)?;
        if self
            .filters
            .iter()
            .any(|existing| existing.model_key == filter.model_key && existing.name == filter.name)
        {
            return Err(ExtensionError::new(
                "extension.duplicate",
                "Der Filter ist für dieses Modell schon registriert.",
            ));
        }
        self.sealed = false;
        self.filters.push(filter);
        Ok(self)
    }

    pub fn widget(mut self, widget: AdminWidget) -> Result<Self, ExtensionError> {
        widgets::check_widget(&widget, None, None)?;
        if self.widgets.iter().any(|existing| {
            existing.model_key == widget.model_key && existing.field == widget.field
        }) {
            return Err(ExtensionError::new(
                "extension.duplicate",
                "Das Feld hat schon ein Widget.",
            ));
        }
        self.sealed = false;
        self.widgets.push(widget);
        Ok(self)
    }

    pub fn template(
        mut self,
        name: &'static str,
        source: &'static str,
    ) -> Result<Self, ExtensionError> {
        if !valid_template_name(name) || crate::rendering::embedded(name) {
            return Err(ExtensionError::new(
                "extension.template",
                "Der Vorlagenname ist reserviert oder kein einfacher HTML-Name.",
            ));
        }
        if source.len() > 65_536
            || source.contains("|safe")
            || source.contains("| safe")
            || source.contains("{% include")
            || source.contains("{% import")
            || !source.contains("{% extends \"base.html\" %}")
        {
            return Err(ExtensionError::new(
                "extension.template",
                "Die Vorlage muss base.html erweitern und bleibt escaped.",
            ));
        }
        if self.templates.iter().any(|(existing, _)| *existing == name) {
            return Err(ExtensionError::new(
                "extension.duplicate",
                "Die Vorlage ist schon registriert.",
            ));
        }
        self.sealed = false;
        self.templates.push((name, source));
        Ok(self)
    }

    /// Prüft Erweiterungen gegen die registrierten Modelle. Danach ist die Registry startbereit.
    pub fn seal(mut self) -> Result<Self, ExtensionError> {
        let names: Vec<&str> = self.templates.iter().map(|(name, _)| *name).collect();
        for view in &self.views {
            views::check_view(view, &names)?;
        }
        for filter in &self.filters {
            let model = self
                .models
                .iter()
                .find(|model| model.meta().key == filter.model_key);
            let Some(model) = model else {
                return Err(ExtensionError::new(
                    "extension.filter",
                    "Der Filter gehört zu keinem registrierten Modell.",
                ));
            };
            filters::check_filter(filter, Some(model.meta()), Some(model.columns()))?;
        }
        for widget in &self.widgets {
            let model = self
                .models
                .iter()
                .find(|model| model.meta().key == widget.model_key);
            let Some(model) = model else {
                return Err(ExtensionError::new(
                    "extension.widget",
                    "Das Widget gehört zu keinem registrierten Modell.",
                ));
            };
            widgets::check_widget(widget, Some(model.meta()), Some(model.columns()))?;
        }
        for action in &self.actions {
            if !matches!(action.operation, crate::actions::ActionOperation::Custom(_)) {
                continue;
            }
            actions::check_custom(action)?;
            let model = self
                .models
                .iter()
                .find(|model| model.meta().key == action.model_key);
            let Some(model) = model else {
                return Err(ExtensionError::new(
                    "extension.action",
                    "Die Aktion gehört zu keinem registrierten Modell.",
                ));
            };
            let crate::actions::ActionOperation::Custom(custom) = &action.operation else {
                continue;
            };
            if custom
                .columns
                .iter()
                .any(|column| !actions::writable(model.meta(), model.columns(), column))
            {
                return Err(ExtensionError::new(
                    "extension.action",
                    "Eine Schreibspalte ist verborgen, sensibel oder keine Modellspalte.",
                ));
            }
        }
        self.sealed = true;
        Ok(self)
    }

    pub(crate) fn views(&self) -> &[AdminView] {
        &self.views
    }

    pub(crate) fn filters(&self) -> &[AdminFilter] {
        &self.filters
    }

    pub(crate) fn widgets(&self) -> &[AdminWidget] {
        &self.widgets
    }

    pub(crate) fn templates(&self) -> &[(&'static str, &'static str)] {
        &self.templates
    }

    pub(crate) fn sealed(&self) -> bool {
        self.sealed
            || (self.views.is_empty()
                && self.filters.is_empty()
                && self.widgets.is_empty()
                && self.templates.is_empty()
                && self.actions.iter().all(|action| {
                    !matches!(action.operation, crate::actions::ActionOperation::Custom(_))
                }))
    }

    #[allow(dead_code)]
    pub(crate) fn nav(
        &self,
        locale: &str,
        path: &str,
        allowed: &[(&str, bool)],
    ) -> Vec<serde_json::Value> {
        self.views
            .iter()
            .filter(|view| view.in_navigation())
            .filter(|view| {
                allowed
                    .iter()
                    .any(|(code, allowed)| *code == view.permission_code() && *allowed)
            })
            .map(|view| {
                let href = format!("/admin/pages/{}", view.key());
                serde_json::json!({
                    "href": href,
                    "label": view.title(locale),
                    "current": path == format!("/admin/pages/{}", view.key())
                })
            })
            .collect()
    }

    pub fn actions_for(&self, meta: &AdminModelMeta, columns: &[ColumnSpec]) -> Vec<AdminAction> {
        actions::for_model(meta, columns, &self.actions)
    }

    pub fn models(&self) -> &[Arc<dyn ManagedModel>] {
        &self.models
    }

    pub fn find(&self, key: &str) -> Option<&Arc<dyn ManagedModel>> {
        self.models.iter().find(|model| model.meta().key == key)
    }

    fn push(mut self, model: Arc<dyn ManagedModel>) -> Self {
        let app_scope = self.take_app_scope();
        if model.meta().key.is_empty() {
            return self;
        }
        let key = model.meta().key.to_owned();
        let models = Arc::make_mut(&mut self.models);
        if models.iter().any(|existing| existing.meta().key == key) {
            return self;
        }
        models.push(model);
        if let Some(app) = app_scope {
            if !self.model_apps.iter().any(|(model, _)| model == &key) {
                self.model_apps.push((key, app));
            }
        }
        self
    }
}

struct Plain<M>(std::marker::PhantomData<M>);

impl<M> Plain<M> {
    fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

struct Policed<M, P>(std::marker::PhantomData<(M, P)>);

impl<M, P> Policed<M, P> {
    fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

#[async_trait::async_trait]
impl<M> ManagedModel for Plain<M>
where
    M: ModelRepository + 'static,
{
    fn meta(&self) -> &'static AdminModelMeta {
        M::admin_meta().expect("admin registration requires admin = true")
    }
    fn descriptor(&self) -> &'static ModelDescriptor {
        M::descriptor()
    }
    fn columns(&self) -> &'static [ColumnSpec] {
        M::columns()
    }
    fn policy_bound(&self) -> bool {
        false
    }
    fn read_scope(&self, _user: &AuthUser) -> QueryScope {
        QueryScope::All
    }
    fn write_scope(&self, user: &AuthUser) -> QueryScope {
        self.read_scope(user)
    }
    async fn page(
        &self,
        db: &DatabaseConnection,
        search_columns: &[&str],
        search: Option<&str>,
        sort_column: &str,
        descending: bool,
        filters: &[(&str, sea_orm::Value)],
        bounds: &[(&str, bool, sea_orm::Value)],
        offset: u64,
        limit: u64,
        scope: &QueryScope,
    ) -> Result<(Vec<Value>, bool, i64), FerrioxidError> {
        load_page::<M>(
            db,
            search_columns,
            search,
            sort_column,
            descending,
            filters,
            bounds,
            offset,
            limit,
            scope,
        )
        .await
    }
    async fn read(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<Option<Value>, FerrioxidError> {
        Ok(M::get_scoped(db, id, scope).await?.map(json_of))
    }
    async fn create(
        &self,
        db: &DatabaseConnection,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrioxidError::Validation(format!("request body is invalid: {error}"))
        })?;
        audited_create::<M>(db, user, input).await
    }
    async fn update(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrioxidError::Validation(format!("request body is invalid: {error}"))
        })?;
        audited_update::<M>(db, user, id, input, scope).await
    }
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrioxidError> {
        audited_delete::<M>(db, user, id, scope).await
    }
    fn object_allows(&self, _user: &AuthUser, _action: &str, _record: &Value) -> bool {
        true
    }
    async fn read_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<Option<Value>, FerrioxidError> {
        Ok(M::get_tx(tx, id, scope).await?.map(json_of))
    }
    async fn create_in(
        &self,
        tx: &DatabaseTransaction,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError> {
        let _ = user;
        let input = serde_json::from_value(body).map_err(|error| {
            FerrioxidError::Validation(format!("request body is invalid: {error}"))
        })?;
        Ok(json_of(M::create_tx(tx, input).await?))
    }
    async fn update_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError> {
        let _ = user;
        let input = serde_json::from_value(body).map_err(|error| {
            FerrioxidError::Validation(format!("request body is invalid: {error}"))
        })?;
        if M::get_tx(tx, id, scope).await?.is_none() {
            return Err(FerrioxidError::NotFound);
        }
        Ok(json_of(M::update_tx(tx, id, input, scope).await?))
    }
    async fn delete_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrioxidError> {
        let _ = user;
        if M::get_tx(tx, id, scope).await?.is_none() {
            return Err(FerrioxidError::NotFound);
        }
        M::delete_tx(tx, id, scope).await
    }
}

#[async_trait::async_trait]
impl<M, P> ManagedModel for Policed<M, P>
where
    M: ModelRepository + 'static,
    P: ObjectPolicy<M> + Send + Sync + 'static,
{
    fn meta(&self) -> &'static AdminModelMeta {
        M::admin_meta().expect("admin registration requires admin = true")
    }
    fn descriptor(&self) -> &'static ModelDescriptor {
        M::descriptor()
    }
    fn columns(&self) -> &'static [ColumnSpec] {
        M::columns()
    }
    fn policy_bound(&self) -> bool {
        true
    }
    fn read_scope(&self, user: &AuthUser) -> QueryScope {
        P::read_scope(user)
    }
    fn write_scope(&self, user: &AuthUser) -> QueryScope {
        P::write_scope(user)
    }
    async fn page(
        &self,
        db: &DatabaseConnection,
        search_columns: &[&str],
        search: Option<&str>,
        sort_column: &str,
        descending: bool,
        filters: &[(&str, sea_orm::Value)],
        bounds: &[(&str, bool, sea_orm::Value)],
        offset: u64,
        limit: u64,
        scope: &QueryScope,
    ) -> Result<(Vec<Value>, bool, i64), FerrioxidError> {
        load_page::<M>(
            db,
            search_columns,
            search,
            sort_column,
            descending,
            filters,
            bounds,
            offset,
            limit,
            scope,
        )
        .await
    }
    async fn read(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<Option<Value>, FerrioxidError> {
        Ok(M::get_scoped(db, id, scope).await?.map(json_of))
    }
    async fn create(
        &self,
        db: &DatabaseConnection,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError> {
        let input = serde_json::from_value::<M::Create>(body).map_err(|error| {
            FerrioxidError::Validation(format!("request body is invalid: {error}"))
        })?;
        if !user.is_superuser && !P::can_create(user, &input) {
            return Err(FerrioxidError::Denied(
                "create policy rejected the record".into(),
            ));
        }
        audited_create::<M>(db, user, input).await
    }
    async fn update(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrioxidError::Validation(format!("request body is invalid: {error}"))
        })?;
        audited_update::<M>(db, user, id, input, scope).await
    }
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrioxidError> {
        audited_delete::<M>(db, user, id, scope).await
    }
    fn object_allows(&self, user: &AuthUser, action: &str, record: &Value) -> bool {
        let Ok(model) = serde_json::from_value::<M>(record.clone()) else {
            return false;
        };
        match action {
            "read" => P::can_read(user, &model),
            "update" => P::can_update(user, &model),
            "delete" => P::can_delete(user, &model),
            _ => false,
        }
    }
    async fn read_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<Option<Value>, FerrioxidError> {
        Ok(M::get_tx(tx, id, scope).await?.map(json_of))
    }
    async fn create_in(
        &self,
        tx: &DatabaseTransaction,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError> {
        let input = serde_json::from_value::<M::Create>(body).map_err(|error| {
            FerrioxidError::Validation(format!("request body is invalid: {error}"))
        })?;
        if !user.is_superuser && !P::can_create(user, &input) {
            return Err(FerrioxidError::Denied(
                "create policy rejected the record".into(),
            ));
        }
        Ok(json_of(M::create_tx(tx, input).await?))
    }
    async fn update_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<Value, FerrioxidError> {
        let existing = M::get_tx(tx, id, scope)
            .await?
            .ok_or(FerrioxidError::NotFound)?;
        if !user.is_superuser && !P::can_update(user, &existing) {
            return Err(FerrioxidError::Denied(
                "update policy rejected the record".into(),
            ));
        }
        let input = serde_json::from_value(body).map_err(|error| {
            FerrioxidError::Validation(format!("request body is invalid: {error}"))
        })?;
        Ok(json_of(M::update_tx(tx, id, input, scope).await?))
    }
    async fn delete_in(
        &self,
        tx: &DatabaseTransaction,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrioxidError> {
        let existing = M::get_tx(tx, id, scope)
            .await?
            .ok_or(FerrioxidError::NotFound)?;
        if !user.is_superuser && !P::can_delete(user, &existing) {
            return Err(FerrioxidError::Denied(
                "delete policy rejected the record".into(),
            ));
        }
        M::delete_tx(tx, id, scope).await
    }
}

#[allow(clippy::too_many_arguments)]
async fn load_page<M: ModelRepository>(
    db: &DatabaseConnection,
    search_columns: &[&str],
    search: Option<&str>,
    sort_column: &str,
    descending: bool,
    filters: &[(&str, sea_orm::Value)],
    bounds: &[(&str, bool, sea_orm::Value)],
    offset: u64,
    limit: u64,
    scope: &QueryScope,
) -> Result<(Vec<Value>, bool, i64), FerrioxidError> {
    let page = query_admin_page(
        db,
        M::descriptor().table,
        M::columns(),
        AdminPageRequest {
            search_columns,
            search,
            sort_column,
            descending,
            equal_filters: filters,
            bounds,
            offset,
            limit,
            scope,
        },
    )
    .await?;
    Ok((page.rows, page.has_more, page.total))
}

async fn audited_create<M>(
    db: &DatabaseConnection,
    user: &AuthUser,
    input: M::Create,
) -> Result<Value, FerrioxidError>
where
    M: ModelRepository,
{
    let txn = db.begin().await.map_err(FerrioxidError::database)?;
    let created = M::create_tx(&txn, input).await?;
    let value = json_of(&created);
    write_audit::<M>(&txn, user, "create", "ok", &value, None, Some(&value)).await?;
    txn.commit().await.map_err(FerrioxidError::database)?;
    Ok(value)
}

async fn audited_update<M>(
    db: &DatabaseConnection,
    user: &AuthUser,
    id: uuid::Uuid,
    input: M::Update,
    scope: &QueryScope,
) -> Result<Value, FerrioxidError>
where
    M: ModelRepository,
{
    let txn = db.begin().await.map_err(FerrioxidError::database)?;
    let before = M::get_tx(&txn, id, scope).await?;
    let updated = M::update_tx(&txn, id, input, scope).await?;
    let value = json_of(&updated);
    let previous = before.as_ref().map(json_of);
    write_audit::<M>(
        &txn,
        user,
        "update",
        "ok",
        &value,
        previous.as_ref(),
        Some(&value),
    )
    .await?;
    txn.commit().await.map_err(FerrioxidError::database)?;
    Ok(value)
}

async fn audited_delete<M>(
    db: &DatabaseConnection,
    user: &AuthUser,
    id: uuid::Uuid,
    scope: &QueryScope,
) -> Result<DeleteOutcome, FerrioxidError>
where
    M: ModelRepository,
{
    let txn = db.begin().await.map_err(FerrioxidError::database)?;
    let before = M::get_tx(&txn, id, scope).await?;
    let outcome = M::delete_tx(&txn, id, scope).await?;
    if outcome == DeleteOutcome::Deleted {
        if let Some(record) = before {
            let value = json_of(record);
            write_audit::<M>(&txn, user, "delete", "ok", &value, Some(&value), None).await?;
        }
    }
    txn.commit().await.map_err(FerrioxidError::database)?;
    Ok(outcome)
}

async fn write_audit<M: ModelRepository>(
    db: &impl sea_orm::ConnectionTrait,
    user: &AuthUser,
    action: &str,
    outcome: &str,
    record: &Value,
    before: Option<&Value>,
    after: Option<&Value>,
) -> Result<(), FerrioxidError> {
    let summary = M::admin_meta()
        .map(|meta| audit::change_summary(meta, before, after))
        .unwrap_or_default();
    audit::insert(
        db,
        &Event {
            actor_id: user.id,
            model_key: M::admin_meta()
                .map(|meta| meta.key.to_owned())
                .unwrap_or_else(|| M::descriptor().table.to_owned()),
            record_id: record
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            action: action.to_owned(),
            outcome: outcome.to_owned(),
            summary,
            operation_id: None,
        },
    )
    .await
}

fn json_of<M: serde::Serialize>(model: M) -> Value {
    serde_json::to_value(model).unwrap_or(Value::Object(Map::new()))
}

pub fn redact(meta: &AdminModelMeta, value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let hidden: Vec<&str> = meta
        .fields
        .iter()
        .filter(|field| field.hidden || sensitive(field.name))
        .map(|field| field.name)
        .collect();
    for name in hidden {
        object.remove(name);
    }
}

pub fn sensitive(name: &str) -> bool {
    matches!(
        name,
        "password"
            | "password_hash"
            | "token"
            | "token_hash"
            | "secret"
            | "csrf_token"
            | "csrf_token_hash"
    )
}

#[cfg(test)]
mod tests {
    use super::AdminBranding;

    #[test]
    fn branding_rejects_css_and_unknown_locale() {
        let branding = AdminBranding {
            primary_color: "red;background:url(x)".to_owned(),
            currency: "euro".to_owned(),
            default_locale: "../de".to_owned(),
            ..AdminBranding::default()
        }
        .sanitized();
        assert_eq!(branding.primary_color, "#0f172a");
        assert_eq!(branding.currency, "EUR");
        assert_eq!(branding.default_locale, "de");
    }
}

#[cfg(test)]
mod app_scope_tests {
    use super::AdminRegistry;

    #[test]
    fn registration_scope_is_used_only_once() {
        let mut registry = AdminRegistry::new().with_app("shop", "Shop");
        assert_eq!(registry.take_app_scope().as_deref(), Some("shop"));
        assert_eq!(registry.take_app_scope(), None);
        assert_eq!(registry.app_catalog().len(), 1);
    }

    #[test]
    fn invalid_scope_does_not_reuse_previously_selected_app() {
        let mut registry = AdminRegistry::new()
            .with_app("shop", "Shop")
            .with_app("Invalid-App", "Invalid");
        assert_eq!(registry.take_app_scope(), None);
        assert_eq!(registry.app_catalog().len(), 1);
    }

    #[test]
    fn successive_scopes_are_explicit() {
        let mut registry = AdminRegistry::new()
            .with_app("shop", "Shop")
            .with_app("notes", "Notes");
        assert_eq!(registry.take_app_scope().as_deref(), Some("notes"));
        assert_eq!(registry.take_app_scope(), None);
        assert_eq!(registry.app_catalog().len(), 2);
    }
}
