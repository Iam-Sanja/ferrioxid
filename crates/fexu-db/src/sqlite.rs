//! One SQLite connection on a dedicated thread.
//!
//! The thread owns rusqlite. Tokio tasks only send a job and wait on an oneshot,
//! so a query does not occupy a runtime worker. The worker never waits on the
//! session gate, which keeps shutdown, rollback and the next caller ordered
//! instead of deadlocked.
//!
//! A dropped transaction or pin queues its cleanup before releasing the gate.
//! The next caller therefore cannot see an open transaction or suspended
//! foreign keys. If rollback or reopen fails, the connection is dropped and the
//! session stays closed; the stored error is returned to the next caller.

use crate::error::{ConstraintKind, DbErr, RuntimeErr};
use rusqlite::params_from_iter;
use rusqlite::types::Value as SqlValue;
use sea_query::Value;
use std::cell::Cell;
use std::path::PathBuf;
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
pub(crate) enum CellValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Clone, Debug)]
pub(crate) struct SqliteRecord {
    pub columns: Vec<(String, CellValue)>,
}

struct Worker {
    connection: Option<rusqlite::Connection>,
    target: Target,
    in_transaction: bool,
    foreign_keys_on: bool,
    #[cfg(test)]
    fail_reopen: bool,
    #[cfg(test)]
    fail_rollback: bool,
}

#[derive(Clone, Debug)]
enum Target {
    Memory,
    File { path: PathBuf, read_only: bool },
}

enum Job {
    Run(Box<dyn FnOnce(&mut Worker) + Send>),
    Shutdown(SyncSender<Result<(), DbErr>>),
}

struct Inner {
    cmd: mpsc::Sender<Job>,
    gate: Arc<tokio::sync::Mutex<()>>,
    thread: std::sync::Mutex<Option<JoinHandle<()>>>,
    closed: AtomicBool,
    fatal: std::sync::Mutex<Option<String>>,
}

#[derive(Clone)]
pub(crate) struct SqliteSession {
    inner: Arc<Inner>,
}

pub(crate) struct SqliteTxn {
    session: SqliteSession,
    #[allow(dead_code)]
    permit: tokio::sync::OwnedMutexGuard<()>,
    finished: bool,
}

pub(crate) struct SqlitePin {
    session: SqliteSession,
    #[allow(dead_code)]
    permit: tokio::sync::OwnedMutexGuard<()>,
}

impl SqliteSession {
    pub(crate) async fn open_url(url: &str) -> Result<Self, DbErr> {
        Self::start(parse_sqlite_url(url)?).await
    }

    pub(crate) async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, DbErr> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.dispatch(move |worker| worker.execute(&sql, &params))
            .await
    }

    pub(crate) async fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<SqliteRecord>, DbErr> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.dispatch(move |worker| worker.query(&sql, &params))
            .await
    }

    pub(crate) async fn execute_batch(&self, sql: &str) -> Result<u64, DbErr> {
        let sql = sql.to_owned();
        self.dispatch(move |worker| worker.execute_batch(&sql))
            .await
    }

    pub(crate) async fn transaction(&self) -> Result<SqliteTxn, DbErr> {
        let permit = self.inner.gate.clone().lock_owned().await;
        self.dispatch_locked(|worker| worker.begin()).await?;
        Ok(SqliteTxn {
            session: self.clone(),
            permit,
            finished: false,
        })
    }

    pub(crate) async fn pin(&self) -> Result<SqlitePin, DbErr> {
        let permit = self.inner.gate.clone().lock_owned().await;
        self.dispatch_locked(|worker| worker.ensure().map(|_| ()))
            .await?;
        Ok(SqlitePin {
            session: self.clone(),
            permit,
        })
    }

    pub(crate) async fn close(&self) -> Result<(), DbErr> {
        let _permit = self.inner.gate.clone().lock_owned().await;
        if !self.inner.closed.swap(true, Ordering::SeqCst) {
            let (reply, ready) = mpsc::sync_channel(1);
            self.inner
                .cmd
                .send(Job::Shutdown(reply))
                .map_err(|_| stopped())?;
            let shutdown = tokio::task::spawn_blocking(move || ready.recv())
                .await
                .map_err(|_| stopped())?
                .map_err(|_| stopped())?;
            let handle = self
                .inner
                .thread
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take();
            if let Some(handle) = handle {
                tokio::task::spawn_blocking(move || handle.join())
                    .await
                    .map_err(|_| stopped())?
                    .map_err(|_| stopped())?;
            }
            shutdown?;
        }
        self.fatal_error()
    }

    #[cfg(test)]
    pub(crate) async fn fail_next_reopen(&self) {
        let _permit = self.inner.gate.clone().lock_owned().await;
        let _ = self
            .dispatch_locked(|worker| {
                worker.fail_reopen = true;
                Ok(())
            })
            .await;
    }

    #[cfg(test)]
    pub(crate) async fn fail_next_rollback(&self) {
        let _permit = self.inner.gate.clone().lock_owned().await;
        let _ = self
            .dispatch_locked(|worker| {
                worker.fail_rollback = true;
                Ok(())
            })
            .await;
    }

    async fn connection_open(&self) -> bool {
        self.dispatch_locked(|worker| Ok(worker.connection.is_some()))
            .await
            .unwrap_or(false)
    }

    fn fatal_error(&self) -> Result<(), DbErr> {
        match self
            .inner
            .fatal
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            Some(message) => Err(exec_err(message)),
            None => Ok(()),
        }
    }

    fn store_fatal(&self, error: &DbErr) {
        *self
            .inner
            .fatal
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(error.to_string());
    }

    async fn dispatch<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Worker) -> Result<T, DbErr> + Send + 'static,
    ) -> Result<T, DbErr> {
        if ON_SQLITE_THREAD.with(|flag| flag.get()) {
            return Err(exec_err("SQLite thread cannot re-enter the session"));
        }
        let _permit = self.inner.gate.clone().lock_owned().await;
        self.dispatch_locked(job).await
    }

    async fn dispatch_locked<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Worker) -> Result<T, DbErr> + Send + 'static,
    ) -> Result<T, DbErr> {
        if self.inner.closed.load(Ordering::SeqCst) {
            return Err(stopped());
        }
        self.fatal_error()?;
        let (reply, ready) = tokio::sync::oneshot::channel();
        self.inner
            .cmd
            .send(Job::Run(Box::new(move |worker| {
                let _ = reply.send(job(worker));
            })))
            .map_err(|_| stopped())?;
        ready.await.map_err(|_| stopped())?
    }

    async fn start(target: Target) -> Result<Self, DbErr> {
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
            .map_err(|error| exec_err(format!("Could not start the SQLite thread: {error}")))?;
        let started = tokio::task::spawn_blocking(move || ready_rx.recv())
            .await
            .map_err(|_| stopped())?
            .map_err(|_| stopped())?;
        match started {
            Ok(()) => Ok(Self {
                inner: Arc::new(Inner {
                    cmd,
                    gate: Arc::new(tokio::sync::Mutex::new(())),
                    thread: std::sync::Mutex::new(Some(thread)),
                    closed: AtomicBool::new(false),
                    fatal: std::sync::Mutex::new(None),
                }),
            }),
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }
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
        // Wait here. A detached close checkpoints the WAL after the caller
        // has already read the file, so a read-only open would look like a write.
        if sent {
            let _ = ready.recv_timeout(Duration::from_secs(5));
        }
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

impl SqliteTxn {
    pub(crate) async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, DbErr> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.session
            .dispatch_locked(move |worker| worker.execute(&sql, &params))
            .await
    }

    pub(crate) async fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<SqliteRecord>, DbErr> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.session
            .dispatch_locked(move |worker| worker.query(&sql, &params))
            .await
    }

    pub(crate) async fn commit(mut self) -> Result<(), DbErr> {
        self.session
            .dispatch_locked(|worker| worker.commit())
            .await?;
        self.finished = true;
        Ok(())
    }

    pub(crate) async fn rollback(mut self) -> Result<(), DbErr> {
        let result = self
            .session
            .dispatch_locked(|worker| worker.rollback())
            .await;
        self.finished = true;
        if result.is_err() && !self.session.connection_open().await {
            if let Err(error) = &result {
                self.session.store_fatal(error);
            }
        }
        result
    }

    pub(crate) async fn execute_batch(&self, sql: &str) -> Result<u64, DbErr> {
        let sql = sql.to_owned();
        self.session
            .dispatch_locked(move |worker| worker.execute_batch(&sql))
            .await
    }
}

impl Drop for SqliteTxn {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let session = self.session.clone();
        let _ = self
            .session
            .inner
            .cmd
            .send(Job::Run(Box::new(move |worker| {
                if let Err(error) = worker.rollback() {
                    if worker.connection.is_none() {
                        session.store_fatal(&error);
                    }
                }
            })));
    }
}

impl SqlitePin {
    pub(crate) async fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<u64, DbErr> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.session
            .dispatch_locked(move |worker| worker.execute(&sql, &params))
            .await
    }

    pub(crate) async fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<SqliteRecord>, DbErr> {
        let sql = sql.to_owned();
        let params = params.to_vec();
        self.session
            .dispatch_locked(move |worker| worker.query(&sql, &params))
            .await
    }

    pub(crate) async fn execute_batch(&self, sql: &str) -> Result<u64, DbErr> {
        let sql = sql.to_owned();
        self.session
            .dispatch_locked(move |worker| worker.execute_batch(&sql))
            .await
    }
}

impl Drop for SqlitePin {
    fn drop(&mut self) {
        let session = self.session.clone();
        let _ = self
            .session
            .inner
            .cmd
            .send(Job::Run(Box::new(move |worker| {
                let _ = worker.rollback();
                if worker.connection.is_none() || !worker.foreign_keys_on {
                    if let Err(error) = worker.reopen() {
                        session.store_fatal(&error);
                    }
                }
            })));
    }
}

impl Worker {
    fn open(target: Target) -> Result<Self, DbErr> {
        let connection = open_connection(&target)?;
        Ok(Self {
            connection: Some(connection),
            target,
            in_transaction: false,
            foreign_keys_on: true,
            #[cfg(test)]
            fail_reopen: false,
            #[cfg(test)]
            fail_rollback: false,
        })
    }

    fn ensure(&self) -> Result<&rusqlite::Connection, DbErr> {
        self.connection.as_ref().ok_or_else(stopped)
    }

    fn ensure_mut(&mut self) -> Result<&mut rusqlite::Connection, DbErr> {
        self.connection.as_mut().ok_or_else(stopped)
    }

    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<u64, DbErr> {
        let changed = match self
            .ensure_mut()?
            .execute(sql, params_from_iter(params.iter()))
        {
            Ok(changed) => changed,
            // INSERT ... RETURNING and some PRAGMA statements produce a row.
            // Callers still use execute for the side effect.
            Err(rusqlite::Error::ExecuteReturnedResults) => self.consume_rows(sql, params)?,
            Err(error) => return Err(rusqlite_err(error, true)),
        };
        self.observe(sql)?;
        Ok(changed_rows(sql, changed))
    }

    fn consume_rows(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, DbErr> {
        let connection = self.ensure_mut()?;
        let changed = {
            let mut statement = connection
                .prepare(sql)
                .map_err(|error| rusqlite_err(error, true))?;
            let mut rows = statement
                .query(params_from_iter(params.iter()))
                .map_err(|error| rusqlite_err(error, true))?;
            while rows
                .next()
                .map_err(|error| rusqlite_err(error, true))?
                .is_some()
            {}
            drop(rows);
            drop(statement);
            connection.changes()
        };
        Ok(changed as usize)
    }

    fn query(&mut self, sql: &str, params: &[SqlValue]) -> Result<Vec<SqliteRecord>, DbErr> {
        let connection = self.ensure_mut()?;
        let mut statement = connection
            .prepare(sql)
            .map_err(|error| rusqlite_err(error, false))?;
        let names = statement
            .column_names()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut rows = statement
            .query(params_from_iter(params.iter()))
            .map_err(|error| rusqlite_err(error, false))?;
        let mut records = Vec::new();
        while let Some(row) = rows.next().map_err(|error| rusqlite_err(error, false))? {
            let mut columns = Vec::with_capacity(names.len());
            for (index, name) in names.iter().enumerate() {
                columns.push((name.clone(), read_cell(row, index)?));
            }
            records.push(SqliteRecord { columns });
        }
        Ok(records)
    }

    fn execute_batch(&mut self, sql: &str) -> Result<u64, DbErr> {
        let connection = self.ensure_mut()?;
        connection
            .execute_batch(sql)
            .map_err(|error| rusqlite_err(error, true))?;
        let changed = connection.changes();
        self.observe(sql)?;
        Ok(changed)
    }

    fn begin(&mut self) -> Result<(), DbErr> {
        if self.in_transaction {
            return Err(exec_err("SQLite transaction is already open"));
        }
        self.execute("BEGIN", &[])?;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), DbErr> {
        if !self.in_transaction {
            return Err(exec_err("SQLite transaction is not open"));
        }
        self.execute("COMMIT", &[])?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), DbErr> {
        if !self.in_transaction {
            return Ok(());
        }
        #[cfg(test)]
        if self.fail_rollback {
            self.fail_rollback = false;
            self.connection = None;
            self.in_transaction = false;
            self.foreign_keys_on = false;
            return self.recover(exec_err("sqlite rollback failed"));
        }
        match self.ensure_mut()?.execute_batch("ROLLBACK") {
            Ok(()) => {
                self.in_transaction = false;
                Ok(())
            }
            Err(error) => {
                // Dropping the handle aborts the open transaction inside SQLite.
                self.connection = None;
                self.in_transaction = false;
                self.foreign_keys_on = false;
                self.recover(rusqlite_err(error, true))
            }
        }
    }

    fn recover(&mut self, error: DbErr) -> Result<(), DbErr> {
        match self.reopen() {
            Ok(()) => Err(error),
            Err(reopen_error) => Err(reopen_error),
        }
    }

    fn reopen(&mut self) -> Result<(), DbErr> {
        #[cfg(test)]
        if self.fail_reopen {
            self.fail_reopen = false;
            self.connection = None;
            self.in_transaction = false;
            self.foreign_keys_on = false;
            return Err(exec_err("sqlite reopen failed"));
        }
        match open_connection(&self.target) {
            Ok(connection) => {
                self.connection = Some(connection);
                self.in_transaction = false;
                self.foreign_keys_on = true;
                Ok(())
            }
            Err(error) => {
                self.connection = None;
                self.in_transaction = false;
                self.foreign_keys_on = false;
                Err(error)
            }
        }
    }

    fn observe(&mut self, sql: &str) -> Result<(), DbErr> {
        let keyword = sql_keyword(sql);
        if keyword.eq_ignore_ascii_case("begin") {
            self.in_transaction = true;
        } else if keyword.eq_ignore_ascii_case("commit")
            || keyword.eq_ignore_ascii_case("end")
            || keyword.eq_ignore_ascii_case("rollback")
        {
            self.in_transaction = false;
        }
        match foreign_key_assignment(sql) {
            Some(false) => self.foreign_keys_on = false,
            Some(true) => {
                let enabled: i64 = self
                    .ensure()?
                    .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                    .map_err(|error| rusqlite_err(error, false))?;
                if enabled != 1 {
                    self.foreign_keys_on = false;
                    return Err(exec_err(
                        "Foreign key enforcement stayed off on the SQLite connection",
                    ));
                }
                self.foreign_keys_on = true;
            }
            None => {}
        }
        Ok(())
    }
}

fn serve(receiver: Receiver<Job>, mut worker: Worker) {
    ON_SQLITE_THREAD.with(|flag| flag.set(true));
    while let Ok(job) = receiver.recv() {
        match job {
            Job::Run(job) => job(&mut worker),
            Job::Shutdown(reply) => {
                let result = if worker.in_transaction {
                    worker.rollback()
                } else {
                    Ok(())
                };
                worker.connection = None;
                let _ = reply.send(result);
                break;
            }
        }
    }
}

fn open_connection(target: &Target) -> Result<rusqlite::Connection, DbErr> {
    let connection = match target {
        Target::Memory => rusqlite::Connection::open_in_memory(),
        Target::File {
            path,
            read_only: true,
        } => {
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        }
        Target::File {
            path,
            read_only: false,
        } => rusqlite::Connection::open(path),
    }
    .map_err(|error| rusqlite_err(error, true))?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|error| rusqlite_err(error, true))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .map_err(|error| rusqlite_err(error, true))?;
    if matches!(
        target,
        Target::File {
            read_only: false,
            ..
        }
    ) {
        let mode: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(|error| rusqlite_err(error, true))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(exec_err(format!(
                "SQLite did not enable WAL; journal_mode={mode}"
            )));
        }
    }
    Ok(connection)
}

fn read_cell(row: &rusqlite::Row<'_>, index: usize) -> Result<CellValue, DbErr> {
    let value = row
        .get_ref(index)
        .map_err(|error| rusqlite_err(error, false))?;
    Ok(match value {
        rusqlite::types::ValueRef::Null => CellValue::Null,
        rusqlite::types::ValueRef::Integer(value) => CellValue::Integer(value),
        rusqlite::types::ValueRef::Real(value) => CellValue::Real(value),
        rusqlite::types::ValueRef::Text(value) => {
            CellValue::Text(String::from_utf8_lossy(value).into_owned())
        }
        rusqlite::types::ValueRef::Blob(value) => CellValue::Blob(value.to_vec()),
    })
}

fn changed_rows(sql: &str, changed: usize) -> u64 {
    let keyword = sql_keyword(sql);
    if keyword.eq_ignore_ascii_case("insert")
        || keyword.eq_ignore_ascii_case("update")
        || keyword.eq_ignore_ascii_case("delete")
    {
        changed as u64
    } else {
        0
    }
}

fn sql_keyword(sql: &str) -> &str {
    let trimmed = sql.trim_start();
    let word = trimmed.split_whitespace().next().unwrap_or("");
    word.trim_end_matches(';')
}

fn foreign_key_assignment(sql: &str) -> Option<bool> {
    let compact = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = compact.to_ascii_lowercase();
    let rest = lower.split("pragma foreign_keys").nth(1)?;
    let rest = rest.trim().trim_end_matches(';').trim();
    let value = rest.strip_prefix('=')?.trim();
    match value {
        "off" | "0" | "false" => Some(false),
        "on" | "1" | "true" => Some(true),
        _ => None,
    }
}

fn parse_sqlite_url(url: &str) -> Result<Target, DbErr> {
    if let Some(rest) = url.strip_prefix("sqlite::memory:") {
        if rest.is_empty() || rest.starts_with('?') {
            return Ok(Target::Memory);
        }
        return Err(exec_err("SQLite memory URL is not valid"));
    }
    let Some(rest) = url.strip_prefix("sqlite://") else {
        return Err(exec_err(
            "SQLite URL must start with sqlite:// or sqlite::memory:",
        ));
    };
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    if path.is_empty() {
        return Err(exec_err("SQLite URL is missing a file path"));
    }
    let mut read_only = false;
    if !query.is_empty() {
        for part in query.split('&').filter(|part| !part.is_empty()) {
            match part {
                "mode=rw" | "mode=rwc" => {}
                "mode=ro" => read_only = true,
                _ => return Err(exec_err("SQLite URL mode must be rw, rwc, or ro")),
            }
        }
    }
    if path == ":memory:" {
        return Ok(Target::Memory);
    }
    Ok(Target::File {
        path: PathBuf::from(path),
        read_only,
    })
}

fn exec_err(message: impl Into<String>) -> DbErr {
    DbErr::Exec(RuntimeErr::internal(message))
}

fn stopped() -> DbErr {
    exec_err("SQLite connection is closed")
}

fn rusqlite_err(error: rusqlite::Error, execute: bool) -> DbErr {
    let (message, constraint) = match &error {
        rusqlite::Error::SqliteFailure(ffi, detail) => {
            let text = detail.clone().unwrap_or_else(|| error.to_string());
            let constraint = match ffi.extended_code {
                2067 | 1555 => Some(ConstraintKind::Unique),
                787 => Some(ConstraintKind::ForeignKey),
                1811 if text.contains("FOREIGN KEY constraint failed") => {
                    Some(ConstraintKind::ForeignKey)
                }
                _ => None,
            };
            (format!("sqlite:{}:{text}", ffi.extended_code), constraint)
        }
        other => (other.to_string(), None),
    };
    let runtime = RuntimeErr::with_constraint(message, constraint);
    if execute {
        DbErr::Exec(runtime)
    } else {
        DbErr::Query(runtime)
    }
}

pub(crate) fn bind_value(value: &Value) -> Result<SqlValue, DbErr> {
    use chrono::SecondsFormat;
    Ok(match value {
        Value::Bool(None)
        | Value::TinyInt(None)
        | Value::SmallInt(None)
        | Value::Int(None)
        | Value::BigInt(None)
        | Value::TinyUnsigned(None)
        | Value::SmallUnsigned(None)
        | Value::Unsigned(None)
        | Value::BigUnsigned(None)
        | Value::Float(None)
        | Value::Double(None)
        | Value::String(None)
        | Value::Char(None)
        | Value::Bytes(None)
        | Value::Json(None)
        | Value::ChronoDate(None)
        | Value::ChronoTime(None)
        | Value::ChronoDateTime(None)
        | Value::ChronoDateTimeUtc(None)
        | Value::ChronoDateTimeLocal(None)
        | Value::ChronoDateTimeWithTimeZone(None)
        | Value::Uuid(None) => SqlValue::Null,
        Value::Bool(Some(value)) => SqlValue::Integer(i64::from(*value)),
        Value::TinyInt(Some(value)) => SqlValue::Integer(i64::from(*value)),
        Value::SmallInt(Some(value)) => SqlValue::Integer(i64::from(*value)),
        Value::Int(Some(value)) => SqlValue::Integer(i64::from(*value)),
        Value::BigInt(Some(value)) => SqlValue::Integer(*value),
        Value::TinyUnsigned(Some(value)) => SqlValue::Integer(i64::from(*value)),
        Value::SmallUnsigned(Some(value)) => SqlValue::Integer(i64::from(*value)),
        Value::Unsigned(Some(value)) => SqlValue::Integer(i64::from(*value)),
        Value::BigUnsigned(Some(value)) => SqlValue::Integer(
            i64::try_from(*value)
                .map_err(|_| exec_err("SQLite cannot bind an unsigned integer above i64::MAX"))?,
        ),
        Value::Float(Some(value)) => SqlValue::Real(f64::from(*value)),
        Value::Double(Some(value)) => SqlValue::Real(*value),
        Value::String(Some(value)) => SqlValue::Text(value.to_string()),
        Value::Char(Some(value)) => SqlValue::Text(value.to_string()),
        Value::Bytes(Some(value)) => SqlValue::Blob(value.to_vec()),
        Value::Json(Some(value)) => SqlValue::Text(value.to_string()),
        Value::ChronoDate(Some(value)) => SqlValue::Text(value.format("%F").to_string()),
        Value::ChronoTime(Some(value)) => SqlValue::Text(value.format("%T%.f").to_string()),
        Value::ChronoDateTime(Some(value)) => SqlValue::Text(value.format("%F %T%.f").to_string()),
        Value::ChronoDateTimeUtc(Some(value)) => {
            SqlValue::Text(value.to_rfc3339_opts(SecondsFormat::AutoSi, false))
        }
        Value::ChronoDateTimeLocal(Some(value)) => {
            SqlValue::Text(value.to_rfc3339_opts(SecondsFormat::AutoSi, false))
        }
        Value::ChronoDateTimeWithTimeZone(Some(value)) => {
            SqlValue::Text(value.to_rfc3339_opts(SecondsFormat::AutoSi, false))
        }
        Value::Uuid(Some(value)) => SqlValue::Blob(value.as_bytes().to_vec()),
        Value::Decimal(_) => {
            return Err(exec_err(
                "SQLite stores Fexu decimals as 16-byte blobs, not NUMERIC",
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::SqliteSession;

    fn file_url() -> (std::path::PathBuf, String) {
        let directory =
            std::env::temp_dir().join(format!("fexu-sqlite-session-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("session.sqlite3");
        let url = format!(
            "sqlite://{}?mode=rwc",
            file.to_string_lossy().replace('\\', "/")
        );
        (directory, url)
    }

    #[tokio::test]
    async fn failed_rollback_reopens_the_same_session() {
        let (directory, url) = file_url();
        let session = SqliteSession::open_url(&url).await.unwrap();
        session.fail_next_rollback().await;
        let transaction = session.transaction().await.unwrap();
        transaction
            .execute("CREATE TABLE kept (id INTEGER PRIMARY KEY)", &[])
            .await
            .unwrap();
        let error = transaction.rollback().await.unwrap_err();
        assert!(error.to_string().contains("rollback failed"));
        session
            .execute("CREATE TABLE recovered (id INTEGER PRIMARY KEY)", &[])
            .await
            .unwrap();
        session.close().await.unwrap();
        let again = SqliteSession::open_url(&url).await.unwrap();
        let rows = again
            .query(
                "SELECT name FROM sqlite_master WHERE name = 'recovered'",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        again.close().await.unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn failed_reopen_keeps_the_session_closed() {
        let (directory, url) = file_url();
        let session = SqliteSession::open_url(&url).await.unwrap();
        session.fail_next_rollback().await;
        session.fail_next_reopen().await;
        let transaction = session.transaction().await.unwrap();
        let error = transaction.rollback().await.unwrap_err();
        assert!(error.to_string().contains("reopen failed"));
        let next = session.execute("SELECT 1", &[]).await.unwrap_err();
        assert!(next.to_string().contains("reopen failed"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn close_rejects_later_statements() {
        let (directory, url) = file_url();
        let session = SqliteSession::open_url(&url).await.unwrap();
        session.close().await.unwrap();
        let error = session.execute("SELECT 1", &[]).await.unwrap_err();
        assert!(error.to_string().contains("stopped") || error.to_string().contains("closed"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
