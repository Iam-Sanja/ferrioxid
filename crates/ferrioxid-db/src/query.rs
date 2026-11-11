//! Composable, bounded queries for a `FerrioxidModel`.
//!
//! Column names come from the model whitelist. Values are bound parameters.
//! `list` and `list_scoped` stay capped at 100 rows; this module is the
//! pagination path. `page` keeps that cap and rejects offsets above
//! [`OFFSET_LIMIT`]. `keyset` is the stable alternative: it walks the same
//! order, including the `id` tie-break, and does not use `OFFSET`.
//! Aggregates and many-to-many loads are separate statements, not one query
//! per parent row.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use ferrioxid_core::{ColumnKind, ColumnSpec, FerrioxidError, FerrioxidModel, QueryScope};
use sea_orm::sea_query::{
    extension::sqlite::SqliteExpr, Alias, Condition, Expr, ExprTrait, Func, JoinType, LikeExpr,
    Order, PostgresQueryBuilder, Query, QueryStatementWriter, SelectStatement, SimpleExpr,
    SqliteQueryBuilder,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use uuid::Uuid;

use crate::sql::{
    escape_like, is_safe_ident, row_to_json, scope_condition, scope_condition_in, LIST_LIMIT,
};

/// Maximum page size. Matches `list` and the admin list.
pub const PAGE_LIMIT: u64 = LIST_LIMIT as u64;
/// Maximum `OFFSET` for [`Select::page`]. Larger windows use [`Select::keyset`].
pub const OFFSET_LIMIT: u64 = 1_000_000;
/// Maximum number of bound values in one `IN` or `NOT IN` list.
pub const IN_LIMIT: usize = 100;
/// Maximum length of one text-search term, before LIKE or GLOB escaping.
pub const TEXT_MATCH_LIMIT: usize = 200;

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
    Contains {
        value: String,
        case_insensitive: bool,
    },
    StartsWith {
        value: String,
        case_insensitive: bool,
    },
    EndsWith {
        value: String,
        case_insensitive: bool,
    },
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

    /// Case-sensitive substring match. `%`, `_` and `\` in `value` are escaped.
    ///
    /// SQLite uses `GLOB` because its `LIKE` is case-insensitive for ASCII.
    /// PostgreSQL uses `LIKE ... ESCAPE`. The term is a bound parameter.
    pub fn contains(self, value: impl Into<String>) -> Predicate {
        self.text_match(TextPosition::Contains, false, value.into())
    }

    /// Case-sensitive prefix match. See [`Column::contains`].
    pub fn starts_with(self, value: impl Into<String>) -> Predicate {
        self.text_match(TextPosition::StartsWith, false, value.into())
    }

    /// Case-sensitive suffix match. See [`Column::contains`].
    pub fn ends_with(self, value: impl Into<String>) -> Predicate {
        self.text_match(TextPosition::EndsWith, false, value.into())
    }

    /// Case-insensitive substring match via `LOWER(column) LIKE … ESCAPE '\'`.
    ///
    /// The pattern is folded in Rust before binding: [`fold_case_insensitive_pattern`]
    /// uses ASCII lowercasing on SQLite and full Unicode lowercasing on PostgreSQL,
    /// matching each backend's `LOWER(column)` behaviour. This is not Unicode
    /// casefolding. `%`, `_` and `\` in the term are escaped and bound.
    pub fn contains_ignore_case(self, value: impl Into<String>) -> Predicate {
        self.text_match(TextPosition::Contains, true, value.into())
    }

    /// Case-insensitive prefix match. See [`Column::contains_ignore_case`].
    pub fn starts_with_ignore_case(self, value: impl Into<String>) -> Predicate {
        self.text_match(TextPosition::StartsWith, true, value.into())
    }

    /// Case-insensitive suffix match. See [`Column::contains_ignore_case`].
    pub fn ends_with_ignore_case(self, value: impl Into<String>) -> Predicate {
        self.text_match(TextPosition::EndsWith, true, value.into())
    }

    fn text_match(
        self,
        position: TextPosition,
        case_insensitive: bool,
        value: String,
    ) -> Predicate {
        let compare = match position {
            TextPosition::Contains => Compare::Contains {
                value,
                case_insensitive,
            },
            TextPosition::StartsWith => Compare::StartsWith {
                value,
                case_insensitive,
            },
            TextPosition::EndsWith => Compare::EndsWith {
                value,
                case_insensitive,
            },
        };
        self.cmp(compare)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextPosition {
    Contains,
    StartsWith,
    EndsWith,
}

/// Filters for a related model. The page window of the outer [`Select`] is not copied.
#[derive(Debug, Clone)]
pub struct Related<R> {
    predicate: Predicate,
    scope: QueryScope,
    negated: bool,
    _model: PhantomData<R>,
}

impl<R> Related<R> {
    pub fn filter(mut self, predicate: Predicate) -> Self {
        self.predicate = match self.predicate {
            Predicate::And(mut parts) => {
                parts.push(predicate);
                Predicate::And(parts)
            }
            other => Predicate::And(vec![other, predicate]),
        };
        self
    }

    pub fn scope(mut self, scope: QueryScope) -> Self {
        self.scope = scope;
        self
    }

    /// Turns the relation check into `NOT EXISTS`.
    pub fn negated(mut self) -> Self {
        self.negated = true;
        self
    }

    fn into_filter(
        self,
        table: &'static str,
        columns: &'static [ColumnSpec],
        link: RelationLink,
    ) -> RelationFilter {
        RelationFilter {
            table,
            columns,
            link,
            predicate: self.predicate,
            scope: self.scope,
            negated: self.negated,
        }
    }
}

impl<R: FerrioxidModel> Related<R> {
    pub fn new() -> Self {
        Self {
            predicate: Predicate::And(Vec::new()),
            scope: QueryScope::All,
            negated: false,
            _model: PhantomData,
        }
    }
}

impl<R: FerrioxidModel> Default for Related<R> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
enum RelationLink {
    ForeignKey { column: &'static str },
    Has { foreign_key: &'static str },
    Many(ManyToMany),
}

#[derive(Debug, Clone)]
struct RelationFilter {
    table: &'static str,
    columns: &'static [ColumnSpec],
    link: RelationLink,
    predicate: Predicate,
    scope: QueryScope,
    negated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPage<T> {
    pub data: Vec<T>,
    pub limit: u64,
    pub offset: u64,
    pub total: i64,
    pub has_more: bool,
}

/// One keyset page. `next` is `None` when the page is empty.
/// Pass `next` back to [`Select::keyset`] to read the following page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeysetPage<T> {
    pub data: Vec<T>,
    pub limit: u64,
    pub next: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFn {
    Count,
    Sum,
}

/// A related `COUNT` or `SUM` for [`Select::annotate`].
pub struct Aggregate<Child> {
    pub alias: &'static str,
    pub function: AggregateFn,
    pub foreign_key: &'static str,
    pub value_column: Option<&'static str>,
    pub child_scope: QueryScope,
    _child: PhantomData<Child>,
}

impl<Child> Aggregate<Child> {
    pub fn count(alias: &'static str, foreign_key: &'static str) -> Self {
        Self {
            alias,
            function: AggregateFn::Count,
            foreign_key,
            value_column: None,
            child_scope: QueryScope::All,
            _child: PhantomData,
        }
    }

    pub fn sum(alias: &'static str, foreign_key: &'static str, value_column: &'static str) -> Self {
        Self {
            alias,
            function: AggregateFn::Sum,
            foreign_key,
            value_column: Some(value_column),
            child_scope: QueryScope::All,
            _child: PhantomData,
        }
    }

    pub fn scope(mut self, scope: QueryScope) -> Self {
        self.child_scope = scope;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotated<M> {
    pub row: M,
    pub alias: &'static str,
    pub value: i64,
}

/// Join table for [`load_many_to_many`]. Both columns are UUID foreign keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManyToMany {
    pub join_table: &'static str,
    pub parent_column: &'static str,
    pub child_column: &'static str,
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
    relations: Vec<RelationFilter>,
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
            relations: Vec::new(),
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

    /// Keeps rows whose foreign key `column` points at a row of `R` matching `related`.
    ///
    /// `column.foreign_table` must be `R`'s table. The related scope and predicate
    /// run inside `EXISTS` and use the same plan as `count`, `page` and `keyset`.
    pub fn where_related<R>(mut self, column: &'static str, related: Related<R>) -> Self
    where
        R: FerrioxidModel,
    {
        self.relations.push(related.into_filter(
            R::descriptor().table,
            R::columns(),
            RelationLink::ForeignKey { column },
        ));
        self
    }

    /// Keeps rows that have at least one `R` row whose foreign key references this model.
    pub fn where_has<R>(mut self, foreign_key: &'static str, related: Related<R>) -> Self
    where
        R: FerrioxidModel,
    {
        self.relations.push(related.into_filter(
            R::descriptor().table,
            R::columns(),
            RelationLink::Has { foreign_key },
        ));
        self
    }

    /// Keeps rows linked through `link`. Both id columns must be UUIDs.
    /// The join table is not a model; its identifiers are checked, not its rows' types.
    pub fn where_linked<R>(mut self, link: ManyToMany, related: Related<R>) -> Self
    where
        R: FerrioxidModel,
    {
        self.relations.push(related.into_filter(
            R::descriptor().table,
            R::columns(),
            RelationLink::Many(link),
        ));
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
        let plan = self.plan(db.get_database_backend())?;
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
        let plan = self.plan(db.get_database_backend())?;
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
        let plan = self.plan(db.get_database_backend())?;
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

    /// Returns the next page after `cursor`.
    ///
    /// `None` starts at the first row. The cursor is an opaque token for the
    /// last returned row and the planned order, including the `id` tie-break.
    /// [`QueryScope`] is applied. Offset is not used; a non-zero offset is
    /// rejected. Nullable and boolean order columns are rejected because their
    /// seek comparison is not stable on both backends. The page size stays
    /// between 1 and [`PAGE_LIMIT`].
    pub async fn keyset<C>(
        &self,
        db: &C,
        cursor: Option<&str>,
    ) -> Result<KeysetPage<M>, FerrioxidError>
    where
        M: FerrioxidModel + DeserializeOwned,
        C: ConnectionTrait,
    {
        if self.offset != 0 {
            return Err(FerrioxidError::Validation(
                "keyset pagination does not use offset".into(),
            ));
        }
        let plan = self.plan(db.get_database_backend())?;
        reject_keyset_orders(M::columns(), &plan.orders)?;
        let mut query = select_body::<M>(plan.filter);
        if let Some(token) = cursor {
            let values = decode_cursor(token)?;
            if values.len() != plan.orders.len() {
                return Err(FerrioxidError::Validation(
                    "keyset cursor does not match the order".into(),
                ));
            }
            query.and_where(seek_expr(
                M::columns(),
                &plan.orders,
                &values,
                db.get_database_backend(),
            )?);
        }
        apply_orders(&mut query, &plan.orders);
        query.limit(plan.limit.saturating_add(1));
        let mut pairs = fetch_pairs(db, &query).await?;
        let has_more = pairs.len() as u64 > plan.limit;
        if has_more {
            pairs.pop();
        }
        let next = match pairs.last() {
            Some((json, _)) => Some(encode_cursor(M::columns(), &plan.orders, json)?),
            None => None,
        };
        Ok(KeysetPage {
            data: pairs.into_iter().map(|(_, row)| row).collect(),
            limit: plan.limit,
            next,
            has_more,
        })
    }

    /// Attaches one related `COUNT` or `SUM` with a single grouped join.
    ///
    /// The parent filter, scope, order and page window stay in force. The
    /// child scope is applied inside the aggregate and cannot be skipped.
    /// Missing children are `0`. `order` sorts by the aggregate before the
    /// parent order. The returned page still contains at most [`PAGE_LIMIT`]
    /// parents.
    pub async fn annotate<Child, C>(
        &self,
        db: &C,
        spec: Aggregate<Child>,
        order: Option<Direction>,
    ) -> Result<Vec<Annotated<M>>, FerrioxidError>
    where
        M: FerrioxidModel + DeserializeOwned,
        Child: FerrioxidModel,
        C: ConnectionTrait,
    {
        let plan = self.plan(db.get_database_backend())?;
        let child_table = Child::descriptor().table;
        if !is_safe_ident(child_table) || !is_safe_ident(spec.foreign_key) {
            return Err(FerrioxidError::Validation(
                "aggregate columns are not available".into(),
            ));
        }
        let child_fk = lookup(Child::columns(), spec.foreign_key)?;
        if child_fk.kind != ColumnKind::Uuid {
            return Err(FerrioxidError::Validation(format!(
                "column {} is not a parent reference",
                spec.foreign_key
            )));
        }
        let mut sub = Query::select();
        sub.column(Alias::new(spec.foreign_key))
            .from(Alias::new(child_table))
            .group_by_col(Alias::new(spec.foreign_key));
        match spec.function {
            AggregateFn::Count => {
                sub.expr_as(
                    Expr::col(Alias::new(spec.foreign_key)).count(),
                    Alias::new("aggregate_value"),
                );
            }
            AggregateFn::Sum => {
                let Some(value_column) = spec.value_column else {
                    return Err(FerrioxidError::Validation(
                        "sum needs a value column".into(),
                    ));
                };
                let column = lookup(Child::columns(), value_column)?;
                if column.kind != ColumnKind::BigInt {
                    return Err(FerrioxidError::Validation(format!(
                        "column {value_column} cannot be summed"
                    )));
                }
                sub.expr_as(
                    Expr::col(Alias::new(value_column)).sum(),
                    Alias::new("aggregate_value"),
                );
            }
        }
        if let Some(scope) = scope_condition(Child::columns(), &spec.child_scope)? {
            sub.and_where(scope);
        }
        let table = M::descriptor().table;
        let mut query = Query::select();
        for column in M::columns() {
            query.expr_as(
                Expr::col((Alias::new(table), Alias::new(column.name))),
                Alias::new(column.name),
            );
        }
        let aggregate = Func::coalesce([
            Expr::col((Alias::new("ferrioxid_agg"), Alias::new("aggregate_value"))).into(),
            Expr::val(0).into(),
        ]);
        let cast = match db.get_database_backend() {
            DatabaseBackend::Postgres => "bigint",
            DatabaseBackend::Sqlite => "integer",
            DatabaseBackend::MySql => {
                return Err(FerrioxidError::database(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "MySQL is not a Ferrioxid database",
                )))
            }
        };
        query.expr_as(
            Func::cast_as(aggregate, Alias::new(cast)),
            Alias::new("ferrioxid_aggregate"),
        );
        query.from(Alias::new(table)).join_subquery(
            JoinType::LeftJoin,
            sub,
            Alias::new("ferrioxid_agg"),
            Condition::all().add(
                Expr::col((Alias::new("ferrioxid_agg"), Alias::new(spec.foreign_key)))
                    .eq(Expr::col((Alias::new(table), Alias::new("id")))),
            ),
        );
        if let Some(filter) = plan.filter {
            query.and_where(filter);
        }
        if let Some(direction) = order {
            query.order_by(
                Alias::new("ferrioxid_aggregate"),
                match direction {
                    Direction::Asc => Order::Asc,
                    Direction::Desc => Order::Desc,
                },
            );
        }
        apply_orders(&mut query, &plan.orders);
        query.limit(plan.limit).offset(plan.offset);
        note_statement();
        let rows = db
            .query_all(render(db.get_database_backend(), &query)?)
            .await
            .map_err(FerrioxidError::database)?;
        let mut annotated = Vec::with_capacity(rows.len());
        for row in rows {
            let value = row
                .try_get::<i64>("", "ferrioxid_aggregate")
                .or_else(|_| row.try_get::<i32>("", "ferrioxid_aggregate").map(i64::from))
                .map_err(FerrioxidError::database)?;
            let json = row_to_json(&row, M::columns())?;
            let model = serde_json::from_value(json).map_err(FerrioxidError::database)?;
            annotated.push(Annotated {
                row: model,
                alias: spec.alias,
                value,
            });
        }
        Ok(annotated)
    }

    fn plan(&self, backend: DatabaseBackend) -> Result<Plan, FerrioxidError>
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
        let mut filter = predicate_expr(columns, &self.predicate, backend, None)?;
        for (index, relation) in self.relations.iter().enumerate() {
            let exists = relation_expr::<M>(relation, index, backend)?;
            filter = Some(match filter {
                Some(current) => current.and(exists),
                None => exists,
            });
        }
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
    backend: DatabaseBackend,
    qualifier: Option<&str>,
) -> Result<Option<SimpleExpr>, FerrioxidError> {
    match predicate {
        Predicate::And(parts) => {
            let mut combined: Option<SimpleExpr> = None;
            for part in parts {
                if let Some(expr) = predicate_expr(columns, part, backend, qualifier)? {
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
                let expr = predicate_expr(columns, part, backend, qualifier)?
                    .unwrap_or_else(|| Expr::val(1).eq(1));
                combined = Some(match combined {
                    Some(current) => current.or(expr),
                    None => expr,
                });
            }
            Ok(combined)
        }
        Predicate::Not(inner) => {
            let expr = predicate_expr(columns, inner, backend, qualifier)?
                .unwrap_or_else(|| Expr::val(1).eq(1));
            Ok(Some(expr.not()))
        }
        Predicate::Cmp { column, compare } => Ok(Some(compare_expr(
            lookup(columns, column)?,
            compare,
            backend,
            qualifier,
        )?)),
    }
}

fn compare_expr(
    column: &ColumnSpec,
    compare: &Compare,
    backend: DatabaseBackend,
    qualifier: Option<&str>,
) -> Result<SimpleExpr, FerrioxidError> {
    if let Some(expr) = text_match_expr(column, compare, backend, qualifier)? {
        return Ok(expr);
    }
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
        Compare::Contains { .. } | Compare::StartsWith { .. } | Compare::EndsWith { .. } => {
            Vec::new()
        }
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
    let expr = qualified_col(qualifier, column.name);
    let bound = |value: &Bound| -> Result<Value, FerrioxidError> {
        bind_for_column(column, value, backend)
    };
    Ok(match compare {
        Compare::Eq(value) => expr.eq(bound(value)?),
        Compare::Ne(value) => expr.ne(bound(value)?),
        Compare::Gt(value) => expr.gt(bound(value)?),
        Compare::Gte(value) => expr.gte(bound(value)?),
        Compare::Lt(value) => expr.lt(bound(value)?),
        Compare::Lte(value) => expr.lte(bound(value)?),
        Compare::In(values) => expr.is_in(values.iter().map(bound).collect::<Result<Vec<_>, _>>()?),
        Compare::NotIn(values) => {
            expr.is_not_in(values.iter().map(bound).collect::<Result<Vec<_>, _>>()?)
        }
        Compare::IsNull => expr.is_null(),
        Compare::IsNotNull => expr.is_not_null(),
        Compare::Contains { .. } | Compare::StartsWith { .. } | Compare::EndsWith { .. } => {
            return Err(FerrioxidError::Validation(format!(
                "column {} does not accept this comparison",
                column.name
            )));
        }
    })
}

fn text_match_expr(
    column: &ColumnSpec,
    compare: &Compare,
    backend: DatabaseBackend,
    qualifier: Option<&str>,
) -> Result<Option<SimpleExpr>, FerrioxidError> {
    let (value, case_insensitive, position) = match compare {
        Compare::Contains {
            value,
            case_insensitive,
        } => (value, *case_insensitive, TextPosition::Contains),
        Compare::StartsWith {
            value,
            case_insensitive,
        } => (value, *case_insensitive, TextPosition::StartsWith),
        Compare::EndsWith {
            value,
            case_insensitive,
        } => (value, *case_insensitive, TextPosition::EndsWith),
        _ => return Ok(None),
    };
    if !matches!(column.kind, ColumnKind::Text | ColumnKind::Choice) {
        return Err(FerrioxidError::Validation(format!(
            "column {} does not accept this comparison",
            column.name
        )));
    }
    if value.chars().count() > TEXT_MATCH_LIMIT {
        return Err(FerrioxidError::Validation(format!(
            "text search is limited to {TEXT_MATCH_LIMIT} characters"
        )));
    }
    let column_expr = qualified_col(qualifier, column.name);
    if case_insensitive {
        let pattern = match position {
            TextPosition::Contains => format!("%{}%", escape_like(value)),
            TextPosition::StartsWith => format!("{}%", escape_like(value)),
            TextPosition::EndsWith => format!("%{}", escape_like(value)),
        };
        let folded = fold_case_insensitive_pattern(pattern, backend);
        return Ok(Some(
            Expr::expr(Func::cust("LOWER").arg(column_expr))
                .like(LikeExpr::new(folded).escape('\\')),
        ));
    }
    let expr = match backend {
        DatabaseBackend::Postgres => {
            let pattern = match position {
                TextPosition::Contains => format!("%{}%", escape_like(value)),
                TextPosition::StartsWith => format!("{}%", escape_like(value)),
                TextPosition::EndsWith => format!("%{}", escape_like(value)),
            };
            column_expr.like(LikeExpr::new(pattern).escape('\\'))
        }
        DatabaseBackend::Sqlite => {
            let pattern = match position {
                TextPosition::Contains => format!("*{}*", escape_glob(value)),
                TextPosition::StartsWith => format!("{}*", escape_glob(value)),
                TextPosition::EndsWith => format!("*{}", escape_glob(value)),
            };
            column_expr.glob(pattern)
        }
        DatabaseBackend::MySql => {
            return Err(FerrioxidError::Validation("MySQL is not supported".into()));
        }
    };
    Ok(Some(expr))
}

fn fold_case_insensitive_pattern(pattern: String, backend: DatabaseBackend) -> String {
    crate::sql::fold_case_insensitive_pattern(pattern, backend)
}

fn escape_glob(term: &str) -> String {
    let mut escaped = String::new();
    for char in term.chars() {
        if matches!(char, '*' | '?' | '[' | ']') {
            escaped.push('[');
            escaped.push(char);
            escaped.push(']');
        } else {
            escaped.push(char);
        }
    }
    escaped
}

fn qualified_col(qualifier: Option<&str>, name: &str) -> SimpleExpr {
    match qualifier {
        Some(alias) => Expr::col((Alias::new(alias), Alias::new(name))).into(),
        None => Expr::col(Alias::new(name)).into(),
    }
}

fn relation_expr<M: FerrioxidModel>(
    relation: &RelationFilter,
    index: usize,
    backend: DatabaseBackend,
) -> Result<SimpleExpr, FerrioxidError> {
    if !is_safe_ident(relation.table) {
        return Err(FerrioxidError::Validation(format!(
            "table {} is not available",
            relation.table
        )));
    }
    let parent = M::descriptor().table;
    let alias = format!("ferrioxid_related_{index}");
    let link = link_condition::<M>(relation, parent, &alias)?;
    let mut query = Query::select();
    query
        .expr(Expr::val(1))
        .from_as(Alias::new(relation.table), Alias::new(&alias));
    query.and_where(link);
    if let Some(filter) =
        predicate_expr(relation.columns, &relation.predicate, backend, Some(&alias))?
    {
        query.and_where(filter);
    }
    if let Some(scope) = scope_condition_in(relation.columns, &relation.scope, Some(&alias))? {
        query.and_where(scope);
    }
    let exists = Expr::exists(query);
    Ok(if relation.negated {
        exists.not()
    } else {
        exists
    })
}

fn link_condition<M: FerrioxidModel>(
    relation: &RelationFilter,
    parent: &str,
    alias: &str,
) -> Result<SimpleExpr, FerrioxidError> {
    match relation.link {
        RelationLink::ForeignKey { column } => {
            let parent_column = lookup(M::columns(), column)?;
            let related_id = lookup(relation.columns, "id")?;
            if parent_column.foreign_table != Some(relation.table)
                || parent_column.kind != related_id.kind
            {
                return Err(FerrioxidError::Validation(format!(
                    "column {} is not a foreign key to {}",
                    parent_column.name, relation.table
                )));
            }
            Ok(
                qualified_col(Some(alias), "id")
                    .eq(qualified_col(Some(parent), parent_column.name)),
            )
        }
        RelationLink::Has { foreign_key } => {
            let child_column = lookup(relation.columns, foreign_key)?;
            let parent_id = lookup(M::columns(), "id")?;
            if child_column.foreign_table != Some(parent) || child_column.kind != parent_id.kind {
                return Err(FerrioxidError::Validation(format!(
                    "column {foreign_key} is not a foreign key to {parent}"
                )));
            }
            Ok(qualified_col(Some(alias), foreign_key).eq(qualified_col(Some(parent), "id")))
        }
        RelationLink::Many(link) => {
            if !is_safe_ident(link.join_table)
                || !is_safe_ident(link.parent_column)
                || !is_safe_ident(link.child_column)
                || link.join_table == parent
                || link.join_table == relation.table
            {
                return Err(FerrioxidError::Validation(
                    "many-to-many link is not available".into(),
                ));
            }
            let parent_id = lookup(M::columns(), "id")?;
            let child_id = lookup(relation.columns, "id")?;
            if parent_id.kind != ColumnKind::Uuid || child_id.kind != ColumnKind::Uuid {
                return Err(FerrioxidError::Validation(
                    "many-to-many links require UUID primary keys".into(),
                ));
            }
            let link_alias = format!("{alias}_link");
            let mut membership = Query::select();
            membership
                .expr(Expr::val(1))
                .from_as(Alias::new(link.join_table), Alias::new(&link_alias))
                .and_where(
                    qualified_col(Some(&link_alias), link.parent_column)
                        .eq(qualified_col(Some(parent), "id")),
                )
                .and_where(
                    qualified_col(Some(&link_alias), link.child_column)
                        .eq(qualified_col(Some(alias), "id")),
                );
            Ok(Expr::exists(membership))
        }
    }
}

fn value_matches(kind: ColumnKind, value: &Bound, inequality: bool) -> bool {
    match (kind, value) {
        (ColumnKind::Boolean, Bound::Bool(_)) => !inequality,
        (ColumnKind::Uuid, Bound::Uuid(_)) => !inequality,
        (ColumnKind::BigInt, Bound::I64(_)) => true,
        (
            ColumnKind::Text
            | ColumnKind::Date
            | ColumnKind::Time
            | ColumnKind::DateTime
            | ColumnKind::Choice,
            Bound::Text(_),
        ) => true,
        (ColumnKind::Decimal, Bound::Text(_)) => true,
        _ => false,
    }
}

fn bind_for_column(
    column: &ColumnSpec,
    value: &Bound,
    backend: DatabaseBackend,
) -> Result<Value, FerrioxidError> {
    if matches!(
        column.kind,
        ColumnKind::Date | ColumnKind::Time | ColumnKind::DateTime
    ) {
        return bind_temporal(column, value, backend);
    }
    if column.kind == ColumnKind::Decimal {
        let Bound::Text(text) = value else {
            return Err(FerrioxidError::Validation(format!(
                "column {} does not accept this comparison",
                column.name
            )));
        };
        let bytes = ferrioxid_core::encode_decimal(
            ferrioxid_core::parse_decimal(text, column.precision, column.scale).map_err(
                |error| FerrioxidError::Validation(format!("{} {}", column.name, error.message)),
            )?,
            column.precision,
            column.scale,
        )
        .map_err(|error| {
            FerrioxidError::Validation(format!("{} {}", column.name, error.message))
        })?;
        return Ok(Value::Bytes(Some(Box::new(bytes.to_vec()))));
    }
    if column.kind == ColumnKind::Choice {
        let Bound::Text(text) = value else {
            return Err(FerrioxidError::Validation(format!(
                "column {} does not accept this comparison",
                column.name
            )));
        };
        if !column.choices.is_empty() && !ferrioxid_core::choice_is_known(column.choices, text) {
            return Err(FerrioxidError::Validation(format!(
                "column {} does not accept {text}",
                column.name
            )));
        }
    }
    Ok(bound_value(value))
}

/// Binds a logical temporal value.
///
/// `DateTime` is an absolute UTC instant. `parse_datetime_utc` accepts `Z` and
/// `+00:00` and rejects every other offset instead of converting it. Subsecond
/// precision from the parsed instant is kept in the parameter.
/// PostgreSQL receives `date`, `time` or `timestamptz`. SQLite columns stay
/// text: dates and times use the canonical strings, and a timestamp uses the
/// RFC 3339 text that the SQLite driver writes for `DateTime<Utc>`.
fn bind_temporal(
    column: &ColumnSpec,
    value: &Bound,
    backend: DatabaseBackend,
) -> Result<Value, FerrioxidError> {
    let Bound::Text(text) = value else {
        return Err(FerrioxidError::Validation(format!(
            "column {} does not accept this comparison",
            column.name
        )));
    };
    let invalid = |code: &str| FerrioxidError::Validation(format!("column {} {code}", column.name));
    match column.kind {
        ColumnKind::Date => {
            let date = ferrioxid_core::parse_date(text).map_err(invalid)?;
            Ok(match backend {
                DatabaseBackend::Postgres => Value::ChronoDate(Some(Box::new(date))),
                DatabaseBackend::Sqlite => {
                    Value::String(Some(Box::new(ferrioxid_core::temporal::format_date(date))))
                }
                DatabaseBackend::MySql => {
                    return Err(FerrioxidError::Validation("MySQL is not supported".into()));
                }
            })
        }
        ColumnKind::Time => {
            let time = ferrioxid_core::parse_time(text).map_err(invalid)?;
            Ok(match backend {
                DatabaseBackend::Postgres => Value::ChronoTime(Some(Box::new(time))),
                DatabaseBackend::Sqlite => {
                    Value::String(Some(Box::new(ferrioxid_core::temporal::format_time(time))))
                }
                DatabaseBackend::MySql => {
                    return Err(FerrioxidError::Validation("MySQL is not supported".into()));
                }
            })
        }
        ColumnKind::DateTime => {
            let instant = ferrioxid_core::parse_datetime_utc(text).map_err(invalid)?;
            Ok(match backend {
                DatabaseBackend::Postgres => Value::ChronoDateTimeUtc(Some(Box::new(instant))),
                DatabaseBackend::Sqlite => {
                    Value::String(Some(Box::new(sqlite_datetime_text(instant))))
                }
                DatabaseBackend::MySql => {
                    return Err(FerrioxidError::Validation("MySQL is not supported".into()));
                }
            })
        }
        _ => Err(FerrioxidError::Validation(format!(
            "column {} does not accept this comparison",
            column.name
        ))),
    }
}

fn sqlite_datetime_text(value: chrono::DateTime<chrono::Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, false)
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

fn reject_keyset_orders(
    columns: &[ColumnSpec],
    orders: &[(&str, Direction)],
) -> Result<(), FerrioxidError> {
    for (name, _) in orders {
        let column = lookup(columns, name)?;
        if column.nullable || column.kind == ColumnKind::Boolean {
            return Err(FerrioxidError::Validation(format!(
                "keyset cannot order by {name}"
            )));
        }
    }
    Ok(())
}

fn seek_expr(
    columns: &[ColumnSpec],
    orders: &[(&str, Direction)],
    values: &[Bound],
    backend: DatabaseBackend,
) -> Result<SimpleExpr, FerrioxidError> {
    let mut outer: Option<SimpleExpr> = None;
    for index in 0..orders.len() {
        let mut inner: Option<SimpleExpr> = None;
        for earlier in 0..index {
            let (name, _) = orders[earlier];
            let column = lookup(columns, name)?;
            let equal =
                Expr::col(Alias::new(name)).eq(bind_for_column(column, &values[earlier], backend)?);
            inner = Some(match inner {
                Some(current) => current.and(equal),
                None => equal,
            });
        }
        let (name, direction) = orders[index];
        let column = lookup(columns, name)?;
        let value = bind_for_column(column, &values[index], backend)?;
        let compared = match direction {
            Direction::Asc => Expr::col(Alias::new(name)).gt(value),
            Direction::Desc => Expr::col(Alias::new(name)).lt(value),
        };
        let branch = match inner {
            Some(current) => current.and(compared),
            None => compared,
        };
        outer = Some(match outer {
            Some(current) => current.or(branch),
            None => branch,
        });
    }
    outer.ok_or_else(|| FerrioxidError::Validation("keyset order is empty".into()))
}

#[derive(Debug, Serialize, Deserialize)]
struct CursorToken {
    values: Vec<CursorValue>,
}

#[derive(Debug, Serialize, Deserialize)]
enum CursorValue {
    I64(i64),
    Text(String),
    Bool(bool),
    Uuid(Uuid),
}

fn encode_cursor(
    columns: &[ColumnSpec],
    orders: &[(&str, Direction)],
    row: &Json,
) -> Result<String, FerrioxidError> {
    let object = row.as_object().ok_or_else(|| {
        FerrioxidError::database(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "keyset row is not an object",
        ))
    })?;
    let mut values = Vec::new();
    for (name, _) in orders {
        let column = lookup(columns, name)?;
        let value = object.get(*name).ok_or_else(|| {
            FerrioxidError::Validation(format!("keyset column {name} is missing"))
        })?;
        values.push(cursor_value(column, value)?);
    }
    let json = serde_json::to_vec(&CursorToken { values }).map_err(FerrioxidError::database)?;
    Ok(format!("k1.{}", hex_encode(&json)))
}

fn cursor_value(column: &ColumnSpec, value: &Json) -> Result<CursorValue, FerrioxidError> {
    if value.is_null() {
        return Err(FerrioxidError::Validation(format!(
            "keyset column {} is null",
            column.name
        )));
    }
    match column.kind {
        ColumnKind::BigInt => value
            .as_i64()
            .map(CursorValue::I64)
            .ok_or_else(|| invalid_cursor(column.name)),
        ColumnKind::Boolean => value
            .as_bool()
            .map(CursorValue::Bool)
            .ok_or_else(|| invalid_cursor(column.name)),
        ColumnKind::Uuid => {
            let text = value.as_str().ok_or_else(|| invalid_cursor(column.name))?;
            Uuid::parse_str(text)
                .map(CursorValue::Uuid)
                .map_err(|_| invalid_cursor(column.name))
        }
        ColumnKind::Text
        | ColumnKind::Date
        | ColumnKind::Time
        | ColumnKind::DateTime
        | ColumnKind::Choice
        | ColumnKind::Decimal => value
            .as_str()
            .map(|text| CursorValue::Text(text.to_owned()))
            .ok_or_else(|| invalid_cursor(column.name)),
    }
}

fn invalid_cursor(name: &str) -> FerrioxidError {
    FerrioxidError::Validation(format!("keyset column {name} cannot be continued"))
}

fn decode_cursor(token: &str) -> Result<Vec<Bound>, FerrioxidError> {
    let Some(encoded) = token.strip_prefix("k1.") else {
        return Err(FerrioxidError::Validation(
            "keyset cursor is not valid".into(),
        ));
    };
    let bytes = hex_decode(encoded)
        .map_err(|_| FerrioxidError::Validation("keyset cursor is not valid".into()))?;
    let token: CursorToken = serde_json::from_slice(&bytes)
        .map_err(|_| FerrioxidError::Validation("keyset cursor is not valid".into()))?;
    token
        .values
        .into_iter()
        .map(|value| {
            Ok(match value {
                CursorValue::I64(number) => Bound::I64(number),
                CursorValue::Text(text) => Bound::Text(text),
                CursorValue::Bool(flag) => Bound::Bool(flag),
                CursorValue::Uuid(id) => Bound::Uuid(id),
            })
        })
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(text: &str) -> Result<Vec<u8>, ()> {
    if !text.len().is_multiple_of(2) {
        return Err(());
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_nibble(bytes[index])?;
        let low = hex_nibble(bytes[index + 1])?;
        out.push((high << 4) | low);
        index += 2;
    }
    Ok(out)
}

fn hex_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(()),
    }
}

async fn fetch_pairs<M, C>(
    db: &C,
    query: &SelectStatement,
) -> Result<Vec<(Json, M)>, FerrioxidError>
where
    M: FerrioxidModel + DeserializeOwned,
    C: ConnectionTrait,
{
    note_statement();
    let rows = db
        .query_all(render(db.get_database_backend(), query)?)
        .await
        .map_err(FerrioxidError::database)?;
    let mut pairs = Vec::with_capacity(rows.len());
    for row in rows {
        let json = row_to_json(&row, M::columns())?;
        let model = serde_json::from_value(json.clone()).map_err(FerrioxidError::database)?;
        pairs.push((json, model));
    }
    Ok(pairs)
}

/// Loads models by primary key on `db`, which may be an open transaction.
///
/// At most [`IN_LIMIT`] identifiers are sent per statement. Rows hidden by
/// `scope`, and unknown identifiers, are omitted. The result is ordered by `id`.
/// An empty `ids` slice performs no query. [`QueryScope::None`] performs no query.
pub async fn load_by_ids<M>(
    db: &impl ConnectionTrait,
    ids: &[Uuid],
    scope: &QueryScope,
) -> Result<Vec<M>, FerrioxidError>
where
    M: FerrioxidModel + DeserializeOwned,
{
    load_where::<M>(db, "id", ids, scope).await
}

/// Loads child rows whose `column` is one of `parent_ids`.
///
/// `column` must be a UUID column of `M`. One statement carries at most
/// [`IN_LIMIT`] parent identifiers. The combined rows are ordered by `id`,
/// including when the parent list spans more than one statement.
/// The number of returned child rows is not capped; page a [`Select`] when the
/// child set can be large. [`QueryScope`] is applied and cannot be skipped.
pub async fn load_children<M>(
    db: &impl ConnectionTrait,
    column: &str,
    parent_ids: &[Uuid],
    scope: &QueryScope,
) -> Result<Vec<M>, FerrioxidError>
where
    M: FerrioxidModel + DeserializeOwned,
{
    let spec = lookup(M::columns(), column)?;
    if spec.kind != ColumnKind::Uuid {
        return Err(FerrioxidError::Validation(format!(
            "column {column} is not a uuid column"
        )));
    }
    load_where::<M>(db, column, parent_ids, scope).await
}

/// Groups child rows by parent without a further query.
pub fn group_by_parent<T>(rows: Vec<T>, parent_of: impl Fn(&T) -> Uuid) -> BTreeMap<Uuid, Vec<T>> {
    let mut grouped = BTreeMap::new();
    for row in rows {
        grouped
            .entry(parent_of(&row))
            .or_insert_with(Vec::new)
            .push(row);
    }
    grouped
}

/// Loads many-to-many children through `link`.
///
/// One statement reads at most [`IN_LIMIT`] parent identifiers from the join
/// table. Children are then loaded with [`load_by_ids`] under `scope`. A child
/// hidden by the scope is omitted. The pairs keep join-table order. An empty
/// parent list or [`QueryScope::None`] performs no query.
pub async fn load_many_to_many<Child>(
    db: &impl ConnectionTrait,
    link: ManyToMany,
    parent_ids: &[Uuid],
    scope: &QueryScope,
) -> Result<Vec<(Uuid, Child)>, FerrioxidError>
where
    Child: FerrioxidModel + DeserializeOwned + Serialize + Clone,
{
    if !is_safe_ident(link.join_table)
        || !is_safe_ident(link.parent_column)
        || !is_safe_ident(link.child_column)
    {
        return Err(FerrioxidError::Validation(
            "many-to-many columns are not available".into(),
        ));
    }
    if parent_ids.is_empty() || matches!(scope, QueryScope::None) {
        return Ok(Vec::new());
    }
    let mut parents = parent_ids.to_vec();
    parents.sort();
    parents.dedup();
    let mut links = Vec::new();
    for chunk in parents.chunks(IN_LIMIT) {
        let mut query = Query::select();
        query
            .columns([
                Alias::new(link.parent_column),
                Alias::new(link.child_column),
            ])
            .from(Alias::new(link.join_table))
            .and_where(Expr::col(Alias::new(link.parent_column)).is_in(chunk.iter().copied()))
            .order_by(Alias::new(link.parent_column), Order::Asc)
            .order_by(Alias::new(link.child_column), Order::Asc);
        note_statement();
        let rows = db
            .query_all(render(db.get_database_backend(), &query)?)
            .await
            .map_err(FerrioxidError::database)?;
        for row in rows {
            let parent = read_uuid(&row, link.parent_column)?;
            let child = read_uuid(&row, link.child_column)?;
            links.push((parent, child));
        }
    }
    let child_ids: Vec<Uuid> = links.iter().map(|(_, child)| *child).collect();
    let children = load_by_ids::<Child>(db, &child_ids, scope).await?;
    let mut by_id = BTreeMap::new();
    for child in children {
        let json = serde_json::to_value(&child).map_err(FerrioxidError::database)?;
        by_id.insert(json_id(&json)?, child);
    }
    let mut pairs = Vec::new();
    for (parent, child_id) in links {
        if let Some(child) = by_id.get(&child_id) {
            pairs.push((parent, child.clone()));
        }
    }
    Ok(pairs)
}

fn read_uuid(row: &sea_orm::QueryResult, column: &str) -> Result<Uuid, FerrioxidError> {
    if let Ok(id) = row.try_get::<Uuid>("", column) {
        return Ok(id);
    }
    let text: String = row.try_get("", column).map_err(FerrioxidError::database)?;
    Uuid::parse_str(&text).map_err(FerrioxidError::database)
}

/// Reads one model on `db` and applies `scope`. A hidden row is `Ok(None)`.
pub async fn read_model<M>(
    db: &impl ConnectionTrait,
    id: Uuid,
    scope: &QueryScope,
) -> Result<Option<M>, FerrioxidError>
where
    M: FerrioxidModel + DeserializeOwned,
{
    note_statement();
    let row = crate::sql::get_record(db, M::descriptor().table, M::columns(), id, scope).await?;
    row.map(|json| serde_json::from_value(json).map_err(FerrioxidError::database))
        .transpose()
}

async fn load_where<M>(
    db: &impl ConnectionTrait,
    column: &str,
    ids: &[Uuid],
    scope: &QueryScope,
) -> Result<Vec<M>, FerrioxidError>
where
    M: FerrioxidModel + DeserializeOwned,
{
    let columns = M::columns();
    let table = M::descriptor().table;
    if !is_safe_ident(table) {
        return Err(FerrioxidError::Validation(format!(
            "table {table} is not available"
        )));
    }
    lookup(columns, column)?;
    lookup(columns, "id")?;
    if ids.is_empty() || matches!(scope, QueryScope::None) {
        return Ok(Vec::new());
    }
    let mut unique = ids.to_vec();
    unique.sort();
    unique.dedup();
    let mut found = Vec::new();
    for chunk in unique.chunks(IN_LIMIT) {
        let mut query = select_body::<M>(None);
        query.and_where(Expr::col(Alias::new(column)).is_in(chunk.iter().copied()));
        if let Some(condition) = scope_condition(columns, scope)? {
            query.and_where(condition);
        }
        query.order_by(Alias::new("id"), Order::Asc);
        found.extend(fetch_json::<M, _>(db, &query).await?);
    }
    let mut keyed = Vec::with_capacity(found.len());
    for json in found {
        let id = json_id(&json)?;
        keyed.push((id, json));
    }
    keyed.sort_by_key(|row| row.0);
    keyed
        .into_iter()
        .map(|(_, json)| serde_json::from_value(json).map_err(FerrioxidError::database))
        .collect()
}

async fn fetch_json<M, C>(
    db: &C,
    query: &SelectStatement,
) -> Result<Vec<serde_json::Value>, FerrioxidError>
where
    M: FerrioxidModel,
    C: ConnectionTrait,
{
    note_statement();
    let rows = db
        .query_all(render(db.get_database_backend(), query)?)
        .await
        .map_err(FerrioxidError::database)?;
    rows.into_iter()
        .map(|row| row_to_json(&row, M::columns()))
        .collect()
}

fn json_id(json: &serde_json::Value) -> Result<Uuid, FerrioxidError> {
    let text = json
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            FerrioxidError::database(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "row has no id",
            ))
        })?;
    Uuid::parse_str(text).map_err(FerrioxidError::database)
}

pub(crate) fn note_statement() {
    let enabled = cfg!(test) || COUNTING.with(|flag| flag.get());
    if enabled {
        STATEMENT_COUNT.with(|count| count.set(count.get().saturating_add(1)));
    }
}

thread_local! {
    static STATEMENT_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Counts model statements on this thread while it is alive.
///
/// `all`, `count`, `exists`, `page`, `keyset`, aggregates and the batch loaders
/// call [`QueryCounter::record`] for their own statements. A direct
/// `ConnectionTrait` query does not. Install the counter in application tests;
/// library unit tests already count. The value is thread-local and is not a
/// pool-wide total.
pub struct QueryCounter {
    previous: bool,
}

impl QueryCounter {
    pub fn install() -> Self {
        let previous = COUNTING.with(|flag| flag.replace(true));
        STATEMENT_COUNT.with(|count| count.set(0));
        Self { previous }
    }

    pub fn reset() {
        STATEMENT_COUNT.with(|count| count.set(0));
    }

    pub fn get() -> usize {
        STATEMENT_COUNT.with(|count| count.get())
    }

    /// Counts one statement the caller executed outside the model query API.
    ///
    /// Direct `ConnectionTrait` calls are not observed. This does not make the
    /// counter a complete trace of every SQL statement on the connection.
    pub fn record() {
        note_statement();
    }
}

impl Drop for QueryCounter {
    fn drop(&mut self) {
        COUNTING.with(|flag| flag.set(self.previous));
    }
}

#[cfg(test)]
impl<M> Select<M>
where
    M: FerrioxidModel,
{
    pub(crate) fn statement_for(
        &self,
        backend: DatabaseBackend,
    ) -> Result<Statement, FerrioxidError> {
        let plan = self.plan(backend)?;
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
    use sea_orm::{
        ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait,
    };
    use serde::{Deserialize, Serialize};

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
            precision: 0,
            scale: 0,
            choices: &[],
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
                    {
                        let mut category = column("category_id", ColumnKind::Uuid, true, false);
                        category.foreign_table = Some("ferrioxid_query_categories");
                        category.foreign_column = Some("id");
                        category
                    },
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

    async fn fixture(db: &impl ConnectionTrait) {
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

    struct Counting {
        inner: DatabaseConnection,
        queries: std::sync::atomic::AtomicUsize,
        sql: std::sync::Mutex<Vec<String>>,
    }

    impl Counting {
        fn new(inner: DatabaseConnection) -> Self {
            Self {
                inner,
                queries: std::sync::atomic::AtomicUsize::new(0),
                sql: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn record(&self, statement: &Statement) {
            self.queries
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.sql
                .lock()
                .expect("query log")
                .push(statement.sql.clone());
        }
    }

    #[async_trait::async_trait]
    impl ConnectionTrait for Counting {
        fn get_database_backend(&self) -> DatabaseBackend {
            self.inner.get_database_backend()
        }

        async fn execute(
            &self,
            statement: Statement,
        ) -> Result<sea_orm::ExecResult, sea_orm::DbErr> {
            self.record(&statement);
            self.inner.execute(statement).await
        }

        async fn execute_unprepared(
            &self,
            sql: &str,
        ) -> Result<sea_orm::ExecResult, sea_orm::DbErr> {
            self.queries
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.inner.execute_unprepared(sql).await
        }

        async fn query_one(
            &self,
            statement: Statement,
        ) -> Result<Option<sea_orm::QueryResult>, sea_orm::DbErr> {
            self.record(&statement);
            self.inner.query_one(statement).await
        }

        async fn query_all(
            &self,
            statement: Statement,
        ) -> Result<Vec<sea_orm::QueryResult>, sea_orm::DbErr> {
            self.record(&statement);
            self.inner.query_all(statement).await
        }
    }

    #[tokio::test]
    async fn batch_load_respects_scope_and_does_not_query_per_row() {
        let db = Counting::new(memory().await);
        fixture(&db).await;
        db.queries.store(0, std::sync::atomic::Ordering::Relaxed);
        db.sql.lock().expect("query log").clear();

        let ids = [
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            Uuid::from_u128(4),
            Uuid::from_u128(5),
            Uuid::from_u128(99),
        ];
        let visible = load_by_ids::<Item>(&db, &ids, &QueryScope::All)
            .await
            .unwrap();
        assert_eq!(visible.len(), 5);
        assert_eq!(db.queries.load(std::sync::atomic::Ordering::Relaxed), 1);
        let sql = db.sql.lock().expect("query log").join("\n");
        assert!(sql.contains("IN"));
        assert!(sql.contains('?'));
        assert!(!sql.contains("DROP"));
        assert!(!sql.contains(PAYLOAD));

        db.queries.store(0, std::sync::atomic::Ordering::Relaxed);
        let owned = load_by_ids::<Item>(
            &db,
            &ids,
            &QueryScope::Equals {
                column: "owner_id",
                value: ScopeValue::Uuid(Uuid::from_u128(7)),
            },
        )
        .await
        .unwrap();
        assert_eq!(names(&owned), vec!["alpha", "beta", PAYLOAD]);
        assert_eq!(db.queries.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(
            read_model::<Item>(&db, Uuid::from_u128(5), &QueryScope::None)
                .await
                .unwrap()
                .is_none()
        );

        db.queries.store(0, std::sync::atomic::Ordering::Relaxed);
        let denied = load_by_ids::<Item>(&db, &ids, &QueryScope::None)
            .await
            .unwrap();
        assert!(denied.is_empty());
        assert_eq!(db.queries.load(std::sync::atomic::Ordering::Relaxed), 0);

        let mut many = Vec::new();
        for number in 1_u128..=101 {
            many.push(Uuid::from_u128(number));
        }
        db.queries.store(0, std::sync::atomic::Ordering::Relaxed);
        let _ = load_by_ids::<Item>(&db, &many, &QueryScope::All)
            .await
            .unwrap();
        assert_eq!(db.queries.load(std::sync::atomic::Ordering::Relaxed), 2);

        db.queries.store(0, std::sync::atomic::Ordering::Relaxed);
        let children =
            load_children::<Item>(&db, "owner_id", &[Uuid::from_u128(8)], &QueryScope::All)
                .await
                .unwrap();
        assert_eq!(names(&children), vec!["gamma", "hidden"]);
        assert_eq!(db.queries.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(
            load_children::<Item>(&db, "name", &[Uuid::from_u128(8)], &QueryScope::All)
                .await
                .is_err()
        );
        assert!(
            load_children::<Item>(&db, "missing", &[Uuid::from_u128(8)], &QueryScope::All)
                .await
                .is_err()
        );

        let mut cart_lines = Vec::new();
        for number in 10_u128..30 {
            let name = format!("line-{number}");
            insert(
                &db,
                NewItem {
                    id: number,
                    name: &name,
                    stock: 1,
                    active: true,
                    price: 10,
                    category: Some(50),
                    owner: 7,
                },
            )
            .await;
            cart_lines.push(Uuid::from_u128(number));
        }
        db.queries.store(0, std::sync::atomic::Ordering::Relaxed);
        let products = load_by_ids::<Item>(&db, &cart_lines, &QueryScope::All)
            .await
            .unwrap();
        assert_eq!(products.len(), 20);
        assert_eq!(db.queries.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn load_children_orders_interleaved_ids_across_parent_chunks() {
        let db = Counting::new(memory().await);
        let mut parents = Vec::new();
        for parent in 1_u128..=101 {
            parents.push(Uuid::from_u128(parent));
        }
        let mut rows = Vec::new();
        rows.push((200_u128, 1_u128));
        for parent in 2_u128..=100 {
            rows.push((parent, parent));
        }
        rows.push((1, 101));
        rows.push((150, 101));
        for (child, parent) in &rows {
            let name = format!("child-{child}");
            insert(
                &db,
                NewItem {
                    id: *child,
                    name: &name,
                    stock: 1,
                    active: true,
                    price: 1,
                    category: None,
                    owner: *parent,
                },
            )
            .await;
        }
        db.queries.store(0, std::sync::atomic::Ordering::Relaxed);
        let loaded = load_children::<Item>(&db, "owner_id", &parents, &QueryScope::All)
            .await
            .unwrap();
        assert_eq!(db.queries.load(std::sync::atomic::Ordering::Relaxed), 2);
        let ids: Vec<_> = loaded.iter().map(|row| row.id).collect();
        let mut expected: Vec<_> = rows
            .iter()
            .map(|(child, _)| Uuid::from_u128(*child))
            .collect();
        expected.sort();
        assert_eq!(ids, expected);
        assert_eq!(loaded[0].id, Uuid::from_u128(1));
        assert_eq!(loaded[0].owner_id, Uuid::from_u128(101));
        assert_eq!(loaded[100].id, Uuid::from_u128(150));
        assert_eq!(loaded[100].owner_id, Uuid::from_u128(101));
        assert_eq!(loaded[101].id, Uuid::from_u128(200));
        assert_eq!(loaded[101].owner_id, Uuid::from_u128(1));
    }

    #[tokio::test]
    async fn sqlite_transaction_reads_writes_and_rolls_back_on_one_connection() {
        let db = memory().await;
        fixture(&db).await;
        let finished = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let tx = db.begin().await.unwrap();
            let found = Item::query()
                .filter(col("active").eq(true))
                .filter(col("stock").gt(0_i64))
                .all(&tx)
                .await
                .unwrap();
            assert_eq!(found.len(), 3);
            let children = crate::find_by_column::<Item>(
                &tx,
                "owner_id",
                Value::Uuid(Some(Box::new(Uuid::from_u128(7)))),
            )
            .await
            .unwrap();
            assert_eq!(children.len(), 3);
            insert(
                &tx,
                NewItem {
                    id: 6,
                    name: "order-line",
                    stock: 1,
                    active: true,
                    price: 12,
                    category: Some(50),
                    owner: 7,
                },
            )
            .await;
            let updated = crate::sql::update_record(
                &tx,
                Item::descriptor().table,
                Item::columns(),
                Uuid::from_u128(2),
                vec![("stock", Value::BigInt(Some(9)))],
                &QueryScope::All,
            )
            .await
            .unwrap();
            let stock = updated.get("stock").and_then(|value| value.as_i64());
            assert_eq!(stock, Some(9));
            let during = Item::query()
                .filter(col("name").eq("order-line"))
                .exists(&tx)
                .await
                .unwrap();
            assert!(during);
            tx.rollback().await.unwrap();
        })
        .await;
        finished.expect("sqlite transaction waited for a second pool connection");
        assert!(Item::query()
            .filter(col("name").eq("order-line"))
            .all(&db)
            .await
            .unwrap()
            .is_empty());
        let beta = read_model::<Item>(&db, Uuid::from_u128(2), &QueryScope::All)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(beta.stock, 4);

        let tx = db.begin().await.unwrap();
        insert(
            &tx,
            NewItem {
                id: 6,
                name: "kept",
                stock: 1,
                active: true,
                price: 12,
                category: Some(50),
                owner: 7,
            },
        )
        .await;
        tx.commit().await.unwrap();
        assert_eq!(
            Item::query()
                .filter(col("name").eq("kept"))
                .count(&db)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
    async fn postgres_transaction_query_and_rollback_runtime() {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run the ignored PostgreSQL transaction test");
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
        let tx = db.begin().await.unwrap();
        insert(
            &tx,
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
        assert!(Item::query().exists(&tx).await.unwrap());
        tx.rollback().await.unwrap();
        assert_eq!(Item::query().count(&db).await.unwrap(), 0);
        db.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "DROP TABLE IF EXISTS ferrioxid_query_items".to_owned(),
        ))
        .await
        .unwrap();
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct Tag {
        id: Uuid,
        item_id: Uuid,
        amount: i64,
        visible: bool,
    }

    const TAG_DESCRIPTOR: ModelDescriptor = ModelDescriptor {
        name: "Tag",
        table: "ferrioxid_query_tags",
        api_path: "",
        allow_anonymous: false,
        permission: None,
        requires_object_policy: false,
    };

    impl FerrioxidModel for Tag {
        fn descriptor() -> &'static ModelDescriptor {
            &TAG_DESCRIPTOR
        }

        fn columns() -> &'static [ColumnSpec] {
            use std::sync::OnceLock;
            static COLUMNS: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            COLUMNS.get_or_init(|| {
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("item_id", ColumnKind::Uuid, false, false),
                    column("amount", ColumnKind::BigInt, false, false),
                    column("visible", ColumnKind::Boolean, false, false),
                ]
            })
        }

        fn schema_sql(kind: DatabaseKind) -> String {
            match kind {
                DatabaseKind::Sqlite => "CREATE TABLE ferrioxid_query_tags (
                    id text PRIMARY KEY NOT NULL,
                    item_id text NOT NULL,
                    amount bigint NOT NULL,
                    visible integer NOT NULL
                )"
                .to_owned(),
                DatabaseKind::Postgres => "CREATE TABLE ferrioxid_query_tags (
                    id uuid PRIMARY KEY NOT NULL,
                    item_id uuid NOT NULL,
                    amount bigint NOT NULL,
                    visible boolean NOT NULL
                )"
                .to_owned(),
            }
        }
    }

    fn tag_sql(kind: DatabaseKind) -> String {
        let id = match kind {
            DatabaseKind::Sqlite => "text",
            DatabaseKind::Postgres => "uuid",
        };
        format!(
            "CREATE TABLE ferrioxid_query_membership (
                item_id {id} NOT NULL,
                tag_id {id} NOT NULL,
                PRIMARY KEY (item_id, tag_id)
            )"
        )
    }

    async fn prepare_relations(db: &impl ConnectionTrait, kind: DatabaseKind) {
        let backend = match kind {
            DatabaseKind::Sqlite => DatabaseBackend::Sqlite,
            DatabaseKind::Postgres => DatabaseBackend::Postgres,
        };
        db.execute(Statement::from_string(backend, Tag::schema_sql(kind)))
            .await
            .unwrap();
        db.execute(Statement::from_string(backend, tag_sql(kind)))
            .await
            .unwrap();
        for (id, item, amount, visible) in
            [(10_u128, 1_u128, 4_i64, 1), (11, 1, 6, 1), (12, 2, 3, 0)]
        {
            insert_tag(db, kind, id, item, amount, visible).await;
        }
        for (item, tag) in [(1_u128, 10_u128), (1, 11), (2, 12)] {
            link_tag(db, kind, item, tag).await;
        }
    }

    async fn insert_tag(
        db: &impl ConnectionTrait,
        kind: DatabaseKind,
        id: u128,
        item: u128,
        amount: i64,
        visible: i64,
    ) {
        let _ = kind;
        let mut query = Query::insert();
        query
            .into_table(Alias::new("ferrioxid_query_tags"))
            .columns([
                Alias::new("id"),
                Alias::new("item_id"),
                Alias::new("amount"),
                Alias::new("visible"),
            ])
            .values_panic([
                Value::Uuid(Some(Box::new(Uuid::from_u128(id)))).into(),
                Value::Uuid(Some(Box::new(Uuid::from_u128(item)))).into(),
                Value::BigInt(Some(amount)).into(),
                Value::Bool(Some(visible == 1)).into(),
            ]);
        db.execute(render(db.get_database_backend(), &query).unwrap())
            .await
            .unwrap();
    }

    async fn link_tag(db: &impl ConnectionTrait, kind: DatabaseKind, item: u128, tag: u128) {
        let _ = kind;
        let mut query = Query::insert();
        query
            .into_table(Alias::new("ferrioxid_query_membership"))
            .columns([Alias::new("item_id"), Alias::new("tag_id")])
            .values_panic([
                Value::Uuid(Some(Box::new(Uuid::from_u128(item)))).into(),
                Value::Uuid(Some(Box::new(Uuid::from_u128(tag)))).into(),
            ]);
        db.execute(render(db.get_database_backend(), &query).unwrap())
            .await
            .unwrap();
    }

    async fn assert_relations(db: &impl ConnectionTrait) {
        let page = Item::query()
            .order_by("price_cents", Direction::Asc)
            .limit(2)
            .keyset(db, None)
            .await
            .unwrap();
        assert_eq!(page.data.len(), 2);
        assert!(page.has_more);
        let again = Item::query()
            .order_by("price_cents", Direction::Asc)
            .limit(2)
            .keyset(db, page.next.as_deref())
            .await
            .unwrap();
        let mut seen = page.data.iter().map(|row| row.id).collect::<Vec<_>>();
        seen.extend(again.data.iter().map(|row| row.id));
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), page.data.len() + again.data.len());
        let rejected = Item::query()
            .order_by("category_id", Direction::Asc)
            .keyset(db, None)
            .await;
        assert!(rejected.is_err());

        let counts = Item::query()
            .filter(col("id").is_in(vec![
                Uuid::from_u128(1),
                Uuid::from_u128(2),
                Uuid::from_u128(3),
            ]))
            .annotate(
                db,
                Aggregate::<Tag>::count("tags", "item_id"),
                Some(Direction::Desc),
            )
            .await
            .unwrap();
        assert_eq!(counts[0].row.id, Uuid::from_u128(1));
        assert_eq!(counts[0].value, 2);
        assert_eq!(counts[2].value, 0);
        let sums = Item::query()
            .filter(col("id").eq(Uuid::from_u128(1)))
            .annotate(
                db,
                Aggregate::<Tag>::sum("amount", "item_id", "amount").scope(QueryScope::Equals {
                    column: "visible",
                    value: ScopeValue::Bool(true),
                }),
                None,
            )
            .await
            .unwrap();
        assert_eq!(sums[0].value, 10);

        STATEMENT_COUNT.with(|count| count.set(0));
        let parents = [Uuid::from_u128(1), Uuid::from_u128(2)];
        let children = load_children::<Tag>(db, "item_id", &parents, &QueryScope::All)
            .await
            .unwrap();
        let grouped = group_by_parent(children, |tag| tag.item_id);
        assert_eq!(grouped.get(&Uuid::from_u128(1)).unwrap().len(), 2);
        let pairs = load_many_to_many::<Tag>(
            db,
            ManyToMany {
                join_table: "ferrioxid_query_membership",
                parent_column: "item_id",
                child_column: "tag_id",
            },
            &parents,
            &QueryScope::All,
        )
        .await
        .unwrap();
        assert_eq!(pairs.len(), 3);
        let hidden = load_many_to_many::<Tag>(
            db,
            ManyToMany {
                join_table: "ferrioxid_query_membership",
                parent_column: "item_id",
                child_column: "tag_id",
            },
            &parents,
            &QueryScope::Equals {
                column: "visible",
                value: ScopeValue::Bool(true),
            },
        )
        .await
        .unwrap();
        assert_eq!(hidden.len(), 2);
        let statements = STATEMENT_COUNT.with(|count| count.get());
        assert!(
            statements <= 6,
            "relation loads issued {statements} statements"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sqlite_keyset_annotations_and_many_to_many() {
        let db = memory().await;
        fixture(&db).await;
        prepare_relations(&db, DatabaseKind::Sqlite).await;
        assert_relations(&db).await;
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
    async fn postgres_keyset_annotations_and_many_to_many() {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run the ignored PostgreSQL relation test");
        let db = connect(&url).await.expect("postgres connection");
        for sql in [
            "DROP TABLE IF EXISTS ferrioxid_query_membership",
            "DROP TABLE IF EXISTS ferrioxid_query_tags",
            "DROP TABLE IF EXISTS ferrioxid_query_items",
        ] {
            db.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                sql.to_owned(),
            ))
            .await
            .unwrap();
        }
        db.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            Item::schema_sql(DatabaseKind::Postgres),
        ))
        .await
        .unwrap();
        fixture(&db).await;
        prepare_relations(&db, DatabaseKind::Postgres).await;
        assert_relations(&db).await;
        for sql in [
            "DROP TABLE IF EXISTS ferrioxid_query_membership",
            "DROP TABLE IF EXISTS ferrioxid_query_tags",
            "DROP TABLE IF EXISTS ferrioxid_query_items",
        ] {
            db.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                sql.to_owned(),
            ))
            .await
            .unwrap();
        }
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct Category {
        id: Uuid,
        name: String,
    }

    const CATEGORY_DESCRIPTOR: ModelDescriptor = ModelDescriptor {
        name: "Category",
        table: "ferrioxid_query_categories",
        api_path: "",
        allow_anonymous: false,
        permission: None,
        requires_object_policy: false,
    };

    impl FerrioxidModel for Category {
        fn descriptor() -> &'static ModelDescriptor {
            &CATEGORY_DESCRIPTOR
        }

        fn columns() -> &'static [ColumnSpec] {
            use std::sync::OnceLock;
            static COLUMNS: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            COLUMNS.get_or_init(|| {
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("name", ColumnKind::Text, false, false),
                ]
            })
        }

        fn schema_sql(kind: DatabaseKind) -> String {
            match kind {
                DatabaseKind::Sqlite => {
                    "CREATE TABLE ferrioxid_query_categories (id text PRIMARY KEY NOT NULL, name text NOT NULL)"
                        .to_owned()
                }
                DatabaseKind::Postgres => {
                    "CREATE TABLE ferrioxid_query_categories (id uuid PRIMARY KEY NOT NULL, name text NOT NULL)"
                        .to_owned()
                }
            }
        }
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct Note {
        id: Uuid,
        item_id: Uuid,
        body: String,
    }

    const NOTE_DESCRIPTOR: ModelDescriptor = ModelDescriptor {
        name: "Note",
        table: "ferrioxid_query_notes",
        api_path: "",
        allow_anonymous: false,
        permission: None,
        requires_object_policy: false,
    };

    impl FerrioxidModel for Note {
        fn descriptor() -> &'static ModelDescriptor {
            &NOTE_DESCRIPTOR
        }

        fn columns() -> &'static [ColumnSpec] {
            use std::sync::OnceLock;
            static COLUMNS: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            COLUMNS.get_or_init(|| {
                let mut item = column("item_id", ColumnKind::Uuid, false, false);
                item.foreign_table = Some("ferrioxid_query_items");
                item.foreign_column = Some("id");
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    item,
                    column("body", ColumnKind::Text, false, false),
                ]
            })
        }

        fn schema_sql(kind: DatabaseKind) -> String {
            match kind {
                DatabaseKind::Sqlite => {
                    "CREATE TABLE ferrioxid_query_notes (id text PRIMARY KEY NOT NULL, item_id text NOT NULL, body text NOT NULL)"
                        .to_owned()
                }
                DatabaseKind::Postgres => {
                    "CREATE TABLE ferrioxid_query_notes (id uuid PRIMARY KEY NOT NULL, item_id uuid NOT NULL, body text NOT NULL)"
                        .to_owned()
                }
            }
        }
    }

    #[test]
    fn fold_case_insensitive_pattern_matches_backend_lower_rules() {
        let pattern = format!("%{}%", escape_like("MÜNCHEN"));
        assert_eq!(
            super::fold_case_insensitive_pattern(pattern.clone(), DatabaseBackend::Sqlite),
            "%mÜnchen%"
        );
        assert_eq!(
            super::fold_case_insensitive_pattern(pattern, DatabaseBackend::Postgres),
            "%münchen%"
        );
    }

    #[test]
    fn text_search_sql_is_bound_and_backend_specific() {
        let select = Item::query().filter(col("name").contains("%_injection"));
        let sqlite = select.statement_for(DatabaseBackend::Sqlite).unwrap();
        let postgres = select.statement_for(DatabaseBackend::Postgres).unwrap();
        assert!(sqlite.sql.contains("GLOB"));
        assert!(!postgres.sql.contains("LIKE ?"));
        assert!(postgres.sql.contains("$1"));
        assert!(!sqlite.sql.contains("LIKE"));
        assert!(postgres.sql.contains("LIKE"));
        assert!(postgres.sql.contains("ESCAPE"));
        assert!(!sqlite.sql.contains("%_injection"));
        assert!(!postgres.sql.contains("%_injection"));
        assert!(!sqlite.sql.contains("DROP"));
        let folded = Item::query()
            .filter(col("name").contains_ignore_case("Beta"))
            .statement_for(DatabaseBackend::Sqlite)
            .unwrap();
        assert!(folded.sql.contains("LOWER("));
        assert!(folded.sql.contains("ESCAPE"));
        let rejected = Item::query()
            .filter(col("stock").contains("1"))
            .statement_for(DatabaseBackend::Sqlite)
            .unwrap_err();
        assert!(rejected.to_string().contains("does not accept"));
        let long = "a".repeat(TEXT_MATCH_LIMIT + 1);
        let limited = Item::query()
            .filter(col("name").contains(long))
            .statement_for(DatabaseBackend::Sqlite)
            .unwrap_err();
        assert!(limited.to_string().contains("200"));
    }

    #[tokio::test]
    async fn sqlite_text_search_relations_count_and_pages() {
        let db = memory().await;
        assert_text_search(&db).await;
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
    async fn postgres_text_search_relations_count_and_pages() {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run the ignored PostgreSQL text-search test");
        let db = connect(&url).await.expect("postgres connection");
        for sql in [
            "DROP TABLE IF EXISTS ferrioxid_query_item_categories",
            "DROP TABLE IF EXISTS ferrioxid_query_notes",
            "DROP TABLE IF EXISTS ferrioxid_query_categories",
            "DROP TABLE IF EXISTS ferrioxid_query_items",
        ] {
            db.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                sql.to_owned(),
            ))
            .await
            .unwrap();
        }
        db.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            Item::schema_sql(DatabaseKind::Postgres),
        ))
        .await
        .unwrap();
        assert_text_search(&db).await;
        for sql in [
            "DROP TABLE IF EXISTS ferrioxid_query_item_categories",
            "DROP TABLE IF EXISTS ferrioxid_query_notes",
            "DROP TABLE IF EXISTS ferrioxid_query_categories",
            "DROP TABLE IF EXISTS ferrioxid_query_items",
        ] {
            db.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                sql.to_owned(),
            ))
            .await
            .unwrap();
        }
    }

    async fn assert_text_search(db: &DatabaseConnection) {
        let kind = match db.get_database_backend() {
            DatabaseBackend::Sqlite => DatabaseKind::Sqlite,
            DatabaseBackend::Postgres => DatabaseKind::Postgres,
            DatabaseBackend::MySql => panic!("MySQL is not supported"),
        };
        for sql in [
            Category::schema_sql(kind),
            Note::schema_sql(kind),
            match kind {
                DatabaseKind::Sqlite => {
                    "CREATE TABLE ferrioxid_query_item_categories (item_id text NOT NULL, category_id text NOT NULL)".to_owned()
                }
                DatabaseKind::Postgres => {
                    "CREATE TABLE ferrioxid_query_item_categories (item_id uuid NOT NULL, category_id uuid NOT NULL)".to_owned()
                }
            },
        ] {
            db.execute(Statement::from_string(
                db.get_database_backend(),
                sql.to_owned(),
            ))
            .await
            .unwrap();
        }
        for (id, name, category, owner) in [
            (1_u128, "Beta", Some(50_u128), 7_u128),
            (2, "beta", Some(51), 7),
            (3, "100%_off", Some(50), 8),
            (4, "100XYoff", None, 7),
            (5, "a*b", None, 7),
        ] {
            insert(
                db,
                NewItem {
                    id,
                    name,
                    stock: 1,
                    active: true,
                    price: i64::try_from(id).unwrap(),
                    category,
                    owner,
                },
            )
            .await;
        }
        let backend = db.get_database_backend();
        let (categories_sql, notes_sql, item_categories_sql) = match backend {
            DatabaseBackend::Postgres => (
                "INSERT INTO ferrioxid_query_categories (id, name) VALUES ($1, $2), ($3, $4)",
                "INSERT INTO ferrioxid_query_notes (id, item_id, body) VALUES ($1, $2, $3)",
                "INSERT INTO ferrioxid_query_item_categories (item_id, category_id) VALUES ($1, $2)",
            ),
            _ => (
                "INSERT INTO ferrioxid_query_categories (id, name) VALUES (?, ?), (?, ?)",
                "INSERT INTO ferrioxid_query_notes (id, item_id, body) VALUES (?, ?, ?)",
                "INSERT INTO ferrioxid_query_item_categories (item_id, category_id) VALUES (?, ?)",
            ),
        };
        db.execute(Statement::from_sql_and_values(
            backend,
            categories_sql,
            [
                uuid_value(50),
                text_value("Tools"),
                uuid_value(51),
                text_value("Food"),
            ],
        ))
        .await
        .unwrap();
        db.execute(Statement::from_sql_and_values(
            backend,
            notes_sql,
            [uuid_value(90), uuid_value(1), text_value("sharp edge")],
        ))
        .await
        .unwrap();
        db.execute(Statement::from_sql_and_values(
            backend,
            item_categories_sql,
            [uuid_value(4), uuid_value(50)],
        ))
        .await
        .unwrap();

        let literal = Item::query()
            .filter(col("name").contains("100%_"))
            .order_by("name", Direction::Asc)
            .all(db)
            .await
            .unwrap();
        assert_eq!(names(&literal), vec!["100%_off"]);

        let sensitive = Item::query()
            .filter(col("name").contains("Beta"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(names(&sensitive), vec!["Beta"]);

        let folded = Item::query()
            .filter(col("name").contains_ignore_case("BETA"))
            .order_by("id", Direction::Asc)
            .all(db)
            .await
            .unwrap();
        assert_eq!(names(&folded), vec!["Beta", "beta"]);

        let prefix = Item::query()
            .filter(col("name").starts_with("100"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(prefix, 2);
        let suffix = Item::query()
            .filter(col("name").ends_with("off"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(suffix, 2);
        let glob = Item::query()
            .filter(col("name").contains("a*b"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(names(&glob), vec!["a*b"]);

        let scoped = Item::query()
            .filter(col("name").contains_ignore_case("beta"))
            .scope(QueryScope::Equals {
                column: "owner_id",
                value: ScopeValue::Uuid(Uuid::from_u128(8)),
            })
            .count(db)
            .await
            .unwrap();
        assert_eq!(scoped, 0);

        let page = Item::query()
            .filter(col("name").starts_with("100"))
            .order_by("name", Direction::Asc)
            .limit(1)
            .page(db)
            .await
            .unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.data.len(), 1);
        assert!(page.has_more);

        let first = Item::query()
            .filter(col("name").ends_with("off"))
            .order_by("name", Direction::Asc)
            .limit(1)
            .keyset(db, None)
            .await
            .unwrap();
        assert_eq!(first.data.len(), 1);
        assert!(first.has_more);
        let second = Item::query()
            .filter(col("name").ends_with("off"))
            .order_by("name", Direction::Asc)
            .limit(1)
            .keyset(db, first.next.as_deref())
            .await
            .unwrap();
        assert_eq!(second.data.len(), 1);
        assert_ne!(first.data[0].id, second.data[0].id);

        let tools = Item::query()
            .where_related(
                "category_id",
                Related::<Category>::new().filter(col("name").contains("Tool")),
            )
            .order_by("name", Direction::Asc)
            .all(db)
            .await
            .unwrap();
        assert_eq!(names(&tools), vec!["100%_off", "Beta"]);

        let noted = Item::query()
            .where_has(
                "item_id",
                Related::<Note>::new().filter(col("body").contains("sharp")),
            )
            .all(db)
            .await
            .unwrap();
        assert_eq!(names(&noted), vec!["Beta"]);

        let linked = Item::query()
            .where_linked(
                ManyToMany {
                    join_table: "ferrioxid_query_item_categories",
                    parent_column: "item_id",
                    child_column: "category_id",
                },
                Related::<Category>::new().filter(col("name").eq("Tools")),
            )
            .all(db)
            .await
            .unwrap();
        assert_eq!(names(&linked), vec!["100XYoff"]);

        let not_food = Item::query()
            .where_related(
                "category_id",
                Related::<Category>::new()
                    .filter(col("name").eq("Food"))
                    .negated(),
            )
            .count(db)
            .await
            .unwrap();
        assert_eq!(not_food, 4);
        let empty = Item::query()
            .filter(col("name").contains("missing-term"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(empty, 0);
        let prefixed = Item::query()
            .filter(col("name").starts_with_ignore_case("100%"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(prefixed, 1);
        let suffixed = Item::query()
            .filter(col("name").ends_with_ignore_case("off"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(suffixed, 2);
        insert(
            db,
            NewItem {
                id: 6,
                name: "München",
                stock: 1,
                active: true,
                price: 6,
                category: Some(50),
                owner: 7,
            },
        )
        .await;
        let unicode = Item::query()
            .filter(col("name").contains("Münch"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(unicode, 1);
        match db.get_database_backend() {
            DatabaseBackend::Sqlite => {
                let folded = Item::query()
                    .filter(col("name").contains_ignore_case("MÜNCHEN"))
                    .count(db)
                    .await
                    .unwrap();
                assert_eq!(folded, 0, "SQLite LOWER folds ASCII only");
                let ascii = Item::query()
                    .filter(col("name").contains_ignore_case("Münch"))
                    .count(db)
                    .await
                    .unwrap();
                assert_eq!(ascii, 1, "ASCII letters still match on SQLite");
            }
            DatabaseBackend::Postgres => {
                let folded = Item::query()
                    .filter(col("name").contains_ignore_case("münchen"))
                    .count(db)
                    .await
                    .unwrap();
                assert_eq!(folded, 1, "PostgreSQL LOWER folds Unicode letters");
                let upper_term = Item::query()
                    .filter(col("name").contains_ignore_case("MÜNCHEN"))
                    .count(db)
                    .await
                    .unwrap();
                assert_eq!(upper_term, 1, "Unicode upper search term matches München");
            }
            DatabaseBackend::MySql => {}
        }
        let combined = Item::query()
            .filter(col("name").contains_ignore_case("beta"))
            .where_related(
                "category_id",
                Related::<Category>::new().filter(col("name").eq("Tools")),
            )
            .limit(10)
            .page(db)
            .await
            .unwrap();
        assert_eq!(combined.total, 1);

        let missing = Item::query()
            .where_related("name", Related::<Category>::new())
            .all(db)
            .await
            .unwrap_err();
        assert!(missing.to_string().contains("not a foreign key"));
    }

    #[tokio::test]
    async fn query_counter_does_not_see_direct_statements_or_other_threads() {
        let db = memory().await;
        let _counter = QueryCounter::install();
        let before = QueryCounter::get();
        let counted = Item::query().count(&db).await.unwrap();
        assert_eq!(counted, 0);
        assert!(QueryCounter::get() > before);
        let after_model = QueryCounter::get();
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT 1".to_owned(),
        ))
        .await
        .unwrap();
        assert_eq!(QueryCounter::get(), after_model);
        let transaction = db.begin().await.unwrap();
        transaction
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT 1".to_owned(),
            ))
            .await
            .unwrap();
        transaction.rollback().await.unwrap();
        assert_eq!(QueryCounter::get(), after_model);
        let before = after_model;
        let failed = db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT nope".to_owned(),
            ))
            .await;
        assert!(failed.is_err());
        assert_eq!(QueryCounter::get(), before);
        QueryCounter::record();
        assert_eq!(QueryCounter::get(), before + 1);
        let main = QueryCounter::get();
        let other = std::thread::spawn(|| {
            let _counter = QueryCounter::install();
            QueryCounter::record();
            QueryCounter::get()
        })
        .join()
        .unwrap();
        assert_eq!(other, 1);
        assert_eq!(QueryCounter::get(), main);
    }

    #[tokio::test]
    async fn sqlite_uuid_text_does_not_match_a_blob_parameter() {
        let db = connect("sqlite::memory:").await.unwrap();
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "CREATE TABLE ferrioxid_uuid_probe (id blob PRIMARY KEY NOT NULL)".to_owned(),
        ))
        .await
        .unwrap();
        let id = Uuid::from_u128(7);
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO ferrioxid_uuid_probe (id) VALUES (?)",
            [Value::Uuid(Some(Box::new(id)))],
        ))
        .await
        .unwrap();
        let typed = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT id FROM ferrioxid_uuid_probe WHERE id = ?",
                [Value::Uuid(Some(Box::new(id)))],
            ))
            .await
            .unwrap();
        assert!(typed.is_some());
        let text = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT id FROM ferrioxid_uuid_probe WHERE id = ?",
                [Value::String(Some(Box::new(id.to_string())))],
            ))
            .await
            .unwrap();
        assert!(text.is_none());
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct TemporalParent {
        id: Uuid,
        name: String,
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct TemporalEvent {
        id: Uuid,
        label: String,
        opened_on: String,
        starts_at: String,
        created_at: String,
        closed_on: Option<String>,
        parent_id: Uuid,
    }

    const TEMPORAL_PARENT: ModelDescriptor = ModelDescriptor {
        name: "TemporalParent",
        table: "ferrioxid_temporal_parents",
        api_path: "",
        allow_anonymous: false,
        permission: None,
        requires_object_policy: false,
    };

    const TEMPORAL_EVENT: ModelDescriptor = ModelDescriptor {
        name: "TemporalEvent",
        table: "ferrioxid_temporal_events",
        api_path: "",
        allow_anonymous: false,
        permission: None,
        requires_object_policy: false,
    };

    impl FerrioxidModel for TemporalParent {
        fn descriptor() -> &'static ModelDescriptor {
            &TEMPORAL_PARENT
        }

        fn columns() -> &'static [ColumnSpec] {
            use std::sync::OnceLock;
            static COLUMNS: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            COLUMNS.get_or_init(|| {
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("name", ColumnKind::Text, false, false),
                ]
            })
        }

        fn schema_sql(kind: DatabaseKind) -> String {
            match kind {
                DatabaseKind::Sqlite => {
                    "CREATE TABLE ferrioxid_temporal_parents (id text PRIMARY KEY NOT NULL, name text NOT NULL)"
                        .to_owned()
                }
                DatabaseKind::Postgres => {
                    "CREATE TABLE ferrioxid_temporal_parents (id uuid PRIMARY KEY NOT NULL, name text NOT NULL)"
                        .to_owned()
                }
            }
        }
    }

    impl FerrioxidModel for TemporalEvent {
        fn descriptor() -> &'static ModelDescriptor {
            &TEMPORAL_EVENT
        }

        fn columns() -> &'static [ColumnSpec] {
            use std::sync::OnceLock;
            static COLUMNS: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            COLUMNS.get_or_init(|| {
                let mut parent = column("parent_id", ColumnKind::Uuid, false, false);
                parent.foreign_table = Some("ferrioxid_temporal_parents");
                parent.foreign_column = Some("id");
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("label", ColumnKind::Text, false, false),
                    column("opened_on", ColumnKind::Date, false, false),
                    column("starts_at", ColumnKind::Time, false, false),
                    column("created_at", ColumnKind::DateTime, false, false),
                    column("closed_on", ColumnKind::Date, true, false),
                    parent,
                ]
            })
        }

        fn schema_sql(kind: DatabaseKind) -> String {
            match kind {
                DatabaseKind::Sqlite => "CREATE TABLE ferrioxid_temporal_events (
                    id text PRIMARY KEY NOT NULL,
                    label text NOT NULL,
                    opened_on text NOT NULL,
                    starts_at text NOT NULL,
                    created_at text NOT NULL,
                    closed_on text,
                    parent_id text NOT NULL
                )"
                .to_owned(),
                DatabaseKind::Postgres => "CREATE TABLE ferrioxid_temporal_events (
                    id uuid PRIMARY KEY NOT NULL,
                    label text NOT NULL,
                    opened_on date NOT NULL,
                    starts_at time NOT NULL,
                    created_at timestamptz NOT NULL,
                    closed_on date,
                    parent_id uuid NOT NULL
                )"
                .to_owned(),
            }
        }
    }

    fn date_value(text: &str) -> Value {
        Value::ChronoDate(Some(Box::new(ferrioxid_core::parse_date(text).unwrap())))
    }

    fn time_value(text: &str) -> Value {
        Value::ChronoTime(Some(Box::new(ferrioxid_core::parse_time(text).unwrap())))
    }

    fn instant_value(text: &str) -> Value {
        Value::ChronoDateTimeUtc(Some(Box::new(
            ferrioxid_core::parse_datetime_utc(text).unwrap(),
        )))
    }

    async fn insert_temporal(db: &impl ConnectionTrait, kind: DatabaseKind) {
        db.execute(Statement::from_string(
            db.get_database_backend(),
            TemporalParent::schema_sql(kind),
        ))
        .await
        .unwrap();
        db.execute(Statement::from_string(
            db.get_database_backend(),
            TemporalEvent::schema_sql(kind),
        ))
        .await
        .unwrap();
        for (id, name) in [(10_u128, "kept"), (11, "other")] {
            crate::insert_record(
                db,
                TemporalParent::descriptor().table,
                TemporalParent::columns(),
                vec![uuid_value(id), text_value(name)],
            )
            .await
            .unwrap();
        }
        let rows = [
            (
                1_u128,
                "first",
                "2026-11-10",
                "09:00:00",
                "2026-11-10T08:00:00Z",
                Some("2026-11-10"),
                11_u128,
            ),
            (
                2,
                "fraction",
                "2026-11-11",
                "14:30:00",
                "2026-11-11T13:30:00.123456Z",
                None,
                10,
            ),
            (
                3,
                "last",
                "2026-11-12",
                "18:00:00",
                "2026-11-12T13:30:00Z",
                None,
                10,
            ),
        ];
        for (id, label, opened, starts, created, closed, parent) in rows {
            crate::insert_record(
                db,
                TemporalEvent::descriptor().table,
                TemporalEvent::columns(),
                vec![
                    uuid_value(id),
                    text_value(label),
                    date_value(opened),
                    time_value(starts),
                    instant_value(created),
                    match closed {
                        Some(day) => date_value(day),
                        None => Value::ChronoDate(None),
                    },
                    uuid_value(parent),
                ],
            )
            .await
            .unwrap();
        }
    }

    fn temporal_labels(rows: &[TemporalEvent]) -> Vec<&str> {
        rows.iter().map(|row| row.label.as_str()).collect()
    }

    async fn exercise_temporal_filters(db: &impl ConnectionTrait) {
        let stored = TemporalEvent::query()
            .order_by("opened_on", Direction::Asc)
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&stored), vec!["first", "fraction", "last"]);
        assert_eq!(stored[1].created_at, "2026-11-11T13:30:00.123456Z");
        assert_eq!(stored[1].starts_at, "14:30:00");
        assert_eq!(stored[1].closed_on, None);

        let equal = TemporalEvent::query()
            .filter(col("opened_on").eq("2026-11-11"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&equal), vec!["fraction"]);
        let after = TemporalEvent::query()
            .filter(col("opened_on").gt("2026-11-11"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&after), vec!["last"]);
        let from = TemporalEvent::query()
            .filter(col("opened_on").gte("2026-11-11"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(from, 2);
        let before = TemporalEvent::query()
            .filter(col("opened_on").lt("2026-11-11"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&before), vec!["first"]);
        let until = TemporalEvent::query()
            .filter(col("opened_on").lte("2026-11-10"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(until, 1);
        let listed = TemporalEvent::query()
            .filter(col("opened_on").is_in(["2026-11-10", "2026-11-12"]))
            .count(db)
            .await
            .unwrap();
        assert_eq!(listed, 2);
        let open = TemporalEvent::query()
            .filter(col("closed_on").is_null())
            .count(db)
            .await
            .unwrap();
        assert_eq!(open, 2);
        let closed = TemporalEvent::query()
            .filter(col("closed_on").eq("2026-11-10"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&closed), vec!["first"]);
        let afternoon = TemporalEvent::query()
            .filter(col("starts_at").eq("14:30"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&afternoon), vec!["fraction"]);
        let later = TemporalEvent::query()
            .filter(col("starts_at").gt("14:30:00"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&later), vec!["last"]);
        let precise = TemporalEvent::query()
            .filter(col("created_at").eq("2026-11-11T13:30:00.123456Z"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&precise), vec!["fraction"]);
        let truncated = TemporalEvent::query()
            .filter(col("created_at").eq("2026-11-11T13:30:00Z"))
            .count(db)
            .await
            .unwrap();
        assert_eq!(truncated, 0);
        let after_instant = TemporalEvent::query()
            .filter(col("created_at").gt("2026-11-11T13:30:00.123456Z"))
            .all(db)
            .await
            .unwrap();
        assert_eq!(temporal_labels(&after_instant), vec!["last"]);
        assert!(TemporalEvent::query()
            .filter(col("opened_on").eq("2026-02-31"))
            .all(db)
            .await
            .unwrap_err()
            .to_string()
            .contains("validation.date"));
        assert!(TemporalEvent::query()
            .filter(col("starts_at").eq("24:00:00"))
            .all(db)
            .await
            .unwrap_err()
            .to_string()
            .contains("validation.time"));
        assert!(TemporalEvent::query()
            .filter(col("starts_at").eq("14:30:00.5"))
            .all(db)
            .await
            .is_err());
        assert!(TemporalEvent::query()
            .filter(col("created_at").eq("2026-11-11T13:30:00+02:00"))
            .all(db)
            .await
            .unwrap_err()
            .to_string()
            .contains("validation.datetime_offset"));

        let related = TemporalParent::query()
            .where_has::<TemporalEvent>(
                "parent_id",
                Related::new().filter(col("opened_on").eq("2026-11-11")),
            )
            .all(db)
            .await
            .unwrap();
        assert_eq!(
            related
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            vec!["kept"]
        );

        let mut cursor = None;
        let mut seen = Vec::new();
        for _ in 0..3 {
            let page = TemporalEvent::query()
                .order_by("opened_on", Direction::Asc)
                .limit(1)
                .keyset(db, cursor.as_deref())
                .await
                .unwrap();
            assert_eq!(page.data.len(), 1);
            assert!(page.has_more || seen.len() == 2);
            seen.push(page.data[0].label.clone());
            cursor = page.next;
        }
        assert_eq!(seen, vec!["first", "fraction", "last"]);
        let done = TemporalEvent::query()
            .order_by("opened_on", Direction::Asc)
            .limit(1)
            .keyset(db, cursor.as_deref())
            .await
            .unwrap();
        assert!(done.data.is_empty());
    }

    #[test]
    fn temporal_parameters_follow_the_column_kind() {
        let date = TemporalEvent::query().filter(col("opened_on").eq("2026-11-11"));
        let postgres = date.statement_for(DatabaseBackend::Postgres).unwrap();
        assert!(postgres.sql.contains("$1"));
        assert!(!postgres.sql.contains('?'));
        assert!(matches!(
            postgres.values.as_ref().unwrap().0[0],
            Value::ChronoDate(Some(_))
        ));
        let sqlite = date.statement_for(DatabaseBackend::Sqlite).unwrap();
        match &sqlite.values.as_ref().unwrap().0[0] {
            Value::String(Some(value)) => assert_eq!(value.as_str(), "2026-11-11"),
            other => panic!("expected sqlite date text, got {other:?}"),
        }
        let instant =
            TemporalEvent::query().filter(col("created_at").eq("2026-11-11T13:30:00.123456Z"));
        let parsed = ferrioxid_core::parse_datetime_utc("2026-11-11T13:30:00.123456Z").unwrap();
        let driver = parsed.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, false);
        assert_ne!(driver, "2026-11-11T13:30:00.123456Z");
        match &instant
            .statement_for(DatabaseBackend::Sqlite)
            .unwrap()
            .values
            .as_ref()
            .unwrap()
            .0[0]
        {
            Value::String(Some(value)) => assert_eq!(value.as_str(), driver),
            other => panic!("expected sqlite datetime text, got {other:?}"),
        }
        assert!(matches!(
            instant
                .statement_for(DatabaseBackend::Postgres)
                .unwrap()
                .values
                .as_ref()
                .unwrap()
                .0[0],
            Value::ChronoDateTimeUtc(Some(_))
        ));
    }

    #[tokio::test]
    async fn sqlite_temporal_filters_relations_and_keyset_pages() {
        let db = connect("sqlite::memory:").await.unwrap();
        insert_temporal(&db, DatabaseKind::Sqlite).await;
        exercise_temporal_filters(&db).await;
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL, CREATEDB, and an approved PostgreSQL test database"]
    async fn postgres_temporal_filters_and_keyset() {
        let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let disposable = crate::DisposablePostgres::create(&admin)
            .await
            .expect("isolated postgres");
        let db = connect(disposable.url()).await.expect("postgres");
        insert_temporal(&db, DatabaseKind::Postgres).await;
        exercise_temporal_filters(&db).await;
        db.close().await.expect("close");
        disposable
            .drop_database()
            .await
            .expect("drop isolated database");
    }

    fn uuid_value(id: u128) -> Value {
        Value::Uuid(Some(Box::new(Uuid::from_u128(id))))
    }

    fn text_value(text: &str) -> Value {
        Value::String(Some(Box::new(text.to_owned())))
    }
}
