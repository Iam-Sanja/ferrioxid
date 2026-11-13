//! Kurzlebige, sitzungsgebundene Formular- und Sammelaktionsentwürfe.

use fexu_auth::ADMIN_DRAFTS;
use fexu_core::FexuError;
use sea_orm::sea_query::{Alias, Expr, PostgresQueryBuilder, Query, SqliteQueryBuilder};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value as DbValue};
use serde_json::{Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const TTL_SECS: i64 = 30 * 60;
pub const MAX_BYTES: usize = 8 * 1024;
pub const MAX_PER_SESSION: i64 = 20;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Draft {
    pub id: Uuid,
    pub model_key: String,
    pub record_id: Option<String>,
    pub purpose: String,
    pub values: Map<String, Value>,
}

pub async fn insert(
    db: &impl ConnectionTrait,
    session_id: Uuid,
    user_id: Uuid,
    model_key: &str,
    record_id: Option<&str>,
    purpose: &str,
    values: &Map<String, Value>,
) -> Result<Uuid, FexuError> {
    let payload = serde_json::to_string(values)
        .map_err(|_| FexuError::Validation("draft payload is invalid".into()))?;
    if payload.len() > MAX_BYTES || model_key.len() > 80 || purpose.len() > 16 {
        return Err(FexuError::Validation("draft payload is too large".into()));
    }
    purge(db, session_id).await?;
    let id = Uuid::new_v4();
    let now = unix_now();
    let backend = db.get_database_backend();
    let mut query = Query::insert();
    let record = record_id
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let values = [
        DbValue::from(id.to_string()).into(),
        DbValue::from(session_id.to_string()).into(),
        DbValue::from(user_id.to_string()).into(),
        DbValue::from(model_key.to_owned()).into(),
        match record {
            Some(value) => DbValue::from(value).into(),
            None => DbValue::String(None).into(),
        },
        DbValue::from(purpose.to_owned()).into(),
        DbValue::from(payload).into(),
        DbValue::from(now).into(),
        DbValue::from(now + TTL_SECS).into(),
    ];
    query
        .into_table(Alias::new(ADMIN_DRAFTS))
        .columns([
            Alias::new("id"),
            Alias::new("session_id"),
            Alias::new("user_id"),
            Alias::new("model_key"),
            Alias::new("record_id"),
            Alias::new("purpose"),
            Alias::new("payload"),
            Alias::new("created_at"),
            Alias::new("expires_at"),
        ])
        .values_panic(values);
    execute(db, backend, &query).await?;
    Ok(id)
}

pub async fn load(
    db: &impl ConnectionTrait,
    id: Uuid,
    session_id: Uuid,
    user_id: Uuid,
    model_key: &str,
    purpose: &str,
) -> Result<Option<Draft>, FexuError> {
    let backend = db.get_database_backend();
    let mut query = Query::select();
    query
        .columns([
            Alias::new("model_key"),
            Alias::new("record_id"),
            Alias::new("purpose"),
            Alias::new("payload"),
            Alias::new("expires_at"),
            Alias::new("session_id"),
            Alias::new("user_id"),
        ])
        .from(Alias::new(ADMIN_DRAFTS))
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    let (sql, values) = build(backend, &query)?;
    let Some(row) = db
        .query_one(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(FexuError::database)?
    else {
        return Ok(None);
    };
    let stored_session = read_uuid(&row, "session_id")?;
    let stored_user = read_uuid(&row, "user_id")?;
    let expires_at = row
        .try_get::<i64>("", "expires_at")
        .map_err(FexuError::database)?;
    let stored_model = row
        .try_get::<String>("", "model_key")
        .map_err(FexuError::database)?;
    let stored_purpose = row
        .try_get::<String>("", "purpose")
        .map_err(FexuError::database)?;
    if stored_session != session_id
        || stored_user != user_id
        || stored_model != model_key
        || stored_purpose != purpose
        || expires_at < unix_now()
    {
        return Ok(None);
    }
    let payload = row
        .try_get::<String>("", "payload")
        .map_err(FexuError::database)?;
    let values = serde_json::from_str::<Map<String, Value>>(&payload).unwrap_or_default();
    let record_id = row.try_get::<String>("", "record_id").ok();
    Ok(Some(Draft {
        id,
        model_key: stored_model,
        record_id,
        purpose: stored_purpose,
        values,
    }))
}

pub async fn delete(
    db: &impl ConnectionTrait,
    id: Uuid,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<(), FexuError> {
    let backend = db.get_database_backend();
    let mut query = Query::delete();
    query
        .from_table(Alias::new(ADMIN_DRAFTS))
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
        .and_where(Expr::col(Alias::new("session_id")).eq(session_id.to_string()))
        .and_where(Expr::col(Alias::new("user_id")).eq(user_id.to_string()));
    execute(db, backend, &query).await
}

async fn purge(db: &impl ConnectionTrait, session_id: Uuid) -> Result<(), FexuError> {
    let now = unix_now();
    let backend = db.get_database_backend();
    let mut expired = Query::delete();
    expired
        .from_table(Alias::new(ADMIN_DRAFTS))
        .and_where(Expr::col(Alias::new("expires_at")).lt(now));
    execute(db, backend, &expired).await?;
    let mut count = Query::select();
    count
        .expr_as(
            Expr::col(Alias::new("id")).count(),
            Alias::new("draft_count"),
        )
        .from(Alias::new(ADMIN_DRAFTS))
        .and_where(Expr::col(Alias::new("session_id")).eq(session_id.to_string()));
    let (sql, values) = build(backend, &count)?;
    let row = db
        .query_one(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(FexuError::database)?
        .ok_or_else(|| FexuError::Validation("draft count is missing".into()))?;
    let total = row
        .try_get::<i64>("", "draft_count")
        .or_else(|_| row.try_get::<i32>("", "draft_count").map(i64::from))
        .map_err(FexuError::database)?;
    if total < MAX_PER_SESSION {
        return Ok(());
    }
    let mut oldest = Query::select();
    oldest
        .column(Alias::new("id"))
        .from(Alias::new(ADMIN_DRAFTS))
        .and_where(Expr::col(Alias::new("session_id")).eq(session_id.to_string()))
        .order_by(Alias::new("created_at"), sea_orm::sea_query::Order::Asc)
        .limit((total - MAX_PER_SESSION + 1) as u64);
    let (sql, values) = build(backend, &oldest)?;
    let rows = db
        .query_all(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(FexuError::database)?;
    for row in rows {
        let id = read_uuid(&row, "id")?;
        let mut query = Query::delete();
        query
            .from_table(Alias::new(ADMIN_DRAFTS))
            .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
            .and_where(Expr::col(Alias::new("session_id")).eq(session_id.to_string()));
        execute(db, backend, &query).await?;
    }
    Ok(())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn read_uuid(row: &sea_orm::QueryResult, column: &str) -> Result<Uuid, FexuError> {
    if let Ok(value) = row.try_get::<Uuid>("", column) {
        return Ok(value);
    }
    let text = row
        .try_get::<String>("", column)
        .map_err(FexuError::database)?;
    Uuid::parse_str(&text).map_err(|_| FexuError::Validation("stored uuid is invalid".into()))
}

async fn execute(
    db: &impl ConnectionTrait,
    backend: DatabaseBackend,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<(), FexuError> {
    let (sql, values) = build(backend, query)?;
    db.execute(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map(|_| ())
        .map_err(FexuError::database)
}

fn build(
    backend: DatabaseBackend,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<(String, Vec<DbValue>), FexuError> {
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(FexuError::Validation(
                "MySQL wird nicht unterstützt.".into(),
            ))
        }
    };
    Ok((sql, values.0))
}
