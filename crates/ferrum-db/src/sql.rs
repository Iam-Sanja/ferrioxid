//! SeaQuery-backed CRUD and DDL for SQLite and PostgreSQL.

use ferrum_core::{ColumnKind, ColumnSpec, DatabaseKind, FerrumError};
use sea_orm::sea_query::{
    Alias, ColumnDef, Expr, Order, PostgresQueryBuilder, Query, SqliteQueryBuilder, Table,
};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, QueryResult, Statement, Value,
};
use serde_json::{Map, Number, Value as Json};
use uuid::Uuid;

pub const LIST_LIMIT: usize = 100;

pub fn kind_of(backend: DatabaseBackend) -> Result<DatabaseKind, FerrumError> {
    match backend {
        DatabaseBackend::Sqlite => Ok(DatabaseKind::Sqlite),
        DatabaseBackend::Postgres => Ok(DatabaseKind::Postgres),
        DatabaseBackend::MySql => Err(FerrumError::database(UnsupportedBackend)),
    }
}

/// Spalte für DDL. CRUD und Migrationen nutzen dieselbe Typabbildung.
#[derive(Debug, Clone, Copy)]
pub struct PhysicalColumn<'a> {
    pub name: &'a str,
    pub kind: ColumnKind,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_bool: Option<bool>,
}

impl<'a> From<&'a ColumnSpec> for PhysicalColumn<'a> {
    fn from(column: &'a ColumnSpec) -> Self {
        Self {
            name: column.name,
            kind: column.kind,
            nullable: column.nullable,
            primary_key: column.primary_key,
            default_bool: column.default_bool,
        }
    }
}

pub fn render_schema(kind: DatabaseKind, table: &str, columns: &[ColumnSpec]) -> String {
    let physical: Vec<PhysicalColumn<'_>> = columns.iter().map(PhysicalColumn::from).collect();
    render_create_table(kind, table, &physical, true)
}

pub fn render_create_table(
    kind: DatabaseKind,
    table: &str,
    columns: &[PhysicalColumn<'_>],
    if_not_exists: bool,
) -> String {
    let mut statement = Table::create();
    statement.table(Alias::new(table));
    if if_not_exists {
        statement.if_not_exists();
    }
    for column in columns {
        let mut definition = column_definition(kind, column);
        statement.col(&mut definition);
    }
    render_ddl(kind, statement)
}

pub fn render_add_column(kind: DatabaseKind, table: &str, column: &PhysicalColumn<'_>) -> String {
    let mut statement = Table::alter();
    let mut definition = column_definition(kind, column);
    statement
        .table(Alias::new(table))
        .add_column(&mut definition);
    render_ddl(kind, statement)
}

pub fn render_drop_table(kind: DatabaseKind, table: &str) -> String {
    let mut statement = Table::drop();
    statement.table(Alias::new(table));
    render_ddl(kind, statement)
}

pub fn render_drop_column(kind: DatabaseKind, table: &str, column: &str) -> String {
    let mut statement = Table::alter();
    statement
        .table(Alias::new(table))
        .drop_column(Alias::new(column));
    render_ddl(kind, statement)
}

fn column_definition(kind: DatabaseKind, column: &PhysicalColumn<'_>) -> ColumnDef {
    let mut definition = ColumnDef::new(Alias::new(column.name));
    match (kind, column.kind) {
        (DatabaseKind::Postgres, ColumnKind::Uuid) => {
            definition.uuid();
        }
        (DatabaseKind::Sqlite, ColumnKind::Uuid) => {
            definition.text();
        }
        (_, ColumnKind::Text) => {
            definition.text();
        }
        (_, ColumnKind::BigInt) => {
            definition.big_integer();
        }
        (DatabaseKind::Postgres, ColumnKind::Boolean) => {
            definition.boolean();
        }
        (DatabaseKind::Sqlite, ColumnKind::Boolean) => {
            definition.integer();
        }
    }
    if !column.nullable {
        definition.not_null();
    }
    if column.primary_key {
        definition.primary_key();
    }
    if let Some(default) = column.default_bool {
        match kind {
            DatabaseKind::Postgres => {
                definition.default(default);
            }
            DatabaseKind::Sqlite => {
                definition.default(i32::from(default));
            }
        }
    }
    definition
}

fn render_ddl(
    kind: DatabaseKind,
    statement: impl sea_orm::sea_query::SchemaStatementBuilder,
) -> String {
    match kind {
        DatabaseKind::Postgres => statement.to_string(PostgresQueryBuilder),
        DatabaseKind::Sqlite => statement.to_string(SqliteQueryBuilder),
    }
}

pub struct ValuePage {
    pub rows: Vec<Json>,
    pub limit: usize,
    pub truncated: bool,
}

pub async fn list_records(
    db: &DatabaseConnection,
    table: &str,
    columns: &[ColumnSpec],
) -> Result<ValuePage, FerrumError> {
    let mut query = Query::select();
    query
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .from(Alias::new(table))
        .order_by(Alias::new("id"), Order::Asc)
        .limit((LIST_LIMIT + 1) as u64);
    let rows = db
        .query_all(statement(db, &query)?)
        .await
        .map_err(FerrumError::database)?;
    let truncated = rows.len() > LIST_LIMIT;
    let rows = rows
        .into_iter()
        .take(LIST_LIMIT)
        .map(|row| row_to_json(&row, columns))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ValuePage {
        rows,
        limit: LIST_LIMIT,
        truncated,
    })
}

pub async fn get_record(
    db: &DatabaseConnection,
    table: &str,
    columns: &[ColumnSpec],
    id: Uuid,
) -> Result<Option<Json>, FerrumError> {
    let mut query = Query::select();
    query
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .from(Alias::new(table))
        .and_where(Expr::col(Alias::new("id")).eq(id));
    let row = db
        .query_one(statement(db, &query)?)
        .await
        .map_err(FerrumError::database)?;
    row.map(|row| row_to_json(&row, columns)).transpose()
}

pub async fn insert_record(
    db: &DatabaseConnection,
    table: &str,
    columns: &[ColumnSpec],
    values: Vec<Value>,
) -> Result<Json, FerrumError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(table))
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .values_panic(values.into_iter().map(Into::into))
        .returning_all();
    let row = db
        .query_one(statement(db, &query)?)
        .await
        .map_err(FerrumError::database)?
        .ok_or_else(|| FerrumError::Validation("no row returned".into()))?;
    row_to_json(&row, columns)
}

pub async fn update_record(
    db: &DatabaseConnection,
    table: &str,
    columns: &[ColumnSpec],
    id: Uuid,
    changes: Vec<(&'static str, Value)>,
) -> Result<Json, FerrumError> {
    if changes.is_empty() {
        return get_record(db, table, columns, id)
            .await?
            .ok_or(FerrumError::NotFound);
    }
    let mut query = Query::update();
    query
        .table(Alias::new(table))
        .and_where(Expr::col(Alias::new("id")).eq(id))
        .returning_all();
    for (name, value) in changes {
        query.value(Alias::new(name), value);
    }
    let row = db
        .query_one(statement(db, &query)?)
        .await
        .map_err(FerrumError::database)?
        .ok_or(FerrumError::NotFound)?;
    row_to_json(&row, columns)
}

pub async fn delete_record(
    db: &DatabaseConnection,
    table: &str,
    id: Uuid,
) -> Result<u64, FerrumError> {
    let mut query = Query::delete();
    query
        .from_table(Alias::new(table))
        .and_where(Expr::col(Alias::new("id")).eq(id));
    let result = db
        .execute(statement(db, &query)?)
        .await
        .map_err(FerrumError::database)?;
    Ok(result.rows_affected())
}

fn statement(
    db: &DatabaseConnection,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Statement, FerrumError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => return Err(FerrumError::database(UnsupportedBackend)),
    };
    Ok(Statement::from_sql_and_values(backend, sql, values.0))
}

fn row_to_json(row: &QueryResult, columns: &[ColumnSpec]) -> Result<Json, FerrumError> {
    let mut object = Map::new();
    for column in columns {
        let value = match column.kind {
            ColumnKind::Text => json_text(row, column)?,
            ColumnKind::BigInt => json_int(row, column)?,
            ColumnKind::Boolean => json_bool(row, column)?,
            ColumnKind::Uuid => json_uuid(row, column)?,
        };
        object.insert(column.name.to_owned(), value);
    }
    Ok(Json::Object(object))
}

fn json_text(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrumError> {
    let value = row
        .try_get::<Option<String>>("", column.name)
        .map_err(FerrumError::database)?;
    Ok(match value {
        Some(text) => Json::String(text),
        None => Json::Null,
    })
}

fn json_int(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrumError> {
    if let Ok(value) = row.try_get::<Option<i64>>("", column.name) {
        return Ok(match value {
            Some(number) => Json::Number(Number::from(number)),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<i32>>("", column.name)
        .map_err(FerrumError::database)?;
    Ok(match value {
        Some(number) => Json::Number(Number::from(number)),
        None => Json::Null,
    })
}

fn json_bool(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrumError> {
    if let Ok(value) = row.try_get::<Option<bool>>("", column.name) {
        return Ok(match value {
            Some(flag) => Json::Bool(flag),
            None => Json::Null,
        });
    }
    if let Ok(value) = row.try_get::<Option<i64>>("", column.name) {
        return Ok(match value {
            Some(flag) => Json::Bool(flag != 0),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<i32>>("", column.name)
        .map_err(FerrumError::database)?;
    Ok(match value {
        Some(flag) => Json::Bool(flag != 0),
        None => Json::Null,
    })
}

fn json_uuid(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrumError> {
    if let Ok(value) = row.try_get::<Option<Uuid>>("", column.name) {
        return Ok(match value {
            Some(id) => Json::String(id.to_string()),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<String>>("", column.name)
        .map_err(FerrumError::database)?;
    Ok(match value {
        Some(text) => Json::String(text),
        None => Json::Null,
    })
}

#[derive(Debug)]
struct UnsupportedBackend;

impl std::fmt::Display for UnsupportedBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("only sqlite and postgres are supported")
    }
}

impl std::error::Error for UnsupportedBackend {}
