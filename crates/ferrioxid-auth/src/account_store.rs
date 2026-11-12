//! Tabellen für TOTP, Wiederherstellungscodes und Anmeldeherausforderungen.

use crate::error::AuthError;
use crate::schema::{LOGIN_CHALLENGES, MFA_TOTP, RECOVERY_CODES, SESSIONS};
use crate::store::SessionRow;
use sea_orm::sea_query::{Alias, Expr, PostgresQueryBuilder, Query, SqliteQueryBuilder};
use sea_orm::{ConnectionTrait, DatabaseBackend, QueryResult, Statement, Value};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TotpRow {
    pub nonce: String,
    pub ciphertext: String,
    pub confirmed_at: Option<i64>,
    pub last_used_step: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ChallengeRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub expires_at: i64,
    pub attempts: i64,
    pub consumed_at: Option<i64>,
}

pub async fn replace_totp(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    nonce: &str,
    ciphertext: &str,
    now: i64,
) -> Result<(), AuthError> {
    delete_totp(db, user_id).await?;
    let mut query = Query::insert();
    query
        .into_table(Alias::new(MFA_TOTP))
        .columns([
            Alias::new("user_id"),
            Alias::new("nonce"),
            Alias::new("ciphertext"),
            Alias::new("confirmed_at"),
            Alias::new("created_at"),
            Alias::new("last_used_step"),
        ])
        .values_panic([
            uuid_expr(db, user_id),
            text(nonce),
            text(ciphertext),
            null_number(),
            number(now),
            null_number(),
        ]);
    execute(db, &query).await
}

pub async fn find_totp(
    db: &impl ConnectionTrait,
    user_id: Uuid,
) -> Result<Option<TotpRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("nonce"),
            Alias::new("ciphertext"),
            Alias::new("confirmed_at"),
            Alias::new("last_used_step"),
        ])
        .from(Alias::new(MFA_TOTP))
        .and_where(Expr::col(Alias::new("user_id")).eq(uuid_expr(db, user_id)));
    let Some(row) = one(db, &query).await? else {
        return Ok(None);
    };
    Ok(Some(TotpRow {
        nonce: required_text(&row, "nonce")?,
        ciphertext: required_text(&row, "ciphertext")?,
        confirmed_at: optional_i64(&row, "confirmed_at")?,
        last_used_step: optional_i64(&row, "last_used_step")?,
    }))
}

pub async fn confirm_totp(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    confirmed_at: i64,
    step: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(MFA_TOTP))
        .values([
            (Alias::new("confirmed_at"), number(confirmed_at)),
            (Alias::new("last_used_step"), number(step)),
        ])
        .and_where(Expr::col(Alias::new("user_id")).eq(uuid_expr(db, user_id)))
        .and_where(Expr::col(Alias::new("confirmed_at")).is_null());
    if execute_rows(db, &query).await? == 1 {
        Ok(())
    } else {
        Err(AuthError::Validation(
            "totp enrollment is not pending".into(),
        ))
    }
}

pub async fn set_totp_step(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    step: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(MFA_TOTP))
        .values([(Alias::new("last_used_step"), number(step))])
        .and_where(Expr::col(Alias::new("user_id")).eq(uuid_expr(db, user_id)));
    execute(db, &query).await
}

pub async fn delete_totp(db: &impl ConnectionTrait, user_id: Uuid) -> Result<(), AuthError> {
    let mut query = Query::delete();
    query
        .from_table(Alias::new(MFA_TOTP))
        .and_where(Expr::col(Alias::new("user_id")).eq(uuid_expr(db, user_id)));
    execute(db, &query).await
}

pub async fn insert_recovery(
    db: &impl ConnectionTrait,
    id: Uuid,
    user_id: Uuid,
    code_hash: &str,
    now: i64,
) -> Result<(), AuthError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(RECOVERY_CODES))
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("code_hash"),
            Alias::new("used_at"),
            Alias::new("created_at"),
        ])
        .values_panic([
            uuid_expr(db, id),
            uuid_expr(db, user_id),
            text(code_hash),
            null_number(),
            number(now),
        ]);
    execute(db, &query).await
}

pub async fn delete_recovery(db: &impl ConnectionTrait, user_id: Uuid) -> Result<(), AuthError> {
    let mut query = Query::delete();
    query
        .from_table(Alias::new(RECOVERY_CODES))
        .and_where(Expr::col(Alias::new("user_id")).eq(uuid_expr(db, user_id)));
    execute(db, &query).await
}

pub async fn unused_recovery(
    db: &impl ConnectionTrait,
    user_id: Uuid,
) -> Result<Vec<(Uuid, String)>, AuthError> {
    let mut query = Query::select();
    query
        .columns([Alias::new("id"), Alias::new("code_hash")])
        .from(Alias::new(RECOVERY_CODES))
        .and_where(Expr::col(Alias::new("user_id")).eq(uuid_expr(db, user_id)))
        .and_where(Expr::col(Alias::new("used_at")).is_null());
    let mut rows = Vec::new();
    for row in all(db, &query).await? {
        rows.push((
            required_uuid(&row, "id")?,
            required_text(&row, "code_hash")?,
        ));
    }
    Ok(rows)
}

pub async fn consume_recovery(
    db: &impl ConnectionTrait,
    id: Uuid,
    now: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(RECOVERY_CODES))
        .values([(Alias::new("used_at"), number(now))])
        .and_where(Expr::col(Alias::new("id")).eq(uuid_expr(db, id)))
        .and_where(Expr::col(Alias::new("used_at")).is_null());
    if execute_rows(db, &query).await? == 1 {
        Ok(())
    } else {
        Err(AuthError::InvalidCredentials)
    }
}

pub async fn insert_challenge(
    db: &impl ConnectionTrait,
    id: Uuid,
    user_id: Uuid,
    token_hash: &str,
    now: i64,
    expires_at: i64,
) -> Result<(), AuthError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(LOGIN_CHALLENGES))
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("token_hash"),
            Alias::new("expires_at"),
            Alias::new("consumed_at"),
            Alias::new("attempts"),
            Alias::new("created_at"),
        ])
        .values_panic([
            uuid_expr(db, id),
            uuid_expr(db, user_id),
            text(token_hash),
            number(expires_at),
            null_number(),
            number(0),
            number(now),
        ]);
    execute(db, &query).await
}

pub async fn find_challenge(
    db: &impl ConnectionTrait,
    token_hash: &str,
) -> Result<Option<ChallengeRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("expires_at"),
            Alias::new("attempts"),
            Alias::new("consumed_at"),
        ])
        .from(Alias::new(LOGIN_CHALLENGES))
        .and_where(Expr::col(Alias::new("token_hash")).eq(token_hash));
    let Some(row) = one(db, &query).await? else {
        return Ok(None);
    };
    Ok(Some(ChallengeRow {
        id: required_uuid(&row, "id")?,
        user_id: required_uuid(&row, "user_id")?,
        expires_at: required_i64(&row, "expires_at")?,
        attempts: required_i64(&row, "attempts")?,
        consumed_at: optional_i64(&row, "consumed_at")?,
    }))
}

pub async fn bump_challenge(
    db: &impl ConnectionTrait,
    id: Uuid,
    attempts: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(LOGIN_CHALLENGES))
        .values([(Alias::new("attempts"), number(attempts))])
        .and_where(Expr::col(Alias::new("id")).eq(uuid_expr(db, id)))
        .and_where(Expr::col(Alias::new("consumed_at")).is_null());
    execute(db, &query).await
}

pub async fn consume_challenge(
    db: &impl ConnectionTrait,
    id: Uuid,
    now: i64,
) -> Result<(), AuthError> {
    let mut query = Query::update();
    query
        .table(Alias::new(LOGIN_CHALLENGES))
        .values([(Alias::new("consumed_at"), number(now))])
        .and_where(Expr::col(Alias::new("id")).eq(uuid_expr(db, id)))
        .and_where(Expr::col(Alias::new("consumed_at")).is_null());
    if execute_rows(db, &query).await? == 1 {
        Ok(())
    } else {
        Err(AuthError::InvalidCredentials)
    }
}

pub async fn delete_user_challenges(
    db: &impl ConnectionTrait,
    user_id: Uuid,
) -> Result<(), AuthError> {
    let mut query = Query::delete();
    query
        .from_table(Alias::new(LOGIN_CHALLENGES))
        .and_where(Expr::col(Alias::new("user_id")).eq(uuid_expr(db, user_id)));
    execute(db, &query).await
}

pub async fn list_open_sessions(
    db: &impl ConnectionTrait,
    user_id: Uuid,
) -> Result<Vec<SessionRow>, AuthError> {
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("user_id"),
            Alias::new("token_hash"),
            Alias::new("csrf_token_hash"),
            Alias::new("created_at"),
            Alias::new("expires_at"),
            Alias::new("last_seen_at"),
            Alias::new("revoked_at"),
        ])
        .from(Alias::new(SESSIONS))
        .and_where(Expr::col(Alias::new("user_id")).eq(uuid_expr(db, user_id)))
        .and_where(Expr::col(Alias::new("revoked_at")).is_null());
    let mut rows = Vec::new();
    for row in all(db, &query).await? {
        rows.push(SessionRow {
            id: required_uuid(&row, "id")?,
            user_id: required_uuid(&row, "user_id")?,
            token_hash: required_text(&row, "token_hash")?,
            csrf_token_hash: required_text(&row, "csrf_token_hash")?,
            created_at: required_i64(&row, "created_at")?,
            expires_at: required_i64(&row, "expires_at")?,
            last_seen_at: required_i64(&row, "last_seen_at")?,
            revoked_at: optional_i64(&row, "revoked_at")?,
        });
    }
    Ok(rows)
}

fn uuid_expr(db: &impl ConnectionTrait, id: Uuid) -> sea_orm::sea_query::SimpleExpr {
    match db.get_database_backend() {
        DatabaseBackend::Postgres => Expr::value(Value::Uuid(Some(Box::new(id)))),
        DatabaseBackend::Sqlite | DatabaseBackend::MySql => Expr::value(id.to_string()),
    }
}

fn text(value: &str) -> sea_orm::sea_query::SimpleExpr {
    Expr::value(value)
}

fn number(value: i64) -> sea_orm::sea_query::SimpleExpr {
    Expr::value(value)
}

fn null_number() -> sea_orm::sea_query::SimpleExpr {
    Expr::value(Value::BigInt(None))
}

async fn execute(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<(), AuthError> {
    execute_rows(db, query).await?;
    Ok(())
}

async fn execute_rows(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<u64, AuthError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(AuthError::Validation("MySQL is not supported".into()))
        }
    };
    let result = db
        .execute(Statement::from_sql_and_values(backend, sql, values.0))
        .await
        .map_err(AuthError::database)?;
    Ok(result.rows_affected())
}

async fn one(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Option<QueryResult>, AuthError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(AuthError::Validation("MySQL is not supported".into()))
        }
    };
    db.query_one(Statement::from_sql_and_values(backend, sql, values.0))
        .await
        .map_err(AuthError::database)
}

async fn all(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Vec<QueryResult>, AuthError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(AuthError::Validation("MySQL is not supported".into()))
        }
    };
    db.query_all(Statement::from_sql_and_values(backend, sql, values.0))
        .await
        .map_err(AuthError::database)
}

fn required_text(row: &QueryResult, column: &str) -> Result<String, AuthError> {
    row.try_get("", column)
        .map_err(|_| AuthError::database(format!("column {column} is not text")))
}

fn required_i64(row: &QueryResult, column: &str) -> Result<i64, AuthError> {
    if let Ok(value) = row.try_get::<i64>("", column) {
        return Ok(value);
    }
    let value: i32 = row
        .try_get("", column)
        .map_err(|_| AuthError::database(format!("column {column} is not an integer")))?;
    Ok(i64::from(value))
}

fn optional_i64(row: &QueryResult, column: &str) -> Result<Option<i64>, AuthError> {
    if let Ok(value) = row.try_get::<Option<i64>>("", column) {
        return Ok(value);
    }
    let value: Option<i32> = row
        .try_get("", column)
        .map_err(|_| AuthError::database(format!("column {column} is not an integer")))?;
    Ok(value.map(i64::from))
}

fn required_uuid(row: &QueryResult, column: &str) -> Result<Uuid, AuthError> {
    if let Ok(value) = row.try_get::<Uuid>("", column) {
        return Ok(value);
    }
    let value: String = required_text(row, column)?;
    Uuid::parse_str(&value)
        .map_err(|_| AuthError::database(format!("column {column} is not a uuid")))
}
