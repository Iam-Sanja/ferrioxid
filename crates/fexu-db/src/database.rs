//! Shared execution for PostgreSQL and SQLite.
//!
//! SQL text still comes from SeaQuery. This module binds the parameters and
//! reads the rows. SQLite work runs on the dedicated threads in [`crate::sqlite`].

use crate::error::DbErr;
use crate::postgres::{PgSession, PgTransaction};
use crate::sqlite::{self, SqlitePin, SqliteSession, SqliteTxn};
use async_trait::async_trait;
use sea_query::{Value, Values};
use std::sync::Arc;
use std::time::Duration;

const SQLITE_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
const SQLITE_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseBackend {
    MySql,
    Postgres,
    Sqlite,
}

pub type DbBackend = DatabaseBackend;

#[derive(Debug, Clone)]
pub struct Statement {
    pub sql: String,
    pub values: Option<Values>,
    pub db_backend: DatabaseBackend,
}

impl Statement {
    pub fn from_string(db_backend: DatabaseBackend, sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            values: None,
            db_backend,
        }
    }

    pub fn from_sql_and_values<I>(
        db_backend: DatabaseBackend,
        sql: impl Into<String>,
        values: I,
    ) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        Self {
            sql: sql.into(),
            values: Some(Values(values.into_iter().collect())),
            db_backend,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExecResult {
    rows_affected: u64,
}

impl ExecResult {
    pub fn rows_affected(&self) -> u64 {
        self.rows_affected
    }

    pub(crate) fn from_count(rows_affected: u64) -> Self {
        Self { rows_affected }
    }
}

pub use crate::rows::{QueryResult, TryGetable};

#[derive(Clone)]
pub struct ConnectOptions {
    url: String,
    max_connections: u32,
    min_connections: u32,
}

pub struct SqlitePragmaBuilder;

impl SqlitePragmaBuilder {
    pub fn pragma(self, _key: &str, _value: &str) -> Self {
        self
    }
}

impl ConnectOptions {
    pub fn new(url: impl Into<String>) -> Self {
        let url = url.into();
        let max_connections = if url.starts_with("sqlite:") { 1 } else { 10 };
        Self {
            url,
            max_connections,
            min_connections: 0,
        }
    }

    pub fn max_connections(&mut self, max_connections: u32) -> &mut Self {
        self.max_connections = max_connections;
        self
    }

    pub fn min_connections(&mut self, min_connections: u32) -> &mut Self {
        self.min_connections = min_connections;
        self
    }

    /// Retained so existing connect setup still compiles.
    /// Foreign keys and the busy timeout are always set by the SQLite session.
    pub fn map_sqlx_sqlite_opts<F>(&mut self, apply: F) -> &mut Self
    where
        F: FnOnce(SqlitePragmaBuilder) -> SqlitePragmaBuilder,
    {
        let _ = apply(SqlitePragmaBuilder);
        self
    }
}

pub struct Database;

impl Database {
    pub async fn connect(options: impl Into<ConnectOptions>) -> Result<DatabaseConnection, DbErr> {
        DatabaseConnection::open(options.into()).await
    }
}

impl From<&str> for ConnectOptions {
    fn from(url: &str) -> Self {
        Self::new(url)
    }
}

impl From<&String> for ConnectOptions {
    fn from(url: &String) -> Self {
        Self::new(url)
    }
}

impl From<String> for ConnectOptions {
    fn from(url: String) -> Self {
        Self::new(url)
    }
}

#[derive(Clone)]
pub struct DatabaseConnection {
    inner: Conn,
}

#[derive(Clone)]
enum Conn {
    Postgres(PgSession),
    Sqlite(Arc<SqlitePool>),
}

pub struct DatabaseTransaction {
    inner: Tx,
}

enum Tx {
    Postgres(PgTransaction),
    Sqlite(SqliteHold),
}

struct SqliteHold {
    _lease: Lease,
    txn: Option<SqliteTxn>,
}

pub struct PinnedSqlite {
    _lease: Lease,
    pin: SqlitePin,
}

struct SqlitePool {
    sessions: Vec<SqliteSession>,
    free: tokio::sync::Mutex<Vec<usize>>,
    available: tokio::sync::Notify,
}

struct Lease {
    pool: Arc<SqlitePool>,
    index: usize,
}

impl DatabaseConnection {
    async fn open(options: ConnectOptions) -> Result<Self, DbErr> {
        if options.max_connections == 0 {
            return Err(DbErr::exec(
                "the connection pool needs at least one connection",
            ));
        }
        if options.url.starts_with("sqlite:") {
            if options.url.contains("memory") && options.max_connections > 1 {
                return Err(DbErr::exec(
                    "an in-memory SQLite database uses one connection",
                ));
            }
            let pool = SqlitePool::open(&options.url, options.max_connections).await?;
            return Ok(Self {
                inner: Conn::Sqlite(Arc::new(pool)),
            });
        }
        if options.url.starts_with("postgres://") || options.url.starts_with("postgresql://") {
            let session = PgSession::connect(
                &options.url,
                options.max_connections,
                options.min_connections,
            )
            .await?;
            return Ok(Self {
                inner: Conn::Postgres(session),
            });
        }
        Err(DbErr::exec(
            "the database URL must start with sqlite://, sqlite::memory:, postgres://, or postgresql://",
        ))
    }

    pub async fn close(self) -> Result<(), DbErr> {
        match self.inner {
            Conn::Postgres(session) => session.close().await,
            Conn::Sqlite(pool) => pool.close().await,
        }
    }

    pub async fn pin_sqlite(&self) -> Result<PinnedSqlite, DbErr> {
        let Conn::Sqlite(pool) = &self.inner else {
            return Err(DbErr::exec(
                "SQLite pin is only available for SQLite connections",
            ));
        };
        let lease = pool.acquire().await?;
        let pin = lease.session().pin().await?;
        Ok(PinnedSqlite { _lease: lease, pin })
    }
}

impl DatabaseTransaction {
    pub async fn commit(mut self) -> Result<(), DbErr> {
        match self.inner {
            Tx::Postgres(transaction) => transaction.commit().await,
            Tx::Sqlite(ref mut hold) => {
                let txn = hold
                    .txn
                    .take()
                    .ok_or_else(|| DbErr::exec("SQLite transaction is closed"))?;
                txn.commit().await
            }
        }
    }

    pub async fn rollback(mut self) -> Result<(), DbErr> {
        match self.inner {
            Tx::Postgres(transaction) => transaction.rollback().await,
            Tx::Sqlite(ref mut hold) => {
                let txn = hold
                    .txn
                    .take()
                    .ok_or_else(|| DbErr::exec("SQLite transaction is closed"))?;
                txn.rollback().await
            }
        }
    }
}

#[async_trait]
pub trait ConnectionTrait: Send + Sync {
    fn get_database_backend(&self) -> DatabaseBackend;
    async fn execute(&self, statement: Statement) -> Result<ExecResult, DbErr>;
    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr>;
    async fn query_one(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr>;
    async fn query_all(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr>;
}

#[async_trait]
pub trait TransactionTrait: ConnectionTrait {
    async fn begin(&self) -> Result<DatabaseTransaction, DbErr>;
}

#[async_trait]
impl ConnectionTrait for DatabaseConnection {
    fn get_database_backend(&self) -> DatabaseBackend {
        match &self.inner {
            Conn::Postgres(_) => DatabaseBackend::Postgres,
            Conn::Sqlite(_) => DatabaseBackend::Sqlite,
        }
    }

    async fn execute(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        match &self.inner {
            Conn::Postgres(session) => {
                let values = statement_values(&statement);
                Ok(ExecResult::from_count(
                    session.execute(&statement.sql, &values).await?,
                ))
            }
            Conn::Sqlite(pool) => {
                let lease = pool.acquire().await?;
                let values = sqlite_values(&statement)?;
                Ok(ExecResult::from_count(
                    lease.session().execute(&statement.sql, &values).await?,
                ))
            }
        }
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        match &self.inner {
            Conn::Postgres(session) => Ok(ExecResult::from_count(
                session.execute_unprepared(sql).await?,
            )),
            Conn::Sqlite(pool) => {
                let lease = pool.acquire().await?;
                Ok(ExecResult::from_count(
                    lease.session().execute_batch(sql).await?,
                ))
            }
        }
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        Ok(self.query_all(statement).await?.into_iter().next())
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        match &self.inner {
            Conn::Postgres(session) => {
                let values = statement_values(&statement);
                session.query_all(&statement.sql, &values).await
            }
            Conn::Sqlite(pool) => {
                let lease = pool.acquire().await?;
                let values = sqlite_values(&statement)?;
                Ok(lease
                    .session()
                    .query(&statement.sql, &values)
                    .await?
                    .into_iter()
                    .map(QueryResult::from_sqlite)
                    .collect())
            }
        }
    }
}

#[async_trait]
impl TransactionTrait for DatabaseConnection {
    async fn begin(&self) -> Result<DatabaseTransaction, DbErr> {
        match &self.inner {
            Conn::Postgres(session) => Ok(DatabaseTransaction {
                inner: Tx::Postgres(session.begin().await?),
            }),
            Conn::Sqlite(pool) => {
                let lease = pool.acquire().await?;
                let txn = lease.session().transaction().await?;
                Ok(DatabaseTransaction {
                    inner: Tx::Sqlite(SqliteHold {
                        _lease: lease,
                        txn: Some(txn),
                    }),
                })
            }
        }
    }
}

#[async_trait]
impl ConnectionTrait for DatabaseTransaction {
    fn get_database_backend(&self) -> DatabaseBackend {
        match &self.inner {
            Tx::Postgres(_) => DatabaseBackend::Postgres,
            Tx::Sqlite(_) => DatabaseBackend::Sqlite,
        }
    }

    async fn execute(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        match &self.inner {
            Tx::Postgres(transaction) => {
                let values = statement_values(&statement);
                Ok(ExecResult::from_count(
                    transaction.execute(&statement.sql, &values).await?,
                ))
            }
            Tx::Sqlite(hold) => {
                let values = sqlite_values(&statement)?;
                let txn = hold
                    .txn
                    .as_ref()
                    .ok_or_else(|| DbErr::exec("SQLite transaction is closed"))?;
                Ok(ExecResult::from_count(
                    txn.execute(&statement.sql, &values).await?,
                ))
            }
        }
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        let sql = sql.to_owned();
        match &self.inner {
            Tx::Postgres(transaction) => Ok(ExecResult::from_count(
                transaction.execute_unprepared(&sql).await?,
            )),
            Tx::Sqlite(hold) => {
                let txn = hold
                    .txn
                    .as_ref()
                    .ok_or_else(|| DbErr::exec("SQLite transaction is closed"))?;
                Ok(ExecResult::from_count(txn.execute_batch(&sql).await?))
            }
        }
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        Ok(self.query_all(statement).await?.into_iter().next())
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        match &self.inner {
            Tx::Postgres(transaction) => {
                let values = statement_values(&statement);
                transaction.query_all(&statement.sql, &values).await
            }
            Tx::Sqlite(hold) => {
                let values = sqlite_values(&statement)?;
                let txn = hold
                    .txn
                    .as_ref()
                    .ok_or_else(|| DbErr::exec("SQLite transaction is closed"))?;
                Ok(txn
                    .query(&statement.sql, &values)
                    .await?
                    .into_iter()
                    .map(QueryResult::from_sqlite)
                    .collect())
            }
        }
    }
}

#[async_trait]
impl ConnectionTrait for PinnedSqlite {
    fn get_database_backend(&self) -> DatabaseBackend {
        DatabaseBackend::Sqlite
    }

    async fn execute(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        let values = sqlite_values(&statement)?;
        Ok(ExecResult::from_count(
            self.pin.execute(&statement.sql, &values).await?,
        ))
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        Ok(ExecResult::from_count(self.pin.execute_batch(sql).await?))
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        Ok(self.query_all(statement).await?.into_iter().next())
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        let values = sqlite_values(&statement)?;
        Ok(self
            .pin
            .query(&statement.sql, &values)
            .await?
            .into_iter()
            .map(QueryResult::from_sqlite)
            .collect())
    }
}

impl SqlitePool {
    async fn open(url: &str, size: u32) -> Result<Self, DbErr> {
        let mut sessions = Vec::with_capacity(size as usize);
        for _ in 0..size {
            sessions.push(SqliteSession::open_url(url).await?);
        }
        let free = (0..sessions.len()).rev().collect();
        Ok(Self {
            sessions,
            free: tokio::sync::Mutex::new(free),
            available: tokio::sync::Notify::new(),
        })
    }

    async fn acquire(self: &Arc<Self>) -> Result<Lease, DbErr> {
        let deadline = tokio::time::Instant::now() + SQLITE_ACQUIRE_TIMEOUT;
        loop {
            if let Some(index) = self.free.lock().await.pop() {
                return Ok(Lease {
                    pool: Arc::clone(self),
                    index,
                });
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(DbErr::exec("timed out waiting for a SQLite connection"));
            }
            if tokio::time::timeout(remaining, self.available.notified())
                .await
                .is_err()
            {
                return Err(DbErr::exec("timed out waiting for a SQLite connection"));
            }
        }
    }

    async fn close(&self) -> Result<(), DbErr> {
        let deadline = tokio::time::Instant::now() + SQLITE_CLOSE_TIMEOUT;
        loop {
            if self.free.lock().await.len() == self.sessions.len() {
                break;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(DbErr::exec(
                    "timed out waiting for SQLite connections to return before shutdown",
                ));
            }
            let _ = tokio::time::timeout(remaining, self.available.notified()).await;
        }
        for session in &self.sessions {
            session.close().await?;
        }
        Ok(())
    }
}

impl Lease {
    fn session(&self) -> &SqliteSession {
        &self.pool.sessions[self.index]
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut free) = self.pool.free.try_lock() {
            free.push(self.index);
            self.pool.available.notify_one();
        } else {
            let pool = Arc::clone(&self.pool);
            let index = self.index;
            tokio::spawn(async move {
                pool.free.lock().await.push(index);
                pool.available.notify_one();
            });
        }
    }
}

fn statement_values(statement: &Statement) -> Vec<Value> {
    statement
        .values
        .as_ref()
        .map(|values| values.0.clone())
        .unwrap_or_default()
}

fn sqlite_values(statement: &Statement) -> Result<Vec<rusqlite::types::Value>, DbErr> {
    statement_values(statement)
        .iter()
        .map(sqlite::bind_value)
        .collect()
}
