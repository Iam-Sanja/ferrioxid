//! SQLite execution for one pinned connection, with PostgreSQL left on SeaORM.
//!
//! Synchronous rusqlite calls run on a dedicated thread named `fexu-sqlite`.
//! Tokio tasks send the work over a channel and wait on an oneshot, so a query
//! does not occupy a runtime worker.
//!
//! The session is the `connect()` rule of one SQLite connection. A transaction
//! or a [`PinnedConnection`] holds that connection until commit, rollback, or
//! restore. If foreign-key enforcement is still off when the pin is dropped,
//! the connection is closed and the same file is opened again. That is the
//! `PinnedSqlite` rule: a connection with enforcement suspended must not return
//! to the pool.
//!
//! This crate is its own Cargo workspace. The Fexu workspace still builds the
//! vendored `sqlx-sqlite` pin, and that pin cannot share a graph with
//! `libsqlite3-sys` 0.40.0.

use rusqlite::params_from_iter;
use rusqlite::types::{ToSql, ToSqlOutput, ValueRef};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

const BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);

thread_local! {
    static ON_SQLITE_THREAD: Cell<bool> = const { Cell::new(false) };
}

#[derive(Clone, Debug, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl ToSql for SqlValue {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let value = match self {
            Self::Null => rusqlite::types::Value::Null,
            Self::Integer(value) => rusqlite::types::Value::Integer(*value),
            Self::Real(value) => rusqlite::types::Value::Real(*value),
            Self::Text(value) => rusqlite::types::Value::Text(value.clone()),
            Self::Blob(value) => rusqlite::types::Value::Blob(value.clone()),
        };
        Ok(ToSqlOutput::Owned(value))
    }
}

#[derive(Debug)]
pub struct SqliteError {
    message: String,
}

impl SqliteError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn stopped() -> Self {
        Self::new("SQLite connection is closed")
    }

    fn rusqlite(error: rusqlite::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl std::fmt::Display for SqliteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SqliteError {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Memory,
    File(PathBuf),
}

enum Job {
    Run(Box<dyn FnOnce(&mut Worker) + Send>),
    Shutdown(SyncSender<()>),
}

struct Worker {
    connection: rusqlite::Connection,
    target: Target,
    in_transaction: bool,
}

impl Worker {
    fn open(target: Target) -> Result<Self, SqliteError> {
        let connection = open_connection(&target)?;
        Ok(Self {
            connection,
            target,
            in_transaction: false,
        })
    }

    fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqliteError> {
        self.connection
            .execute(sql, params_from_iter(params.iter()))
            .map(|changed| changed as u64)
            .map_err(SqliteError::rusqlite)
    }

    fn query(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<Vec<SqlValue>>, SqliteError> {
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(SqliteError::rusqlite)?;
        let width = statement.column_count();
        let mut rows = statement
            .query(params_from_iter(params.iter()))
            .map_err(SqliteError::rusqlite)?;
        let mut records = Vec::new();
        loop {
            let Some(row) = rows.next().map_err(SqliteError::rusqlite)? else {
                break;
            };
            let mut record = Vec::with_capacity(width);
            for index in 0..width {
                record.push(read_value(row, index)?);
            }
            records.push(record);
        }
        Ok(records)
    }

    fn begin(&mut self) -> Result<(), SqliteError> {
        if self.in_transaction {
            return Err(SqliteError::new("SQLite transaction is already open"));
        }
        self.connection
            .execute_batch("BEGIN")
            .map_err(SqliteError::rusqlite)?;
        self.in_transaction = true;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), SqliteError> {
        if !self.in_transaction {
            return Err(SqliteError::new("SQLite transaction is not open"));
        }
        self.connection
            .execute_batch("COMMIT")
            .map_err(SqliteError::rusqlite)?;
        self.in_transaction = false;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), SqliteError> {
        if !self.in_transaction {
            return Ok(());
        }
        self.connection
            .execute_batch("ROLLBACK")
            .map_err(SqliteError::rusqlite)?;
        self.in_transaction = false;
        Ok(())
    }

    fn suspend_foreign_keys(&mut self) -> Result<(), SqliteError> {
        if self.in_transaction {
            return Err(SqliteError::new(
                "Foreign keys must be suspended before the SQLite transaction starts",
            ));
        }
        self.connection
            .execute_batch("PRAGMA foreign_keys = OFF")
            .map_err(SqliteError::rusqlite)
    }

    fn restore_foreign_keys(&mut self) -> Result<(), SqliteError> {
        if self.in_transaction {
            return Err(SqliteError::new(
                "Foreign keys must be restored after the SQLite transaction ends",
            ));
        }
        self.connection
            .execute_batch("PRAGMA foreign_keys = ON")
            .map_err(SqliteError::rusqlite)?;
        let enabled: i64 = self
            .connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .map_err(SqliteError::rusqlite)?;
        if enabled != 1 {
            return Err(SqliteError::new(
                "Foreign key enforcement stayed off on the pinned SQLite connection",
            ));
        }
        Ok(())
    }

    fn replace(&mut self) -> Result<(), SqliteError> {
        if self.in_transaction {
            self.connection
                .execute_batch("ROLLBACK")
                .map_err(SqliteError::rusqlite)?;
            self.in_transaction = false;
        }
        self.connection = open_connection(&self.target)?;
        Ok(())
    }
}

fn open_connection(target: &Target) -> Result<rusqlite::Connection, SqliteError> {
    let connection = match target {
        Target::Memory => rusqlite::Connection::open_in_memory(),
        Target::File(path) => rusqlite::Connection::open(path),
    }
    .map_err(SqliteError::rusqlite)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(SqliteError::rusqlite)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .map_err(SqliteError::rusqlite)?;
    if matches!(target, Target::File(_)) {
        let mode: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(SqliteError::rusqlite)?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(SqliteError::new(format!(
                "SQLite did not enable WAL; journal_mode={mode}"
            )));
        }
    }
    Ok(connection)
}

fn read_value(row: &rusqlite::Row<'_>, index: usize) -> Result<SqlValue, SqliteError> {
    let value = row.get_ref(index).map_err(SqliteError::rusqlite)?;
    Ok(match value {
        ValueRef::Null => SqlValue::Null,
        ValueRef::Integer(value) => SqlValue::Integer(value),
        ValueRef::Real(value) => SqlValue::Real(value),
        ValueRef::Text(value) => SqlValue::Text(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => SqlValue::Blob(value.to_vec()),
    })
}

fn serve(receiver: Receiver<Job>, mut worker: Worker) {
    ON_SQLITE_THREAD.with(|flag| flag.set(true));
    while let Ok(job) = receiver.recv() {
        match job {
            Job::Run(job) => job(&mut worker),
            Job::Shutdown(reply) => {
                drop(worker);
                let _ = reply.send(());
                break;
            }
        }
    }
}

struct Inner {
    cmd: mpsc::Sender<Job>,
    gate: Arc<tokio::sync::Mutex<()>>,
    thread: std::sync::Mutex<Option<JoinHandle<()>>>,
    closed: AtomicBool,
    discard_error: std::sync::Mutex<Option<String>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let (reply, ready) = mpsc::sync_channel(1);
        let sent = self.cmd.send(Job::Shutdown(reply)).is_ok();
        let handle = self
            .thread
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        let finish = move || {
            if sent {
                let _ = ready.recv_timeout(Duration::from_secs(5));
            }
            if let Some(handle) = handle {
                let _ = handle.join();
            }
        };
        if tokio::runtime::Handle::try_current().is_ok() {
            drop(tokio::task::spawn_blocking(finish));
        } else {
            finish();
        }
    }
}

/// One SQLite connection owned by the `fexu-sqlite` thread.
#[derive(Clone)]
pub struct SqliteSession {
    inner: Arc<Inner>,
}

impl SqliteSession {
    pub async fn open_memory() -> Result<Self, SqliteError> {
        Self::start(Target::Memory).await
    }

    pub async fn open_url(url: &str) -> Result<Self, SqliteError> {
        Self::start(parse_sqlite_url(url)?).await
    }

    pub async fn open_path(path: impl AsRef<Path>) -> Result<Self, SqliteError> {
        Self::start(Target::File(path.as_ref().to_path_buf())).await
    }

    pub async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqliteError> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.dispatch(move |worker| worker.execute(&sql, &params))
            .await
    }

    pub async fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<Vec<SqlValue>>, SqliteError> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.dispatch(move |worker| worker.query(&sql, &params))
            .await
    }

    pub async fn sqlite_version(&self) -> Result<String, SqliteError> {
        let rows = self.query("SELECT sqlite_version()", &[]).await?;
        match rows.first().and_then(|row| row.first()) {
            Some(SqlValue::Text(version)) => Ok(version.clone()),
            _ => Err(SqliteError::new("sqlite_version() returned no text")),
        }
    }

    pub async fn begin(&self) -> Result<SqliteTransaction, SqliteError> {
        let permit = self.inner.gate.clone().lock_owned().await;
        self.dispatch_locked(|worker| worker.begin()).await?;
        Ok(SqliteTransaction {
            session: self.clone(),
            permit,
            finished: false,
        })
    }

    /// Exclusive lease of the single connection, including its foreign-key flag.
    pub async fn pin(&self) -> Result<PinnedConnection, SqliteError> {
        let permit = self.inner.gate.clone().lock_owned().await;
        Ok(PinnedConnection {
            session: self.clone(),
            permit,
            enforcement_intact: true,
        })
    }

    /// Sleep on the SQLite thread. The runtime worker that awaited this stays free.
    pub async fn wait_on_connection(&self, duration: Duration) -> Result<(), SqliteError> {
        self.dispatch(move |worker| {
            let _probe: i64 = worker
                .connection
                .query_row("SELECT 1", [], |row| row.get(0))
                .map_err(SqliteError::rusqlite)?;
            std::thread::sleep(duration);
            Ok(())
        })
        .await
    }

    pub async fn connection_thread_name(&self) -> Result<String, SqliteError> {
        self.dispatch(|_worker| Ok(std::thread::current().name().unwrap_or("").to_owned()))
            .await
    }

    pub fn discard_error(&self) -> Option<String> {
        self.inner
            .discard_error
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// Close the connection and join the SQLite thread without blocking a worker.
    pub async fn close(&self) -> Result<(), SqliteError> {
        let _permit = self.inner.gate.clone().lock_owned().await;
        if self.inner.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let (reply, ready) = mpsc::sync_channel(1);
        self.inner
            .cmd
            .send(Job::Shutdown(reply))
            .map_err(|_| SqliteError::stopped())?;
        tokio::task::spawn_blocking(move || ready.recv())
            .await
            .map_err(|_| SqliteError::stopped())?
            .map_err(|_| SqliteError::stopped())?;
        let handle = self
            .inner
            .thread
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(handle) = handle {
            tokio::task::spawn_blocking(move || handle.join())
                .await
                .map_err(|_| SqliteError::stopped())?
                .map_err(|_| SqliteError::stopped())?;
        }
        Ok(())
    }

    async fn start(target: Target) -> Result<Self, SqliteError> {
        let (cmd, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("fexu-sqlite".to_owned())
            .spawn(move || match Worker::open(target) {
                Ok(worker) => {
                    let _ = ready_tx.send(Ok(()));
                    serve(receiver, worker);
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                }
            })
            .map_err(|error| {
                SqliteError::new(format!("Could not start the SQLite thread: {error}"))
            })?;
        let started = tokio::task::spawn_blocking(move || ready_rx.recv())
            .await
            .map_err(|_| SqliteError::stopped())?
            .map_err(|_| SqliteError::stopped())?;
        match started {
            Ok(()) => Ok(Self {
                inner: Arc::new(Inner {
                    cmd,
                    gate: Arc::new(tokio::sync::Mutex::new(())),
                    thread: std::sync::Mutex::new(Some(thread)),
                    closed: AtomicBool::new(false),
                    discard_error: std::sync::Mutex::new(None),
                }),
            }),
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }

    async fn dispatch<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Worker) -> Result<T, SqliteError> + Send + 'static,
    ) -> Result<T, SqliteError> {
        if ON_SQLITE_THREAD.with(|flag| flag.get()) {
            return Err(SqliteError::new(
                "SQLite thread cannot re-enter the session",
            ));
        }
        let _permit = self.inner.gate.clone().lock_owned().await;
        self.dispatch_locked(job).await
    }

    async fn dispatch_locked<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Worker) -> Result<T, SqliteError> + Send + 'static,
    ) -> Result<T, SqliteError> {
        if self.inner.closed.load(Ordering::SeqCst) {
            return Err(SqliteError::stopped());
        }
        let (reply, ready) = tokio::sync::oneshot::channel();
        self.inner
            .cmd
            .send(Job::Run(Box::new(move |worker| {
                let _ = reply.send(job(worker));
            })))
            .map_err(|_| SqliteError::stopped())?;
        ready.await.map_err(|_| SqliteError::stopped())?
    }
}

/// Transaction bound to the single connection. Drop rolls it back.
pub struct SqliteTransaction {
    session: SqliteSession,
    /// Held until commit, rollback, or drop so no other call uses the connection.
    #[allow(dead_code)]
    permit: tokio::sync::OwnedMutexGuard<()>,
    finished: bool,
}

impl SqliteTransaction {
    pub async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqliteError> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.session
            .dispatch_locked(move |worker| worker.execute(&sql, &params))
            .await
    }

    pub async fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<Vec<SqlValue>>, SqliteError> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.session
            .dispatch_locked(move |worker| worker.query(&sql, &params))
            .await
    }

    pub async fn commit(mut self) -> Result<(), SqliteError> {
        self.session
            .dispatch_locked(|worker| worker.commit())
            .await?;
        self.finished = true;
        Ok(())
    }

    pub async fn rollback(mut self) -> Result<(), SqliteError> {
        self.session
            .dispatch_locked(|worker| worker.rollback())
            .await?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for SqliteTransaction {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        // Queue the rollback and return. The permit is released after this
        // function, so the next caller lines up behind the rollback. Waiting
        // here would block whichever Tokio worker drops the transaction.
        let _ = self.session.inner.cmd.send(Job::Run(Box::new(|worker| {
            let _ = worker.rollback();
        })));
    }
}

/// One checked-out connection. Foreign-key changes stay on this connection.
pub struct PinnedConnection {
    session: SqliteSession,
    /// Held until the pin is dropped so pragma changes stay on this connection.
    #[allow(dead_code)]
    permit: tokio::sync::OwnedMutexGuard<()>,
    enforcement_intact: bool,
}

impl PinnedConnection {
    pub async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, SqliteError> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.session
            .dispatch_locked(move |worker| worker.execute(&sql, &params))
            .await
    }

    pub async fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<Vec<SqlValue>>, SqliteError> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.session
            .dispatch_locked(move |worker| worker.query(&sql, &params))
            .await
    }

    pub async fn suspend_foreign_keys(&mut self) -> Result<(), SqliteError> {
        self.session
            .dispatch_locked(|worker| worker.suspend_foreign_keys())
            .await?;
        self.enforcement_intact = false;
        Ok(())
    }

    pub async fn restore_foreign_keys(&mut self) -> Result<(), SqliteError> {
        self.session
            .dispatch_locked(|worker| worker.restore_foreign_keys())
            .await?;
        self.enforcement_intact = true;
        Ok(())
    }

    pub async fn begin(&self) -> Result<(), SqliteError> {
        self.session.dispatch_locked(|worker| worker.begin()).await
    }

    pub async fn commit(&self) -> Result<(), SqliteError> {
        self.session.dispatch_locked(|worker| worker.commit()).await
    }

    pub async fn rollback(&self) -> Result<(), SqliteError> {
        self.session
            .dispatch_locked(|worker| worker.rollback())
            .await
    }
}

impl Drop for PinnedConnection {
    fn drop(&mut self) {
        if self.enforcement_intact {
            return;
        }
        let inner = Arc::clone(&self.session.inner);
        // Reopen is queued before the permit is released, so the next call on
        // this session observes the fresh connection without waiting here.
        let _ = self
            .session
            .inner
            .cmd
            .send(Job::Run(Box::new(move |worker| {
                if let Err(error) = worker.replace() {
                    *inner
                        .discard_error
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) = Some(error.message);
                }
            })));
    }
}

/// PostgreSQL through SeaORM 1.1 and the published SQLx Postgres driver.
///
/// SQLite URLs are rejected here so this helper cannot request `sqlx-sqlite`.
pub async fn connect_postgres(url: &str) -> Result<sea_orm::DatabaseConnection, sea_orm::DbErr> {
    if url.starts_with("sqlite:") {
        return Err(sea_orm::DbErr::Conn(sea_orm::RuntimeErr::Internal(
            "connect_postgres only opens PostgreSQL".to_owned(),
        )));
    }
    sea_orm::Database::connect(url).await
}

fn parse_sqlite_url(url: &str) -> Result<Target, SqliteError> {
    if let Some(rest) = url.strip_prefix("sqlite::memory:") {
        if rest.is_empty() || rest.starts_with('?') {
            return Ok(Target::Memory);
        }
        return Err(SqliteError::new("SQLite memory URL is not valid"));
    }
    let Some(rest) = url.strip_prefix("sqlite://") else {
        return Err(SqliteError::new(
            "SQLite URL must start with sqlite:// or sqlite::memory:",
        ));
    };
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    if path.is_empty() {
        return Err(SqliteError::new("SQLite URL is missing a file path"));
    }
    if !query.is_empty() {
        let mode_ok = query
            .split('&')
            .all(|part| part.is_empty() || part == "mode=rwc" || part == "mode=rw");
        if !mode_ok {
            return Err(SqliteError::new("SQLite URL mode must be rw or rwc"));
        }
    }
    Ok(Target::File(PathBuf::from(path)))
}

#[cfg(test)]
mod tests {
    use super::{parse_sqlite_url, Target};
    use std::path::PathBuf;

    #[test]
    fn parses_memory_and_file_urls() {
        assert_eq!(parse_sqlite_url("sqlite::memory:").unwrap(), Target::Memory);
        assert_eq!(
            parse_sqlite_url("sqlite://db.sqlite3?mode=rwc").unwrap(),
            Target::File(PathBuf::from("db.sqlite3"))
        );
        assert_eq!(
            parse_sqlite_url("sqlite://C:/temp/app.sqlite3?mode=rw").unwrap(),
            Target::File(PathBuf::from("C:/temp/app.sqlite3"))
        );
        assert!(parse_sqlite_url("postgres://localhost/fexu").is_err());
        assert!(parse_sqlite_url("sqlite://db.sqlite3?mode=ro").is_err());
    }
}
