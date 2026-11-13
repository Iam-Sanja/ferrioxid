//! PostgreSQL execution through the published `sqlx-postgres` and `sqlx-core` crates.
//!
//! The SQLx facade crate is not a dependency. Every call that talks to those crates
//! stays in this module so a later driver update has one place to change.

use crate::error::{ConstraintKind, DbErr, RuntimeErr};
use crate::rows::{
    self, cell_bool, cell_bytes, cell_date, cell_datetime, cell_int, cell_json, cell_null,
    cell_numeric, cell_real, cell_text, cell_time, cell_timestamptz, cell_uuid, QueryResult,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use sea_query::Value;
use serde_json::Value as Json;
use sqlx_core::arguments::Arguments;
use sqlx_core::column::Column;
use sqlx_core::error::ErrorKind;
use sqlx_core::executor::Executor;
use sqlx_core::query::{query_with, Query};
use sqlx_core::row::Row;
use sqlx_core::type_info::TypeInfo;
use sqlx_postgres::{
    PgArguments, PgConnectOptions, PgPool, PgPoolOptions, PgQueryResult, PgRow, PgSslMode, Postgres,
};
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone)]
pub(crate) struct PgSession {
    pool: PgPool,
}

pub(crate) struct PgTransaction {
    inner: tokio::sync::Mutex<Option<sqlx_postgres::PgTransaction<'static>>>,
}

impl PgSession {
    pub(crate) async fn connect(
        url: &str,
        max_connections: u32,
        min_connections: u32,
    ) -> Result<Self, DbErr> {
        let options = PgConnectOptions::from_str(url)
            .map_err(|error| DbErr::Conn(RuntimeErr::internal(error.to_string())))?
            .ssl_mode(PgSslMode::Prefer);
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .acquire_timeout(ACQUIRE_TIMEOUT)
            .idle_timeout(IDLE_TIMEOUT)
            .connect_with(options)
            .await
            .map_err(map_connect)?;
        Ok(Self { pool })
    }

    pub(crate) async fn close(self) -> Result<(), DbErr> {
        self.pool.close().await;
        Ok(())
    }

    pub(crate) async fn begin(&self) -> Result<PgTransaction, DbErr> {
        let transaction = self.pool.begin().await.map_err(map_connect)?;
        Ok(PgTransaction {
            inner: tokio::sync::Mutex::new(Some(transaction)),
        })
    }

    pub(crate) async fn execute(&self, sql: &str, values: &[Value]) -> Result<u64, DbErr> {
        let query = prepared(sql, values)?;
        let result = query
            .execute(&self.pool)
            .await
            .map_err(|error| map_sqlx(error, true))?;
        Ok(result.rows_affected())
    }

    pub(crate) async fn execute_unprepared(&self, sql: &str) -> Result<u64, DbErr> {
        let query = query_with::<Postgres, PgArguments>(sql, PgArguments::default());
        let result = run_query(query, &self.pool)
            .await
            .map_err(|error| map_sqlx(error, true))?;
        Ok(result.rows_affected())
    }

    pub(crate) async fn query_all(
        &self,
        sql: &str,
        values: &[Value],
    ) -> Result<Vec<QueryResult>, DbErr> {
        let query = prepared(sql, values)?;
        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|error| map_sqlx(error, false))?;
        rows.iter().map(decode_row).collect()
    }
}

impl PgTransaction {
    pub(crate) async fn commit(self) -> Result<(), DbErr> {
        let transaction = self
            .inner
            .lock()
            .await
            .take()
            .ok_or_else(|| DbErr::exec("PostgreSQL transaction is closed"))?;
        transaction
            .commit()
            .await
            .map_err(|error| map_sqlx(error, true))
    }

    pub(crate) async fn rollback(self) -> Result<(), DbErr> {
        let transaction = self
            .inner
            .lock()
            .await
            .take()
            .ok_or_else(|| DbErr::exec("PostgreSQL transaction is closed"))?;
        transaction
            .rollback()
            .await
            .map_err(|error| map_sqlx(error, true))
    }

    pub(crate) async fn execute(&self, sql: &str, values: &[Value]) -> Result<u64, DbErr> {
        let query = prepared(sql, values)?;
        let mut guard = self.inner.lock().await;
        let mut transaction = guard
            .take()
            .ok_or_else(|| DbErr::exec("PostgreSQL transaction is closed"))?;
        let result = run_query(query, &mut *transaction)
            .await
            .map_err(|error| map_sqlx(error, true));
        *guard = Some(transaction);
        Ok(result?.rows_affected())
    }

    pub(crate) async fn execute_unprepared(&self, sql: &str) -> Result<u64, DbErr> {
        let mut guard = self.inner.lock().await;
        let mut transaction = guard
            .take()
            .ok_or_else(|| DbErr::exec("PostgreSQL transaction is closed"))?;
        let query = query_with::<Postgres, PgArguments>(sql, PgArguments::default());
        let result = run_query(query, &mut *transaction)
            .await
            .map_err(|error| map_sqlx(error, true));
        *guard = Some(transaction);
        Ok(rows_affected(&result?))
    }

    pub(crate) async fn query_all(
        &self,
        sql: &str,
        values: &[Value],
    ) -> Result<Vec<QueryResult>, DbErr> {
        let query = prepared(sql, values)?;
        let mut guard = self.inner.lock().await;
        let mut transaction = guard
            .take()
            .ok_or_else(|| DbErr::exec("PostgreSQL transaction is closed"))?;
        let rows = fetch_query(query, &mut *transaction)
            .await
            .map_err(|error| map_sqlx(error, false));
        *guard = Some(transaction);
        rows?.iter().map(decode_row).collect()
    }
}

async fn run_query<'e, E>(
    query: Query<'e, Postgres, PgArguments>,
    executor: E,
) -> Result<PgQueryResult, sqlx_core::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    query.execute(executor).await
}

async fn fetch_query<'e, E>(
    query: Query<'e, Postgres, PgArguments>,
    executor: E,
) -> Result<Vec<PgRow>, sqlx_core::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    query.fetch_all(executor).await
}

fn prepared<'a>(
    sql: &'a str,
    values: &[Value],
) -> Result<sqlx_core::query::Query<'a, Postgres, PgArguments>, DbErr> {
    let mut arguments = PgArguments::default();
    for value in values {
        bind(&mut arguments, value)?;
    }
    Ok(query_with(sql, arguments))
}

fn bind(arguments: &mut PgArguments, value: &Value) -> Result<(), DbErr> {
    let bound = match value {
        Value::Bool(None) => arguments.add(None::<bool>),
        Value::Bool(Some(value)) => arguments.add(*value),
        Value::TinyInt(None) => arguments.add(None::<i8>),
        Value::TinyInt(Some(value)) => arguments.add(*value),
        Value::SmallInt(None) => arguments.add(None::<i16>),
        Value::SmallInt(Some(value)) => arguments.add(*value),
        Value::Int(None) => arguments.add(None::<i32>),
        Value::Int(Some(value)) => arguments.add(*value),
        Value::BigInt(None) => arguments.add(None::<i64>),
        Value::BigInt(Some(value)) => arguments.add(*value),
        Value::TinyUnsigned(None) => arguments.add(None::<i16>),
        Value::TinyUnsigned(Some(value)) => arguments.add(i16::from(*value)),
        Value::SmallUnsigned(None) => arguments.add(None::<i32>),
        Value::SmallUnsigned(Some(value)) => arguments.add(i32::from(*value)),
        Value::Unsigned(None) => arguments.add(None::<i64>),
        Value::Unsigned(Some(value)) => arguments.add(i64::from(*value)),
        Value::BigUnsigned(None) => arguments.add(None::<i64>),
        Value::BigUnsigned(Some(value)) => {
            let signed = i64::try_from(*value).map_err(|_| {
                DbErr::exec("PostgreSQL cannot bind an unsigned integer above i64::MAX")
            })?;
            arguments.add(signed)
        }
        Value::Float(None) => arguments.add(None::<f32>),
        Value::Float(Some(value)) => arguments.add(*value),
        Value::Double(None) => arguments.add(None::<f64>),
        Value::Double(Some(value)) => arguments.add(*value),
        Value::String(None) | Value::Char(None) => arguments.add(None::<String>),
        Value::String(Some(value)) => arguments.add(value.to_string()),
        Value::Char(Some(value)) => arguments.add(value.to_string()),
        Value::Bytes(None) => arguments.add(None::<Vec<u8>>),
        Value::Bytes(Some(value)) => arguments.add(value.as_ref().clone()),
        Value::Json(None) => arguments.add(None::<Json>),
        Value::Json(Some(value)) => arguments.add(value.as_ref().clone()),
        Value::ChronoDate(None) => arguments.add(None::<NaiveDate>),
        Value::ChronoDate(Some(value)) => arguments.add(**value),
        Value::ChronoTime(None) => arguments.add(None::<NaiveTime>),
        Value::ChronoTime(Some(value)) => arguments.add(**value),
        Value::ChronoDateTime(None) => arguments.add(None::<NaiveDateTime>),
        Value::ChronoDateTime(Some(value)) => arguments.add(**value),
        Value::ChronoDateTimeUtc(None)
        | Value::ChronoDateTimeLocal(None)
        | Value::ChronoDateTimeWithTimeZone(None) => arguments.add(None::<DateTime<Utc>>),
        Value::ChronoDateTimeUtc(Some(value)) => arguments.add(**value),
        Value::ChronoDateTimeLocal(Some(value)) => arguments.add(value.with_timezone(&Utc)),
        Value::ChronoDateTimeWithTimeZone(Some(value)) => arguments.add(value.with_timezone(&Utc)),
        Value::Uuid(None) => arguments.add(None::<Uuid>),
        Value::Uuid(Some(value)) => arguments.add(**value),
        Value::Decimal(None) => arguments.add(None::<Decimal>),
        Value::Decimal(Some(value)) => arguments.add(*value.as_ref()),
    };
    bound.map_err(|error| DbErr::exec(error.to_string()))
}

fn decode_row(row: &PgRow) -> Result<QueryResult, DbErr> {
    let mut columns = Vec::with_capacity(row.len());
    for index in 0..row.len() {
        let name = row.column(index).name().to_owned();
        columns.push((name, decode_cell(row, index)?));
    }
    Ok(QueryResult::from_cells(columns))
}

fn decode_cell(row: &PgRow, index: usize) -> Result<rows::Cell, DbErr> {
    let info = row.column(index).type_info();
    if info.is_null() {
        return Ok(cell_null());
    }
    match info.name() {
        "BOOL" => nullable(row, index, cell_bool),
        "INT2" => nullable(row, index, |value: i16| cell_int(i64::from(value))),
        "INT4" | "OID" => nullable(row, index, |value: i32| cell_int(i64::from(value))),
        "INT8" => nullable(row, index, cell_int),
        "FLOAT4" => nullable(row, index, |value: f32| cell_real(f64::from(value))),
        "FLOAT8" => nullable(row, index, cell_real),
        "TEXT" | "VARCHAR" | "BPCHAR" | "CHAR" | "NAME" | "CITEXT" => {
            nullable(row, index, cell_text)
        }
        "BYTEA" => nullable(row, index, cell_bytes),
        "JSON" | "JSONB" => nullable(row, index, cell_json),
        "UUID" => nullable(row, index, cell_uuid),
        "DATE" => nullable(row, index, cell_date),
        "TIME" => nullable(row, index, cell_time),
        "TIMESTAMP" => nullable(row, index, cell_datetime),
        "TIMESTAMPTZ" => nullable(row, index, cell_timestamptz),
        "NUMERIC" | "DECIMAL" => nullable(row, index, cell_numeric),
        other => Err(DbErr::query(format!(
            "PostgreSQL type {other} has no Fexu decoder"
        ))),
    }
}

fn nullable<T>(
    row: &PgRow,
    index: usize,
    present: impl FnOnce(T) -> rows::Cell,
) -> Result<rows::Cell, DbErr>
where
    T: for<'a> sqlx_core::decode::Decode<'a, Postgres> + sqlx_core::types::Type<Postgres>,
{
    match row.try_get::<Option<T>, _>(index).map_err(decode_err)? {
        Some(value) => Ok(present(value)),
        None => Ok(cell_null()),
    }
}

fn rows_affected(result: &PgQueryResult) -> u64 {
    result.rows_affected()
}

fn decode_err(error: sqlx_core::Error) -> DbErr {
    map_sqlx(error, false)
}

fn map_connect(error: sqlx_core::Error) -> DbErr {
    DbErr::Conn(runtime_from(error))
}

fn map_sqlx(error: sqlx_core::Error, execute: bool) -> DbErr {
    let runtime = runtime_from(error);
    if execute {
        DbErr::Exec(runtime)
    } else {
        DbErr::Query(runtime)
    }
}

fn runtime_from(error: sqlx_core::Error) -> RuntimeErr {
    let constraint = constraint_of(&error);
    RuntimeErr::with_constraint(error.to_string(), constraint)
}

fn constraint_of(error: &sqlx_core::Error) -> Option<ConstraintKind> {
    let sqlx_core::Error::Database(database) = error else {
        return None;
    };
    match database.kind() {
        ErrorKind::UniqueViolation => Some(ConstraintKind::Unique),
        ErrorKind::ForeignKeyViolation => Some(ConstraintKind::ForeignKey),
        ErrorKind::Other if database.code().as_deref() == Some("23001") => {
            Some(ConstraintKind::ForeignKey)
        }
        _ => None,
    }
}
