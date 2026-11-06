//! Explizite Admin-Registry. Nur Modelle mit `admin = true` werden aufgenommen.

use ferrum_auth::{AuthUser, ObjectPolicy};
use ferrum_core::{AdminModelMeta, ColumnSpec, FerrumError, ModelDescriptor, QueryScope};
use ferrum_db::{query_admin_page, AdminPageRequest, DeleteOutcome, ModelRepository};
use sea_orm::DatabaseConnection;
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
    async fn create(&self, db: &DatabaseConnection, body: Value) -> Result<Value, FerrumError>;
    async fn update(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
    ) -> Result<Value, FerrumError>;
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<DeleteOutcome, FerrumError>;
    fn object_allows(&self, user: &AuthUser, action: &str, record: &Value) -> bool;
}

#[derive(Clone, Default)]
pub struct AdminRegistry {
    models: Arc<Vec<Arc<dyn ManagedModel>>>,
}

impl AdminRegistry {
    pub fn new() -> Self {
        Self::default()
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
    async fn create(&self, db: &DatabaseConnection, body: Value) -> Result<Value, FerrumError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrumError::Validation(format!("request body is invalid: {error}"))
        })?;
        Ok(json_of(M::create(db, input).await?))
    }
    async fn update(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
    ) -> Result<Value, FerrumError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrumError::Validation(format!("request body is invalid: {error}"))
        })?;
        Ok(json_of(M::update_scoped(db, id, input, scope).await?))
    }
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<DeleteOutcome, FerrumError> {
        M::delete_scoped(db, id, scope).await
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
    async fn create(&self, db: &DatabaseConnection, body: Value) -> Result<Value, FerrumError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrumError::Validation(format!("request body is invalid: {error}"))
        })?;
        Ok(json_of(M::create(db, input).await?))
    }
    async fn update(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        body: Value,
        scope: &QueryScope,
    ) -> Result<Value, FerrumError> {
        let input = serde_json::from_value(body).map_err(|error| {
            FerrumError::Validation(format!("request body is invalid: {error}"))
        })?;
        Ok(json_of(M::update_scoped(db, id, input, scope).await?))
    }
    async fn remove(
        &self,
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &QueryScope,
    ) -> Result<DeleteOutcome, FerrumError> {
        M::delete_scoped(db, id, scope).await
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
