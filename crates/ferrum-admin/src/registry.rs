//! Explizite Admin-Registry. Nur Modelle mit `admin = true` werden aufgenommen.

use crate::actions::{self, AdminAction};
use crate::audit::{self, Event};
use ferrum_auth::{AuthUser, ObjectPolicy};
use ferrum_core::{AdminModelMeta, ColumnSpec, FerrumError, ModelDescriptor, QueryScope};
use ferrum_db::{query_admin_page, AdminPageRequest, DeleteOutcome, ModelRepository};
use sea_orm::{DatabaseConnection, TransactionTrait};
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
        offset: u64,
        limit: u64,
        scope: &QueryScope,
    ) -> Result<(Vec<Value>, bool, i64), FerrumError>;
    async fn read(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<Option<Value>, FerrumError>;
    async fn create(
        &self,
        db: &DatabaseConnection,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrumError>;
    async fn update(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<Value, FerrumError>;
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrumError>;
    fn object_allows(&self, user: &AuthUser, action: &str, record: &Value) -> bool;
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
            site_title: "Ferrum Admin".to_owned(),
            site_header: "Ferrum Verwaltung".to_owned(),
            app_name: "Ferrum".to_owned(),
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

    pub fn add<M>(self) -> Self
    where
        M: ModelRepository + 'static,
    {
        if M::admin_meta().is_none() {
            return self;
        }
        self.push(Arc::new(Plain::<M>::new()))
    }

    pub fn add_policy<M, P>(self) -> Self
    where
        M: ModelRepository + 'static,
        P: ObjectPolicy<M> + Send + Sync + 'static,
    {
        if M::admin_meta().is_none() {
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
        self.actions.push(action);
        self
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
        if model.meta().key.is_empty() {
            return self;
        }
        let models = Arc::make_mut(&mut self.models);
        if models
            .iter()
            .any(|existing| existing.meta().key == model.meta().key)
        {
            return self;
        }
        models.push(model);
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
        offset: u64,
        limit: u64,
        scope: &QueryScope,
    ) -> Result<(Vec<Value>, bool, i64), FerrumError> {
        load_page::<M>(
            db,
            search_columns,
            search,
            sort_column,
            descending,
            filters,
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
    ) -> Result<Option<Value>, FerrumError> {
        Ok(M::get_scoped(db, id, scope).await?.map(json_of))
    }
    async fn create(
        &self,
        db: &DatabaseConnection,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrumError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrumError::Validation(format!("request body is invalid: {error}"))
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
    ) -> Result<Value, FerrumError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrumError::Validation(format!("request body is invalid: {error}"))
        })?;
        audited_update::<M>(db, user, id, input, scope).await
    }
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrumError> {
        audited_delete::<M>(db, user, id, scope).await
    }
    fn object_allows(&self, _user: &AuthUser, _action: &str, _record: &Value) -> bool {
        true
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
        offset: u64,
        limit: u64,
        scope: &QueryScope,
    ) -> Result<(Vec<Value>, bool, i64), FerrumError> {
        load_page::<M>(
            db,
            search_columns,
            search,
            sort_column,
            descending,
            filters,
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
    ) -> Result<Option<Value>, FerrumError> {
        Ok(M::get_scoped(db, id, scope).await?.map(json_of))
    }
    async fn create(
        &self,
        db: &DatabaseConnection,
        body: Value,
        user: &AuthUser,
    ) -> Result<Value, FerrumError> {
        let input = serde_json::from_value::<M::Create>(body).map_err(|error| {
            FerrumError::Validation(format!("request body is invalid: {error}"))
        })?;
        if !user.is_superuser && !P::can_create(user, &input) {
            return Err(FerrumError::Denied(
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
    ) -> Result<Value, FerrumError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrumError::Validation(format!("request body is invalid: {error}"))
        })?;
        audited_update::<M>(db, user, id, input, scope).await
    }
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
        user: &AuthUser,
    ) -> Result<DeleteOutcome, FerrumError> {
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
}

#[allow(clippy::too_many_arguments)]
async fn load_page<M: ModelRepository>(
    db: &DatabaseConnection,
    search_columns: &[&str],
    search: Option<&str>,
    sort_column: &str,
    descending: bool,
    filters: &[(&str, sea_orm::Value)],
    offset: u64,
    limit: u64,
    scope: &QueryScope,
) -> Result<(Vec<Value>, bool, i64), FerrumError> {
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
) -> Result<Value, FerrumError>
where
    M: ModelRepository,
{
    let txn = db.begin().await.map_err(FerrumError::database)?;
    let created = M::create_tx(&txn, input).await?;
    let value = json_of(&created);
    write_audit::<M>(&txn, user, "create", "ok", &value, None, Some(&value)).await?;
    txn.commit().await.map_err(FerrumError::database)?;
    Ok(value)
}

async fn audited_update<M>(
    db: &DatabaseConnection,
    user: &AuthUser,
    id: uuid::Uuid,
    input: M::Update,
    scope: &QueryScope,
) -> Result<Value, FerrumError>
where
    M: ModelRepository,
{
    let txn = db.begin().await.map_err(FerrumError::database)?;
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
    txn.commit().await.map_err(FerrumError::database)?;
    Ok(value)
}

async fn audited_delete<M>(
    db: &DatabaseConnection,
    user: &AuthUser,
    id: uuid::Uuid,
    scope: &QueryScope,
) -> Result<DeleteOutcome, FerrumError>
where
    M: ModelRepository,
{
    let txn = db.begin().await.map_err(FerrumError::database)?;
    let before = M::get_tx(&txn, id, scope).await?;
    let outcome = M::delete_tx(&txn, id, scope).await?;
    if outcome == DeleteOutcome::Deleted {
        if let Some(record) = before {
            let value = json_of(record);
            write_audit::<M>(&txn, user, "delete", "ok", &value, Some(&value), None).await?;
        }
    }
    txn.commit().await.map_err(FerrumError::database)?;
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
) -> Result<(), FerrumError> {
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
