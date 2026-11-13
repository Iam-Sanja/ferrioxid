//! Cursor pages for generated list routes.
//!
//! The page uses [`fexu_db::Select::keyset`]. Offset is not accepted.
//! Filters and sorts stay on the admin allow-list and on fields the read
//! serializer still returns. [`QueryScope`] is applied by the caller.

use fexu_core::{ColumnKind, ColumnSpec, FexuError, FexuModel, QueryScope};
use fexu_db::driver::DatabaseConnection;
use fexu_db::{col, Bound, Direction, ModelQuery, ModelRepository, PAGE_LIMIT};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct RawList {
    pub limit: Option<u64>,
    pub cursor: Option<String>,
    pub sort: Option<String>,
    pub filter: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ApiPage {
    pub data: Vec<Value>,
    pub limit: u64,
    pub next: Option<String>,
    pub has_more: bool,
}

pub fn read_json<M>(model: &M) -> Result<Value, FexuError>
where
    M: FexuModel + Serialize,
{
    let mut value = serde_json::to_value(model).map_err(FexuError::database)?;
    if let Some(object) = value.as_object_mut() {
        for contract in M::field_contracts() {
            if !contract.api_read {
                object.remove(contract.column);
            }
        }
    }
    Ok(value)
}

pub async fn page<M>(
    db: &DatabaseConnection,
    scope: QueryScope,
    query: &RawList,
) -> Result<ApiPage, FexuError>
where
    M: ModelRepository,
{
    let limit = query.limit.unwrap_or(PAGE_LIMIT);
    let mut select = M::query().scope(scope).limit(limit);
    if let Some(sort) = query
        .sort
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if sort.len() > 64 {
            return Err(FexuError::Validation("sort field is not allowed".into()));
        }
        let (name, direction) = parse_sort(sort)?;
        let column = allow_sort::<M>(name)?;
        select = select.order_by(column.name, direction);
    }
    if query.filter.len() > 8 {
        return Err(FexuError::Validation("filter field is not allowed".into()));
    }
    for filter in &query.filter {
        let predicate = parse_filter::<M>(filter)?;
        select = select.filter(predicate);
    }
    let cursor = query
        .cursor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(token) = cursor {
        if token.len() > 4096 || !token.starts_with("k1.") {
            return Err(FexuError::Validation("keyset cursor is not valid".into()));
        }
    }
    let result = select.keyset(db, cursor).await?;
    let mut data = Vec::with_capacity(result.data.len());
    for row in &result.data {
        data.push(read_json(row)?);
    }
    Ok(ApiPage {
        data,
        limit: result.limit,
        next: if result.has_more { result.next } else { None },
        has_more: result.has_more,
    })
}

fn parse_sort(sort: &str) -> Result<(&str, Direction), FexuError> {
    if let Some(name) = sort.strip_prefix('-') {
        if name.is_empty() || name.contains('-') {
            return Err(FexuError::Validation("sort field is not allowed".into()));
        }
        Ok((name, Direction::Desc))
    } else if sort.contains('-') {
        Err(FexuError::Validation("sort field is not allowed".into()))
    } else {
        Ok((sort, Direction::Asc))
    }
}

fn allow_sort<M: FexuModel>(name: &str) -> Result<&'static ColumnSpec, FexuError> {
    let column = M::columns()
        .iter()
        .find(|column| column.name == name)
        .ok_or_else(|| FexuError::Validation("sort field is not allowed".into()))?;
    if !readable::<M>(column.name) {
        return Err(FexuError::Validation("sort field is not allowed".into()));
    }
    let allowed = column.name == "id"
        || M::admin_meta().is_some_and(|meta| {
            meta.fields
                .iter()
                .any(|field| field.name == column.name && field.sortable && !field.hidden)
        });
    if allowed {
        Ok(column)
    } else {
        Err(FexuError::Validation("sort field is not allowed".into()))
    }
}

fn parse_filter<M: FexuModel>(raw: &str) -> Result<fexu_db::Predicate, FexuError> {
    if raw.len() > 300 {
        return Err(FexuError::Validation("filter field is not allowed".into()));
    }
    let (name, op, value) = split_filter(raw)
        .ok_or_else(|| FexuError::Validation("filter field is not allowed".into()))?;
    let column = M::columns()
        .iter()
        .find(|column| column.name == name)
        .ok_or_else(|| FexuError::Validation("filter field is not allowed".into()))?;
    if !filterable::<M>(column) {
        return Err(FexuError::Validation("filter field is not allowed".into()));
    }
    let column_name = column.name;
    match op {
        "isnull" if value.is_none() && column.nullable => Ok(col(column_name).is_null()),
        "notnull" if value.is_none() && column.nullable => Ok(col(column_name).is_not_null()),
        "eq" | "ne" | "gt" | "gte" | "lt" | "lte" | "contains" => {
            let text =
                value.ok_or_else(|| FexuError::Validation("filter field is not allowed".into()))?;
            if text.chars().count() > 200 {
                return Err(FexuError::Validation("filter field is not allowed".into()));
            }
            let bound = bound_for(column, text)?;
            match op {
                "eq" => Ok(col(column_name).eq(bound)),
                "ne" => Ok(col(column_name).ne(bound)),
                "gt" => Ok(col(column_name).gt(bound)),
                "gte" => Ok(col(column_name).gte(bound)),
                "lt" => Ok(col(column_name).lt(bound)),
                "lte" => Ok(col(column_name).lte(bound)),
                "contains" if column.kind == ColumnKind::Text => {
                    Ok(col(column_name).contains(text))
                }
                _ => Err(FexuError::Validation("filter field is not allowed".into())),
            }
        }
        _ => Err(FexuError::Validation("filter field is not allowed".into())),
    }
}

fn filterable<M: FexuModel>(column: &ColumnSpec) -> bool {
    readable::<M>(column.name)
        && M::admin_meta().is_some_and(|meta| {
            meta.fields
                .iter()
                .any(|field| field.name == column.name && field.filterable && !field.hidden)
        })
}

fn readable<M: FexuModel>(name: &str) -> bool {
    match M::field_contracts()
        .iter()
        .find(|contract| contract.column == name)
    {
        Some(contract) => contract.api_read,
        None => true,
    }
}

fn bound_for(column: &ColumnSpec, text: &str) -> Result<Bound, FexuError> {
    match column.kind {
        ColumnKind::Text | ColumnKind::Date | ColumnKind::Time | ColumnKind::DateTime => {
            Ok(Bound::Text(text.to_owned()))
        }
        ColumnKind::Decimal => Ok(Bound::Text(text.to_owned())),
        ColumnKind::Choice => {
            if column.choices.iter().any(|choice| choice.value == text) {
                Ok(Bound::Text(text.to_owned()))
            } else {
                Err(FexuError::Validation("filter field is not allowed".into()))
            }
        }
        ColumnKind::BigInt => text
            .parse::<i64>()
            .map(Bound::I64)
            .map_err(|_| FexuError::Validation("filter field is not allowed".into())),
        ColumnKind::Boolean => match text {
            "true" => Ok(Bound::Bool(true)),
            "false" => Ok(Bound::Bool(false)),
            _ => Err(FexuError::Validation("filter field is not allowed".into())),
        },
        ColumnKind::Uuid => uuid::Uuid::parse_str(text)
            .map(Bound::Uuid)
            .map_err(|_| FexuError::Validation("filter field is not allowed".into())),
        ColumnKind::Json => Err(FexuError::Validation("filter field is not allowed".into())),
    }
}

pub(crate) fn split_filter(raw: &str) -> Option<(&str, &str, Option<&str>)> {
    let (name, rest) = raw.split_once(':')?;
    if name.is_empty()
        || !name
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_')
    {
        return None;
    }
    match rest.split_once(':') {
        Some((op, value)) if !op.is_empty() => Some((name, op, Some(value))),
        None if !rest.is_empty() => Some((name, rest, None)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::split_filter;

    #[test]
    fn filter_values_may_contain_colons() {
        assert_eq!(
            split_filter("name:eq:a:b"),
            Some(("name", "eq", Some("a:b")))
        );
        assert_eq!(split_filter("note:isnull"), Some(("note", "isnull", None)));
        assert_eq!(split_filter(":eq:a"), None);
    }
}
