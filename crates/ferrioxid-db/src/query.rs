//! Composable, bounded queries for a `FerrioxidModel`.
//!
//! Column names come from the model whitelist. Values are bound parameters.
//! `list` and `list_scoped` stay capped at 100 rows; this module is the
//! pagination path.

use std::marker::PhantomData;

use ferrioxid_core::{ColumnKind, ColumnSpec, FerrioxidError, FerrioxidModel, QueryScope};
use sea_orm::sea_query::{
    Alias, Expr, Order, PostgresQueryBuilder, Query, QueryStatementWriter, SelectStatement,
    SimpleExpr, SqliteQueryBuilder,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::sql::{is_safe_ident, row_to_json, scope_condition, LIST_LIMIT};

/// Maximum page size. Matches `list` and the admin list.
pub const PAGE_LIMIT: u64 = LIST_LIMIT as u64;
/// Maximum `OFFSET`. Larger windows need a keyset that this API does not provide.
pub const OFFSET_LIMIT: u64 = 1_000_000;
/// Maximum number of bound values in one `IN` or `NOT IN` list.
pub const IN_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    Bool(bool),
    I64(i64),
    Text(String),
    Uuid(Uuid),
}

impl From<bool> for Bound {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for Bound {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<i32> for Bound {
    fn from(value: i32) -> Self {
        Self::I64(i64::from(value))
    }
}

impl From<String> for Bound {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Bound {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<Uuid> for Bound {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compare {
    Eq(Bound),
    Ne(Bound),
    Gt(Bound),
    Gte(Bound),
    Lt(Bound),
    Lte(Bound),
    In(Vec<Bound>),
    NotIn(Vec<Bound>),
    IsNull,
    IsNotNull,
}

/// A boolean expression over whitelisted columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Not(Box<Predicate>),
    Cmp {
        column: &'static str,
        compare: Compare,
    },
}

/// One model column. The name is checked against `M::columns()` when the query runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    name: &'static str,
}

pub fn col(name: &'static str) -> Column {
    Column { name }
}

impl Column {
    pub fn eq(self, value: impl Into<Bound>) -> Predicate {
        self.cmp(Compare::Eq(value.into()))
    }

    pub fn ne(self, value: impl Into<Bound>) -> Predicate {
        self.cmp(Compare::Ne(value.into()))
    }

    pub fn gt(self, value: impl Into<Bound>) -> Predicate {
        self.cmp(Compare::Gt(value.into()))
    }

    pub fn gte(self, value: impl Into<Bound>) -> Predicate {
        self.cmp(Compare::Gte(value.into()))
    }

    pub fn lt(self, value: impl Into<Bound>) -> Predicate {
        self.cmp(Compare::Lt(value.into()))
    }

    pub fn lte(self, value: impl Into<Bound>) -> Predicate {
        self.cmp(Compare::Lte(value.into()))
    }

    pub fn is_in<I, T>(self, values: I) -> Predicate
    where
        I: IntoIterator<Item = T>,
        T: Into<Bound>,
    {
        self.cmp(Compare::In(values.into_iter().map(Into::into).collect()))
    }

    pub fn is_not_in<I, T>(self, values: I) -> Predicate
    where
        I: IntoIterator<Item = T>,
        T: Into<Bound>,
    {
        self.cmp(Compare::NotIn(values.into_iter().map(Into::into).collect()))
    }

    pub fn is_null(self) -> Predicate {
        self.cmp(Compare::IsNull)
    }

    pub fn is_not_null(self) -> Predicate {
        self.cmp(Compare::IsNotNull)
    }

    fn cmp(self, compare: Compare) -> Predicate {
        Predicate::Cmp {
            column: self.name,
            compare,
        }
    }
}

pub fn all_of(parts: impl IntoIterator<Item = Predicate>) -> Predicate {
    Predicate::And(parts.into_iter().collect())
}

pub fn any_of(parts: impl IntoIterator<Item = Predicate>) -> Predicate {
    Predicate::Or(parts.into_iter().collect())
}

pub fn not(predicate: Predicate) -> Predicate {
    Predicate::Not(Box::new(predicate))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPage<T> {
    pub data: Vec<T>,
    pub limit: u64,
    pub offset: u64,
    pub total: i64,
    pub has_more: bool,
}

/// A bounded `SELECT` for `M`.
///
/// `filter` combines with AND. `any_of` and `not` build OR and NOT.
/// The default order is `id ASC`. Any other order gets `id ASC` appended
/// so pages do not overlap when sort keys are equal.
/// `all` and `page` return at most [`PAGE_LIMIT`] rows. `count` and `exists`
/// use the filter and scope, not the page window.
///
/// `scope` defaults to [`QueryScope::All`], the same default as `list`.
/// Pass the scope produced by the object policy. [`QueryScope::None`] matches
/// no row.
#[derive(Debug, Clone)]
pub struct Select<M> {
    predicate: Predicate,
    orders: Vec<(&'static str, Direction)>,
    limit: u64,
    offset: u64,
    scope: QueryScope,
    _model: PhantomData<M>,
}

pub trait ModelQuery: FerrioxidModel + DeserializeOwned + Sized {
    fn query() -> Select<Self> {
        Select::new()
    }
}

impl<M> ModelQuery for M where M: FerrioxidModel + DeserializeOwned {}

impl<M> Select<M> {
    pub fn new() -> Self {
        Self {
            predicate: Predicate::And(Vec::new()),
            orders: Vec::new(),
            limit: PAGE_LIMIT,
            offset: 0,
            scope: QueryScope::All,
            _model: PhantomData,
        }
    }

    pub fn filter(mut self, predicate: Predicate) -> Self {
        match &mut self.predicate {
            Predicate::And(parts) => parts.push(predicate),
            other => {
                let previous = std::mem::replace(other, Predicate::And(Vec::new()));
                *other = Predicate::And(vec![previous, predicate]);
            }
        }
        self
    }

    pub fn scope(mut self, scope: QueryScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn order_by(mut self, column: &'static str, direction: Direction) -> Self {
        self.orders.push((column, direction));
        self
    }

    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = limit;
        self
    }

    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }

    pub async fn all<C>(&self, db: &C) -> Result<Vec<M>, FerrioxidError>
    where
        M: FerrioxidModel + DeserializeOwned,
        C: ConnectionTrait,
    {
        let plan = self.plan()?;
        let mut query = select_body::<M>(plan.filter);
        apply_orders(&mut query, &plan.orders);
        query.limit(plan.limit).offset(plan.offset);
        fetch_models(db, &query).await
    }

    pub async fn count<C>(&self, db: &C) -> Result<i64, FerrioxidError>
    where
        M: FerrioxidModel,
        C: ConnectionTrait,
    {
        let plan = self.plan()?;
        let mut query = Query::select();
        query
            .expr_as(Expr::col(Alias::new("id")).count(), Alias::new("row_count"))
            .from(Alias::new(M::descriptor().table));
        if let Some(filter) = plan.filter {
            query.and_where(filter);
        }
        note_statement();
        let row = db
            .query_one(render(db.get_database_backend(), &query)?)
            .await
            .map_err(FerrioxidError::database)?
            .ok_or_else(|| {
                FerrioxidError::database(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "count returned no row",
                ))
            })?;
        row.try_get::<i64>("", "row_count")
            .or_else(|_| row.try_get::<i32>("", "row_count").map(i64::from))
            .map_err(FerrioxidError::database)
    }

    pub async fn exists<C>(&self, db: &C) -> Result<bool, FerrioxidError>
    where
        M: FerrioxidModel,
        C: ConnectionTrait,
    {
        let plan = self.plan()?;
        let mut query = Query::select();
        query
            .expr(Expr::val(1))
            .from(Alias::new(M::descriptor().table))
            .limit(1);
        if let Some(filter) = plan.filter {
            query.and_where(filter);
        }
        note_statement();
        let row = db
            .query_one(render(db.get_database_backend(), &query)?)
            .await
            .map_err(FerrioxidError::database)?;
        Ok(row.is_some())
    }

    pub async fn page<C>(&self, db: &C) -> Result<QueryPage<M>, FerrioxidError>
    where
        M: FerrioxidModel + DeserializeOwned,
        C: ConnectionTrait,
    {
        let total = self.count(db).await?;
        let data = self.all(db).await?;
        let end = self.offset.saturating_add(data.len() as u64);
        let has_more = total > i64::try_from(end).unwrap_or(i64::MAX);
        Ok(QueryPage {
            data,
            limit: self.limit,
            offset: self.offset,
            total,
            has_more,
        })
    }

    fn plan(&self) -> Result<Plan, FerrioxidError>
    where
        M: FerrioxidModel,
    {
        let columns = M::columns();
        let table = M::descriptor().table;
        if !is_safe_ident(table) {
            return Err(FerrioxidError::Validation(format!(
                "table {table} is not available"
            )));
        }
        if self.limit == 0 || self.limit > PAGE_LIMIT {
            return Err(FerrioxidError::Validation(
                "page size must be between 1 and 100".into(),
            ));
        }
        if self.offset > OFFSET_LIMIT {
            return Err(FerrioxidError::Validation(format!(
                "offset must be at most {OFFSET_LIMIT}"
            )));
        }
        let mut seen = Vec::new();
        for (name, _) in &self.orders {
            let column = lookup(columns, name)?;
            if seen.contains(&column.name) {
                return Err(FerrioxidError::Validation(format!(
                    "column {} is ordered more than once",
                    column.name
                )));
            }
            seen.push(column.name);
        }
        lookup(columns, "id")?;
        let mut orders = self.orders.clone();
        if !orders.iter().any(|(name, _)| *name == "id") {
            orders.push(("id", Direction::Asc));
        }
        let mut filter = predicate_expr(columns, &self.predicate)?;
        if let Some(scope) = scope_condition(columns, &self.scope)? {
            filter = Some(match filter {
                Some(current) => current.and(scope),
                None => scope,
            });
        }
        Ok(Plan {
            filter,
            orders,
            limit: self.limit,
            offset: self.offset,
        })
    }
}

impl<M> Default for Select<M> {
    fn default() -> Self {
        Self::new()
    }
}

struct Plan {
    filter: Option<SimpleExpr>,
    orders: Vec<(&'static str, Direction)>,
    limit: u64,
    offset: u64,
}

fn select_body<M: FerrioxidModel>(filter: Option<SimpleExpr>) -> SelectStatement {
    let mut query = Query::select();
    query
        .columns(M::columns().iter().map(|column| Alias::new(column.name)))
        .from(Alias::new(M::descriptor().table));
    if let Some(filter) = filter {
        query.and_where(filter);
    }
    query
}

fn apply_orders(query: &mut SelectStatement, orders: &[(&str, Direction)]) {
    for (name, direction) in orders {
        query.order_by(
            Alias::new(*name),
            match direction {
                Direction::Asc => Order::Asc,
                Direction::Desc => Order::Desc,
            },
        );
    }
}

fn predicate_expr(
    columns: &[ColumnSpec],
    predicate: &Predicate,
) -> Result<Option<SimpleExpr>, FerrioxidError> {
    match predicate {
        Predicate::And(parts) => {
            let mut combined: Option<SimpleExpr> = None;
            for part in parts {
                if let Some(expr) = predicate_expr(columns, part)? {
                    combined = Some(match combined {
                        Some(current) => current.and(expr),
                        None => expr,
                    });
                }
            }
            Ok(combined)
        }
        Predicate::Or(parts) => {
            if parts.is_empty() {
                return Ok(Some(Expr::val(0).eq(1)));
            }
            let mut combined: Option<SimpleExpr> = None;
            for part in parts {
                let expr = predicate_expr(columns, part)?.unwrap_or_else(|| Expr::val(1).eq(1));
                combined = Some(match combined {
                    Some(current) => current.or(expr),
                    None => expr,
                });
            }
            Ok(combined)
        }
        Predicate::Not(inner) => {
            let expr = predicate_expr(columns, inner)?.unwrap_or_else(|| Expr::val(1).eq(1));
            Ok(Some(expr.not()))
        }
        Predicate::Cmp { column, compare } => {
            Ok(Some(compare_expr(lookup(columns, column)?, compare)?))
        }
    }
}

fn compare_expr(column: &ColumnSpec, compare: &Compare) -> Result<SimpleExpr, FerrioxidError> {
    let values = match compare {
        Compare::Eq(value)
        | Compare::Ne(value)
        | Compare::Gt(value)
        | Compare::Gte(value)
        | Compare::Lt(value)
        | Compare::Lte(value) => vec![value.clone()],
        Compare::In(values) | Compare::NotIn(values) => {
            if values.is_empty() || values.len() > IN_LIMIT {
                return Err(FerrioxidError::Validation(
                    "IN and NOT IN need between 1 and 100 values".into(),
                ));
            }
            values.clone()
        }
        Compare::IsNull | Compare::IsNotNull => Vec::new(),
    };
    let inequality = matches!(
        compare,
        Compare::Gt(_) | Compare::Gte(_) | Compare::Lt(_) | Compare::Lte(_)
    );
    if values
        .iter()
        .any(|value| !value_matches(column.kind, value, inequality))
    {
        return Err(FerrioxidError::Validation(format!(
            "column {} does not accept this comparison",
            column.name
        )));
    }
    let expr = Expr::col(Alias::new(column.name));
    Ok(match compare {
        Compare::Eq(value) => expr.eq(bound_value(value)),
        Compare::Ne(value) => expr.ne(bound_value(value)),
        Compare::Gt(value) => expr.gt(bound_value(value)),
        Compare::Gte(value) => expr.gte(bound_value(value)),
        Compare::Lt(value) => expr.lt(bound_value(value)),
        Compare::Lte(value) => expr.lte(bound_value(value)),
        Compare::In(values) => expr.is_in(values.iter().map(bound_value)),
        Compare::NotIn(values) => expr.is_not_in(values.iter().map(bound_value)),
        Compare::IsNull => expr.is_null(),
        Compare::IsNotNull => expr.is_not_null(),
    })
}

fn value_matches(kind: ColumnKind, value: &Bound, inequality: bool) -> bool {
    match (kind, value) {
        (ColumnKind::Boolean, Bound::Bool(_)) => !inequality,
        (ColumnKind::Uuid, Bound::Uuid(_)) => !inequality,
        (ColumnKind::BigInt, Bound::I64(_)) => true,
        (
            ColumnKind::Text | ColumnKind::Date | ColumnKind::Time | ColumnKind::DateTime,
            Bound::Text(_),
        ) => true,
        _ => false,
    }
}

fn bound_value(value: &Bound) -> Value {
    match value {
        Bound::Bool(flag) => Value::Bool(Some(*flag)),
        Bound::I64(number) => Value::BigInt(Some(*number)),
        Bound::Text(text) => Value::String(Some(Box::new(text.clone()))),
        Bound::Uuid(id) => Value::Uuid(Some(Box::new(*id))),
    }
}

fn lookup<'a>(columns: &'a [ColumnSpec], name: &str) -> Result<&'a ColumnSpec, FerrioxidError> {
    if !is_safe_ident(name) {
        return Err(FerrioxidError::Validation(format!(
            "column {name} is not available"
        )));
    }
    columns
        .iter()
        .find(|column| column.name == name)
        .ok_or_else(|| FerrioxidError::Validation(format!("column {name} is not available")))
}

async fn fetch_models<M, C>(db: &C, query: &SelectStatement) -> Result<Vec<M>, FerrioxidError>
where
    M: FerrioxidModel + DeserializeOwned,
    C: ConnectionTrait,
{
    note_statement();
    let rows = db
        .query_all(render(db.get_database_backend(), query)?)
        .await
        .map_err(FerrioxidError::database)?;
    rows.into_iter()
        .map(|row| {
            let json = row_to_json(&row, M::columns())?;
            serde_json::from_value(json).map_err(FerrioxidError::database)
        })
        .collect()
}

fn render(
    backend: DatabaseBackend,
    query: &impl QueryStatementWriter,
) -> Result<Statement, FerrioxidError> {
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(FerrioxidError::database(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "MySQL is not a Ferrioxid database",
            )))
        }
    };
    Ok(Statement::from_sql_and_values(backend, sql, values.0))
}

fn note_statement() {
    #[cfg(test)]
    {
        STATEMENT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
pub(crate) static STATEMENT_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
impl<M> Select<M>
where
    M: FerrioxidModel,
{
    pub(crate) fn statement_for(
        &self,
        backend: DatabaseBackend,
    ) -> Result<Statement, FerrioxidError> {
        let plan = self.plan()?;
        let mut query = select_body::<M>(plan.filter);
        apply_orders(&mut query, &plan.orders);
        query.limit(plan.limit).offset(plan.offset);
        render(backend, &query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrioxid_core::{
        ColumnKind, ColumnSpec, DatabaseKind, ModelDescriptor, QueryScope, ScopeValue,
    };
    use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
    use serde::Deserialize;

    use crate::connect;

    const PAYLOAD: &str = "'); DROP TABLE ferrioxid_query_items; --";

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct Item {
        id: Uuid,
        name: String,
        stock: i64,
        active: bool,
        price_cents: i64,
        category_id: Option<Uuid>,
        owner_id: Uuid,
    }

    const DESCRIPTOR: ModelDescriptor = ModelDescriptor {
        name: "Item",
        table: "ferrioxid_query_items",
        api_path: "",
        allow_anonymous: false,
        permission: None,
        requires_object_policy: false,
    };

    fn column(
        name: &'static str,
        kind: ColumnKind,
        nullable: bool,
        primary_key: bool,
    ) -> ColumnSpec {
        ColumnSpec {
            name,
            kind,
            nullable,
            primary_key,
            default_bool: None,
            unique: false,
            indexed: false,
            foreign_table: None,
            foreign_column: None,
            on_delete: None,
        }
    }

    impl FerrioxidModel for Item {
        fn descriptor() -> &'static ModelDescriptor {
            &DESCRIPTOR
        }

        fn columns() -> &'static [ColumnSpec] {
            use std::sync::OnceLock;
            static COLUMNS: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            COLUMNS.get_or_init(|| {
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("name", ColumnKind::Text, false, false),
                    column("stock", ColumnKind::BigInt, false, false),
                    column("active", ColumnKind::Boolean, false, false),
                    column("price_cents", ColumnKind::BigInt, false, false),
                    column("category_id", ColumnKind::Uuid, true, false),
                    column("owner_id", ColumnKind::Uuid, false, false),
                ]
            })
        }

        fn schema_sql(kind: DatabaseKind) -> String {
            match kind {
                DatabaseKind::Sqlite => "CREATE TABLE ferrioxid_query_items (
                    id text PRIMARY KEY NOT NULL,
                    name text NOT NULL,
                    stock bigint NOT NULL,
                    active integer NOT NULL,
                    price_cents bigint NOT NULL,
                    category_id text,
                    owner_id text NOT NULL
                )"
                .to_owned(),
                DatabaseKind::Postgres => "CREATE TABLE ferrioxid_query_items (
                    id uuid PRIMARY KEY NOT NULL,
                    name text NOT NULL,
                    stock bigint NOT NULL,
                    active boolean NOT NULL,
                    price_cents bigint NOT NULL,
                    category_id uuid,
                    owner_id uuid NOT NULL
                )"
                .to_owned(),
            }
        }
    }

    async fn memory() -> DatabaseConnection {
        let db = connect("sqlite::memory:").await.unwrap();
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            Item::schema_sql(DatabaseKind::Sqlite),
        ))
        .await
        .unwrap();
        db
    }

    struct NewItem<'a> {
        id: u128,
        name: &'a str,
        stock: i64,
        active: bool,
        price: i64,
        category: Option<u128>,
        owner: u128,
    }

    async fn insert(db: &impl ConnectionTrait, row: NewItem<'_>) {
        let json = crate::sql::insert_record(
            db,
            Item::descriptor().table,
            Item::columns(),
            vec![
                Value::Uuid(Some(Box::new(Uuid::from_u128(row.id)))),
                Value::String(Some(Box::new(row.name.to_owned()))),
                Value::BigInt(Some(row.stock)),
                Value::Bool(Some(row.active)),
                Value::BigInt(Some(row.price)),
                match row.category {
                    Some(category) => Value::Uuid(Some(Box::new(Uuid::from_u128(category)))),
                    None => Value::Uuid(None),
                },
                Value::Uuid(Some(Box::new(Uuid::from_u128(row.owner)))),
            ],
        )
        .await
        .unwrap();
        serde_json::from_value::<Item>(json).unwrap();
    }

    async fn fixture(db: &DatabaseConnection) {
        insert(
            db,
            NewItem {
                id: 1,
                name: "alpha",
                stock: 0,
                active: true,
                price: 10,
                category: Some(50),
                owner: 7,
            },
        )
        .await;
        insert(
            db,
            NewItem {
                id: 2,
                name: "beta",
                stock: 4,
                active: true,
                price: 10,
                category: Some(50),
                owner: 7,
            },
        )
        .await;
        insert(
            db,
            NewItem {
                id: 3,
                name: "gamma",
                stock: 9,
                active: false,
                price: 5,
                category: None,
                owner: 8,
            },
        )
        .await;
        insert(
            db,
            NewItem {
                id: 4,
                name: PAYLOAD,
                stock: 3,
                active: true,
                price: 30,
                category: Some(51),
                owner: 7,
            },
        )
        .await;
        insert(
            db,
            NewItem {
                id: 5,
                name: "hidden",
                stock: 8,
                active: true,
                price: 15,
                category: Some(51),
                owner: 8,
            },
        )
        .await;
    }

    fn names(rows: &[Item]) -> Vec<&str> {
        rows.iter().map(|row| row.name.as_str()).collect()
    }

    #[test]
    fn postgres_and_sqlite_sql_bind_values_and_reject_unknown_fields() {
        let select = Item::query()
            .filter(col("active").eq(true))
            .filter(col("stock").gt(0_i64))
            .filter(col("name").eq(PAYLOAD))
            .order_by("price_cents", Direction::Asc)
            .limit(40)
            .offset(80);
        let sqlite = select.statement_for(DatabaseBackend::Sqlite).unwrap();
        let postgres = select.statement_for(DatabaseBackend::Postgres).unwrap();
        assert!(sqlite.sql.contains('?'));
        assert!(postgres.sql.contains("$1"));
        assert!(!sqlite.sql.contains("DROP"));
        assert!(!postgres.sql.contains("DROP"));
        assert!(sqlite.sql.contains("\"price_cents\""));
        assert!(sqlite.sql.contains("\"id\""));
        assert!(postgres.sql.contains("\"ferrioxid_query_items\""));
        let error = Item::query()
            .filter(col("name; DROP TABLE ferrioxid_query_items").eq(PAYLOAD))
            .statement_for(DatabaseBackend::Sqlite)
            .unwrap_err();
        assert!(error.to_string().contains("not available"));
        let missing = Item::query()
            .order_by("missing", Direction::Desc)
            .statement_for(DatabaseBackend::Sqlite)
            .unwrap_err();
        assert!(missing.to_string().contains("not available"));
    }

    #[tokio::test]
    async fn filters_sort_nulls_and_scopes_on_sqlite() {
        let db = memory().await;
        fixture(&db).await;

        let active = Item::query()
            .filter(col("active").eq(true))
            .filter(col("stock").gt(0_i64))
            .order_by("price_cents", Direction::Asc)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(names(&active), vec!["beta", "hidden", PAYLOAD]);

        let combined = Item::query()
            .filter(col("owner_id").eq(Uuid::from_u128(7)))
            .filter(any_of([
                col("stock").eq(0_i64),
                col("price_cents").gte(30_i64),
            ]))
            .filter(not(col("name").eq("hidden")))
            .order_by("name", Direction::Asc)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(names(&combined), vec![PAYLOAD, "alpha"]);

        let priced = Item::query()
            .filter(col("price_cents").gt(5_i64))
            .filter(col("price_cents").lte(15_i64))
            .filter(col("name").ne(PAYLOAD))
            .filter(col("name").is_in(["alpha", "beta", "hidden"]))
            .filter(col("name").is_not_in(["hidden"]))
            .order_by("price_cents", Direction::Asc)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(names(&priced), vec!["alpha", "beta"]);

        let missing_category = Item::query()
            .filter(col("category_id").is_null())
            .all(&db)
            .await
            .unwrap();
        assert_eq!(names(&missing_category), vec!["gamma"]);
        assert_eq!(
            Item::query()
                .filter(col("category_id").is_not_null())
                .count(&db)
                .await
                .unwrap(),
            4
        );

        let tied = Item::query()
            .filter(col("price_cents").eq(10_i64))
            .order_by("price_cents", Direction::Desc)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            tied.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![Uuid::from_u128(1), Uuid::from_u128(2)]
        );
        let again = Item::query()
            .filter(col("price_cents").eq(10_i64))
            .order_by("price_cents", Direction::Desc)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(tied, again);

        assert!(!Item::query()
            .filter(col("name").eq("absent"))
            .exists(&db)
            .await
            .unwrap());
        assert!(Item::query()
            .filter(col("active").eq(false))
            .exists(&db)
            .await
            .unwrap());
        let window = Item::query().limit(1);
        assert_eq!(window.clone().count(&db).await.unwrap(), 5);
        assert_eq!(window.all(&db).await.unwrap().len(), 1);

        let hidden = Item::query()
            .scope(QueryScope::None)
            .all(&db)
            .await
            .unwrap();
        assert!(hidden.is_empty());
        let owned = Item::query()
            .scope(QueryScope::Equals {
                column: "owner_id",
                value: ScopeValue::Uuid(Uuid::from_u128(8)),
            })
            .order_by("name", Direction::Asc)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(names(&owned), vec!["gamma", "hidden"]);

        let injected = Item::query()
            .filter(col("name").eq(PAYLOAD))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(injected.len(), 1);
        assert_eq!(Item::query().count(&db).await.unwrap(), 5);
        assert!(Item::query()
            .filter(any_of([]))
            .all(&db)
            .await
            .unwrap()
            .is_empty());

        assert!(Item::query()
            .filter(col("active").gt(true))
            .all(&db)
            .await
            .is_err());
        assert!(Item::query()
            .filter(col("stock").eq(true))
            .all(&db)
            .await
            .is_err());
        assert!(Item::query()
            .filter(col("name").is_in(Vec::<String>::new()))
            .all(&db)
            .await
            .is_err());
        assert!(Item::query()
            .filter(col("stock").is_in(0..=100))
            .all(&db)
            .await
            .is_err());
        assert!(Item::query().limit(0).all(&db).await.is_err());
        assert!(Item::query().limit(101).all(&db).await.is_err());
        assert!(Item::query()
            .offset(OFFSET_LIMIT + 1)
            .all(&db)
            .await
            .is_err());
        assert!(Item::query()
            .order_by("name", Direction::Asc)
            .order_by("name", Direction::Desc)
            .all(&db)
            .await
            .is_err());
        assert!(any_of([]).eq(&Predicate::Or(Vec::new())));
    }

    #[tokio::test]
    async fn pages_more_than_one_hundred_rows_without_gaps() {
        let db = memory().await;
        for number in 1_u128..=120 {
            let name = format!("row-{number}");
            insert(
                &db,
                NewItem {
                    id: number,
                    name: &name,
                    stock: 1,
                    active: true,
                    price: i64::try_from(number).unwrap(),
                    category: Some(50),
                    owner: 7,
                },
            )
            .await;
        }
        let listed = crate::sql::list_records(
            &db,
            Item::descriptor().table,
            Item::columns(),
            &QueryScope::All,
        )
        .await
        .unwrap();
        assert_eq!(listed.rows.len(), 100);
        assert!(listed.truncated);

        let mut seen = Vec::new();
        for offset in [0_u64, 40, 80] {
            let page = Item::query()
                .order_by("price_cents", Direction::Asc)
                .limit(40)
                .offset(offset)
                .page(&db)
                .await
                .unwrap();
            assert_eq!(page.total, 120);
            assert_eq!(page.data.len(), 40);
            assert_eq!(page.has_more, offset < 80);
            let ids: Vec<_> = page.data.iter().map(|row| row.id).collect();
            let mut ordered = ids.clone();
            ordered.sort();
            assert_eq!(ids, ordered);
            seen.extend(ids);
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 120);
        assert_eq!(seen.first().copied(), Some(Uuid::from_u128(1)));
        assert_eq!(seen.last().copied(), Some(Uuid::from_u128(120)));

        let first = Item::query()
            .order_by("price_cents", Direction::Asc)
            .limit(25)
            .page(&db)
            .await
            .unwrap();
        let repeated = Item::query()
            .order_by("price_cents", Direction::Asc)
            .limit(25)
            .page(&db)
            .await
            .unwrap();
        assert_eq!(first.data, repeated.data);
        assert_eq!(
            Item::query().all(&db).await.unwrap().len(),
            usize::try_from(PAGE_LIMIT).unwrap()
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
    async fn postgres_filters_and_pages_runtime() {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run the ignored PostgreSQL query test");
        let db = connect(&url).await.expect("postgres connection");
        let _ = db
            .execute(Statement::from_string(
                DatabaseBackend::Postgres,
                "DROP TABLE IF EXISTS ferrioxid_query_items".to_owned(),
            ))
            .await;
        db.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            Item::schema_sql(DatabaseKind::Postgres),
        ))
        .await
        .expect("create query table");
        insert(
            &db,
            NewItem {
                id: 1,
                name: "alpha",
                stock: 2,
                active: true,
                price: 10,
                category: Some(50),
                owner: 7,
            },
        )
        .await;
        insert(
            &db,
            NewItem {
                id: 2,
                name: PAYLOAD,
                stock: 0,
                active: false,
                price: 5,
                category: None,
                owner: 8,
            },
        )
        .await;
        let rows = Item::query()
            .filter(col("active").eq(true))
            .filter(col("stock").gt(0_i64))
            .order_by("price_cents", Direction::Asc)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(names(&rows), vec!["alpha"]);
        let page = Item::query().limit(1).offset(1).page(&db).await.unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.data.len(), 1);
        assert!(!page.has_more);
        db.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "DROP TABLE IF EXISTS ferrioxid_query_items".to_owned(),
        ))
        .await
        .unwrap();
    }
}
