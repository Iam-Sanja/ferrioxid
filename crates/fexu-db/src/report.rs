//! Grouped reports over a model. Column names and aliases come from a whitelist.
//! Values are bound. Application code does not supply SQL text.

use std::collections::BTreeMap;

use fexu_core::{ColumnKind, FexuError, FexuModel};
use sea_orm::sea_query::{Alias, Expr, ExprTrait, Func, Order, Query, SelectStatement};
use sea_orm::{ConnectionTrait, DatabaseBackend, QueryResult};
use uuid::Uuid;

use crate::query::{Direction, Select, PAGE_LIMIT};
use crate::sql::is_safe_ident;

const AVERAGE_SCALE: i64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureFn {
    Count,
    Sum,
    Average,
    Min,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measure {
    alias: &'static str,
    function: MeasureFn,
    column: Option<&'static str>,
}

impl Measure {
    pub fn count(alias: &'static str) -> Self {
        Self {
            alias,
            function: MeasureFn::Count,
            column: None,
        }
    }

    pub fn count_column(alias: &'static str, column: &'static str) -> Self {
        Self {
            alias,
            function: MeasureFn::Count,
            column: Some(column),
        }
    }

    pub fn sum(alias: &'static str, column: &'static str) -> Self {
        Self {
            alias,
            function: MeasureFn::Sum,
            column: Some(column),
        }
    }

    /// Exact average of a bigint column, truncated toward zero at six decimal places.
    ///
    /// The value is `sum * 1_000_000 / count`, computed from those two integers.
    /// SQLite `avg()` and PostgreSQL `avg()` are not used, because one returns
    /// a float and the other a numeric value.
    pub fn average(alias: &'static str, column: &'static str) -> Self {
        Self {
            alias,
            function: MeasureFn::Average,
            column: Some(column),
        }
    }

    pub fn min(alias: &'static str, column: &'static str) -> Self {
        Self {
            alias,
            function: MeasureFn::Min,
            column: Some(column),
        }
    }

    pub fn max(alias: &'static str, column: &'static str) -> Self {
        Self {
            alias,
            function: MeasureFn::Max,
            column: Some(column),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HavingCompare {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Having {
    alias: &'static str,
    compare: HavingCompare,
    value: i64,
}

impl Having {
    pub fn new(alias: &'static str, compare: HavingCompare, value: i64) -> Self {
        Self {
            alias,
            compare,
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupKey {
    Null,
    Integer(i64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasureValue {
    Null,
    Integer(i64),
    /// Six fractional digits, truncated toward zero. See [`Measure::average`].
    Average(String),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRow {
    pub keys: BTreeMap<&'static str, GroupKey>,
    pub measures: BTreeMap<&'static str, MeasureValue>,
}

enum OrderKey {
    Column(&'static str),
    Measure(&'static str),
}

struct PlannedOrder {
    key: OrderKey,
    direction: Direction,
}

pub struct Report<M> {
    source: Select<M>,
    groups: Vec<&'static str>,
    measures: Vec<Measure>,
    having: Vec<Having>,
    orders: Vec<PlannedOrder>,
    limit: u64,
}

impl<M> Report<M> {
    pub(crate) fn from_select(source: Select<M>) -> Self {
        Self {
            source,
            groups: Vec::new(),
            measures: Vec::new(),
            having: Vec::new(),
            orders: Vec::new(),
            limit: PAGE_LIMIT,
        }
    }

    pub fn group_by(mut self, columns: &[&'static str]) -> Self {
        self.groups.extend(columns.iter().copied());
        self
    }

    pub fn measure(mut self, measure: Measure) -> Self {
        self.measures.push(measure);
        self
    }

    pub fn having(mut self, having: Having) -> Self {
        self.having.push(having);
        self
    }

    pub fn order_by_key(mut self, column: &'static str, direction: Direction) -> Self {
        self.orders.push(PlannedOrder {
            key: OrderKey::Column(column),
            direction,
        });
        self
    }

    pub fn order_by_measure(mut self, alias: &'static str, direction: Direction) -> Self {
        self.orders.push(PlannedOrder {
            key: OrderKey::Measure(alias),
            direction,
        });
        self
    }

    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = limit;
        self
    }

    pub async fn all<C>(self, db: &C) -> Result<Vec<GroupRow>, FexuError>
    where
        M: FexuModel,
        C: ConnectionTrait,
    {
        if self.limit == 0 || self.limit > PAGE_LIMIT {
            return Err(FexuError::Validation(
                "report size must be between 1 and 100".into(),
            ));
        }
        if self.measures.is_empty() {
            return Err(FexuError::Validation(
                "a report needs at least one measure".into(),
            ));
        }
        let columns = M::columns();
        let mut seen_groups = Vec::new();
        for name in &self.groups {
            let column = lookup(columns, name)?;
            if !groupable(column.kind) {
                return Err(FexuError::Validation(format!(
                    "column {name} cannot be grouped"
                )));
            }
            if seen_groups.contains(name) {
                return Err(FexuError::Validation(format!(
                    "column {name} is grouped more than once"
                )));
            }
            seen_groups.push(*name);
        }
        let mut seen_alias = Vec::new();
        for measure in &self.measures {
            if !is_safe_ident(measure.alias) || seen_groups.contains(&measure.alias) {
                return Err(FexuError::Validation(
                    "measure alias is not available".into(),
                ));
            }
            if seen_alias.contains(&measure.alias) {
                return Err(FexuError::Validation(format!(
                    "measure {} is declared more than once",
                    measure.alias
                )));
            }
            seen_alias.push(measure.alias);
            validate_measure(columns, measure)?;
        }
        for having in &self.having {
            let measure = self
                .measures
                .iter()
                .find(|measure| measure.alias == having.alias)
                .ok_or_else(|| {
                    FexuError::Validation(format!("having alias {} is not a measure", having.alias))
                })?;
            if !having_integer(columns, measure)? {
                return Err(FexuError::Validation(format!(
                    "having does not accept {}",
                    having.alias
                )));
            }
        }
        for order in &self.orders {
            match order.key {
                OrderKey::Column(name) if !self.groups.contains(&name) => {
                    return Err(FexuError::Validation(format!(
                        "column {name} is not grouped"
                    )));
                }
                OrderKey::Measure(alias) => {
                    let measure = self
                        .measures
                        .iter()
                        .find(|measure| measure.alias == alias)
                        .ok_or_else(|| {
                            FexuError::Validation(format!("order alias {alias} is not a measure"))
                        })?;
                    if measure.function == MeasureFn::Average {
                        return Err(FexuError::Validation(
                            "average ordering is not portable; order by sum or count".into(),
                        ));
                    }
                }
                OrderKey::Column(_) => {}
            }
        }
        let backend = db.get_database_backend();
        let mut query = Query::select();
        query.from(Alias::new(M::descriptor().table));
        for name in &self.groups {
            query.expr_as(Expr::col(Alias::new(*name)), Alias::new(*name));
            query.group_by_col(Alias::new(*name));
        }
        for measure in &self.measures {
            push_measure(&mut query, columns, measure, backend)?;
        }
        if let Some(filter) = self.source.filter_expr(backend)? {
            query.and_where(filter);
        }
        for having in &self.having {
            let measure = self
                .measures
                .iter()
                .find(|measure| measure.alias == having.alias)
                .ok_or_else(|| {
                    FexuError::Validation(format!("having alias {} is not a measure", having.alias))
                })?;
            let expr = aggregate_expr(columns, measure, backend)?;
            let compared = match having.compare {
                HavingCompare::Eq => expr.eq(having.value),
                HavingCompare::Ne => expr.ne(having.value),
                HavingCompare::Gt => expr.gt(having.value),
                HavingCompare::Gte => expr.gte(having.value),
                HavingCompare::Lt => expr.lt(having.value),
                HavingCompare::Lte => expr.lte(having.value),
            };
            query.and_having(compared);
        }
        let mut ordered = Vec::new();
        for order in &self.orders {
            let name = match order.key {
                OrderKey::Column(name) => name,
                OrderKey::Measure(alias) => alias,
            };
            if !ordered.contains(&name) {
                query.order_by(Alias::new(name), sea_order(order.direction));
                ordered.push(name);
            }
        }
        for name in &self.groups {
            if !ordered.contains(name) {
                query.order_by(Alias::new(*name), Order::Asc);
            }
        }
        query.limit(self.limit);
        crate::query::note_statement();
        let statement = crate::sql::statement(db, &query)?;
        let rows = db.query_all(statement).await.map_err(FexuError::database)?;
        rows.into_iter()
            .map(|row| read_row(&row, columns, &self.groups, &self.measures))
            .collect()
    }
}

fn groupable(kind: ColumnKind) -> bool {
    matches!(
        kind,
        ColumnKind::Text
            | ColumnKind::Choice
            | ColumnKind::BigInt
            | ColumnKind::Uuid
            | ColumnKind::Date
            | ColumnKind::Time
            | ColumnKind::DateTime
    )
}

fn validate_measure(columns: &[fexu_core::ColumnSpec], measure: &Measure) -> Result<(), FexuError> {
    match measure.function {
        MeasureFn::Count => {
            if let Some(name) = measure.column {
                let _ = lookup(columns, name)?;
            }
            Ok(())
        }
        MeasureFn::Sum | MeasureFn::Average => {
            let name = measure
                .column
                .ok_or_else(|| FexuError::Validation("sum and average need a column".into()))?;
            let column = lookup(columns, name)?;
            if column.kind != ColumnKind::BigInt {
                return Err(FexuError::Validation(format!(
                    "column {name} cannot be aggregated as a number"
                )));
            }
            Ok(())
        }
        MeasureFn::Min | MeasureFn::Max => {
            let name = measure
                .column
                .ok_or_else(|| FexuError::Validation("min and max need a column".into()))?;
            let column = lookup(columns, name)?;
            if !matches!(
                column.kind,
                ColumnKind::BigInt
                    | ColumnKind::Text
                    | ColumnKind::Choice
                    | ColumnKind::Date
                    | ColumnKind::Time
                    | ColumnKind::DateTime
            ) {
                return Err(FexuError::Validation(format!(
                    "column {name} cannot be aggregated"
                )));
            }
            Ok(())
        }
    }
}

fn having_integer(columns: &[fexu_core::ColumnSpec], measure: &Measure) -> Result<bool, FexuError> {
    Ok(match measure.function {
        MeasureFn::Count | MeasureFn::Sum => true,
        MeasureFn::Average => false,
        MeasureFn::Min | MeasureFn::Max => numeric_column(columns, measure)?,
    })
}

fn numeric_column(columns: &[fexu_core::ColumnSpec], measure: &Measure) -> Result<bool, FexuError> {
    let Some(name) = measure.column else {
        return Ok(false);
    };
    Ok(lookup(columns, name)?.kind == ColumnKind::BigInt)
}

fn push_measure(
    query: &mut SelectStatement,
    columns: &[fexu_core::ColumnSpec],
    measure: &Measure,
    backend: DatabaseBackend,
) -> Result<(), FexuError> {
    match measure.function {
        MeasureFn::Average => {
            let sum_alias = average_part(measure.alias, "sum")?;
            let count_alias = average_part(measure.alias, "count")?;
            let column = measure
                .column
                .ok_or_else(|| FexuError::Validation("average needs a column".into()))?;
            query.expr_as(
                integer_aggregate(Expr::col(Alias::new(column)).sum(), backend),
                Alias::new(sum_alias),
            );
            query.expr_as(
                integer_aggregate(Expr::col(Alias::new(column)).count(), backend),
                Alias::new(count_alias),
            );
        }
        _ => {
            query.expr_as(
                aggregate_expr(columns, measure, backend)?,
                Alias::new(measure.alias),
            );
        }
    }
    Ok(())
}

fn aggregate_expr(
    columns: &[fexu_core::ColumnSpec],
    measure: &Measure,
    backend: DatabaseBackend,
) -> Result<sea_orm::sea_query::SimpleExpr, FexuError> {
    let column = measure.column.unwrap_or("id");
    let source = Expr::col(Alias::new(column));
    Ok(match measure.function {
        MeasureFn::Count => integer_aggregate(source.count(), backend),
        MeasureFn::Sum => integer_aggregate(source.sum(), backend),
        MeasureFn::Min | MeasureFn::Max => {
            let expr = if measure.function == MeasureFn::Min {
                source.min()
            } else {
                source.max()
            };
            if numeric_column(columns, measure)? {
                integer_aggregate(expr, backend)
            } else {
                expr
            }
        }
        MeasureFn::Average => {
            return Err(FexuError::Validation(
                "average is not a single SQL value".into(),
            ))
        }
    })
}

fn integer_aggregate(
    expr: sea_orm::sea_query::SimpleExpr,
    backend: DatabaseBackend,
) -> sea_orm::sea_query::SimpleExpr {
    let cast = match backend {
        DatabaseBackend::Postgres => "bigint",
        DatabaseBackend::Sqlite | DatabaseBackend::MySql => "integer",
    };
    Func::cast_as(expr, Alias::new(cast)).into()
}

fn average_part(alias: &str, suffix: &str) -> Result<String, FexuError> {
    let name = format!("fexu_{alias}_{suffix}");
    if !is_safe_ident(&name) {
        return Err(FexuError::Validation(
            "measure alias is not available".into(),
        ));
    }
    Ok(name)
}

fn sea_order(direction: Direction) -> Order {
    match direction {
        Direction::Asc => Order::Asc,
        Direction::Desc => Order::Desc,
    }
}

fn lookup<'a>(
    columns: &'a [fexu_core::ColumnSpec],
    name: &str,
) -> Result<&'a fexu_core::ColumnSpec, FexuError> {
    if !is_safe_ident(name) {
        return Err(FexuError::Validation(format!(
            "column {name} is not available"
        )));
    }
    columns
        .iter()
        .find(|column| column.name == name)
        .ok_or_else(|| FexuError::Validation(format!("column {name} is not available")))
}

fn read_row(
    row: &QueryResult,
    columns: &[fexu_core::ColumnSpec],
    groups: &[&'static str],
    measures: &[Measure],
) -> Result<GroupRow, FexuError> {
    let mut keys = BTreeMap::new();
    for name in groups {
        let column = lookup(columns, name)?;
        keys.insert(*name, read_key(row, column)?);
    }
    let mut values = BTreeMap::new();
    for measure in measures {
        values.insert(measure.alias, read_measure(row, columns, measure)?);
    }
    Ok(GroupRow {
        keys,
        measures: values,
    })
}

fn read_key(row: &QueryResult, column: &fexu_core::ColumnSpec) -> Result<GroupKey, FexuError> {
    if column.kind == ColumnKind::BigInt {
        return Ok(match read_i64(row, column.name)? {
            Some(value) => GroupKey::Integer(value),
            None => GroupKey::Null,
        });
    }
    if column.kind == ColumnKind::Uuid {
        if let Ok(value) = row.try_get::<Option<Uuid>>("", column.name) {
            return Ok(match value {
                Some(id) => GroupKey::Text(id.to_string()),
                None => GroupKey::Null,
            });
        }
    }
    Ok(match read_text(row, column.name)? {
        Some(value) => GroupKey::Text(value),
        None => GroupKey::Null,
    })
}

fn read_measure(
    row: &QueryResult,
    columns: &[fexu_core::ColumnSpec],
    measure: &Measure,
) -> Result<MeasureValue, FexuError> {
    match measure.function {
        MeasureFn::Count | MeasureFn::Sum => Ok(match read_i64(row, measure.alias)? {
            Some(value) => MeasureValue::Integer(value),
            None => MeasureValue::Null,
        }),
        MeasureFn::Average => {
            let sum_alias = average_part(measure.alias, "sum")?;
            let count_alias = average_part(measure.alias, "count")?;
            let sum = read_i64(row, &sum_alias)?.unwrap_or(0);
            let count = read_i64(row, &count_alias)?.unwrap_or(0);
            if count == 0 {
                return Ok(MeasureValue::Null);
            }
            Ok(MeasureValue::Average(format_average(sum, count)?))
        }
        MeasureFn::Min | MeasureFn::Max => {
            let name = measure.column.unwrap_or("id");
            let column = lookup(columns, name)?;
            if column.kind == ColumnKind::BigInt {
                return Ok(match read_i64(row, measure.alias)? {
                    Some(value) => MeasureValue::Integer(value),
                    None => MeasureValue::Null,
                });
            }
            Ok(match read_text(row, measure.alias)? {
                Some(value) => MeasureValue::Text(value),
                None => MeasureValue::Null,
            })
        }
    }
}

fn format_average(sum: i64, count: i64) -> Result<String, FexuError> {
    let scaled = sum
        .checked_mul(AVERAGE_SCALE)
        .ok_or_else(|| FexuError::Validation("average is outside the portable range".into()))?;
    let whole = scaled / count;
    let negative = whole < 0;
    let absolute = whole.unsigned_abs();
    let integer = absolute / AVERAGE_SCALE.unsigned_abs();
    let fraction = absolute % AVERAGE_SCALE.unsigned_abs();
    Ok(format!(
        "{}{integer}.{fraction:06}",
        if negative { "-" } else { "" }
    ))
}

fn read_i64(row: &QueryResult, column: &str) -> Result<Option<i64>, FexuError> {
    if let Ok(value) = row.try_get::<Option<i64>>("", column) {
        return Ok(value);
    }
    if let Ok(value) = row.try_get::<Option<i32>>("", column) {
        return Ok(value.map(i64::from));
    }
    row.try_get::<Option<i64>>("", column)
        .map_err(FexuError::database)
}

fn read_text(row: &QueryResult, column: &str) -> Result<Option<String>, FexuError> {
    row.try_get::<Option<String>>("", column)
        .map_err(FexuError::database)
}

pub use MeasureValue as AverageValue;
