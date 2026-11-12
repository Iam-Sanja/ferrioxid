//! Persistente Queue. Änderungen laufen über bedingte Updates und gebundene Werte.

use crate::clock::Clock;
use crate::error::JobError;
use crate::schema::{table_ready, JOBS};
use sea_orm::sea_query::{
    Alias, Expr, ExprTrait, Order, PostgresQueryBuilder, Query, SqliteQueryBuilder,
};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, QueryResult, Statement, TransactionTrait,
    Value,
};
use serde::Serialize;
use serde_json::Value as Json;
use std::sync::Arc;
use uuid::Uuid;

pub const MAX_PAYLOAD_BYTES: usize = 16 * 1024;
pub const MAX_ATTEMPTS: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Succeeded,
    Retry,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Retry => "retry",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self, JobError> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "retry" => Ok(Self::Retry),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(JobError::permanent("job.payload.invalid")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct JobRecord {
    pub id: Uuid,
    pub handler: String,
    pub payload_version: u32,
    pub payload: String,
    pub status: JobStatus,
    pub created_at: i64,
    pub available_at: i64,
    pub attempts: u32,
    pub max_attempts: u32,
    pub next_attempt_at: Option<i64>,
    pub lease_owner: Option<String>,
    pub lease_token: Option<String>,
    pub lease_expires_at: Option<i64>,
    pub error_code: Option<String>,
    pub finished_at: Option<i64>,
    pub idempotency_key: Option<String>,
    pub interval_secs: Option<i64>,
    pub schedule_key: Option<String>,
    pub cron_expression: Option<String>,
    pub cron_zone: Option<String>,
}

#[derive(Clone, Debug)]
pub struct JobDiagnosis {
    pub id: Uuid,
    pub handler: String,
    pub status: JobStatus,
    pub attempts: u32,
    pub max_attempts: u32,
    pub error_code: Option<String>,
    pub available_at: i64,
    pub next_attempt_at: Option<i64>,
    pub finished_at: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct JobSummary {
    pub id: Uuid,
    pub handler: String,
    pub status: JobStatus,
    pub attempts: u32,
    pub max_attempts: u32,
    pub available_at: i64,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug)]
pub struct When {
    pub available_at: Option<i64>,
    pub max_attempts: Option<u32>,
    pub idempotency_key: Option<String>,
    pub interval_secs: Option<i64>,
    pub schedule_key: Option<String>,
    pub cron_expression: Option<String>,
    pub cron_zone: Option<String>,
}

impl When {
    pub fn now() -> Self {
        Self {
            available_at: None,
            max_attempts: None,
            idempotency_key: None,
            interval_secs: None,
            schedule_key: None,
            cron_expression: None,
            cron_zone: None,
        }
    }

    pub fn at(unix: i64) -> Self {
        Self {
            available_at: Some(unix),
            ..Self::now()
        }
    }

    pub fn every(schedule_key: impl Into<String>, interval_secs: i64) -> Self {
        Self {
            interval_secs: Some(interval_secs),
            schedule_key: Some(schedule_key.into()),
            ..Self::now()
        }
    }

    pub fn cron(
        schedule_key: impl Into<String>,
        expression: &str,
        zone: &str,
    ) -> Result<Self, JobError> {
        let _parsed = crate::schedule::parse_cron(expression)?;
        let zone = crate::schedule::parse_zone(zone)?;
        Ok(Self {
            schedule_key: Some(schedule_key.into()),
            cron_expression: Some(expression.trim().to_owned()),
            cron_zone: Some(zone.canonical()),
            ..Self::now()
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
    Created(Uuid),
    Existing(Uuid),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Stored,
    LostLease,
}

#[derive(Clone, Debug)]
pub struct JobPolicy {
    pub max_attempts: u32,
    pub retention_secs: i64,
    pub jitter: bool,
    pub lease_secs: i64,
}

impl Default for JobPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            retention_secs: 7 * 24 * 60 * 60,
            jitter: false,
            lease_secs: 60,
        }
    }
}

#[derive(Clone)]
pub struct Queue {
    db: DatabaseConnection,
    clock: Arc<dyn Clock>,
    policy: JobPolicy,
}

impl Queue {
    pub fn new(db: DatabaseConnection, clock: Arc<dyn Clock>, policy: JobPolicy) -> Self {
        Self { db, clock, policy }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    pub fn policy(&self) -> &JobPolicy {
        &self.policy
    }

    pub async fn ready(&self) -> Result<(), JobError> {
        if table_ready(&self.db).await? {
            Ok(())
        } else {
            Err(JobError::message(
                "job.migration.missing",
                "Die Job-Migrationen 0001_jobs und 0002_job_schedule sind nicht vollständig angewendet. ferrioxid migrate up legt sie an. Der Worker migriert nicht selbst.",
            ))
        }
    }

    pub async fn enqueue_json(
        &self,
        handler: &str,
        version: u32,
        payload: &impl Serialize,
        when: When,
    ) -> Result<EnqueueOutcome, JobError> {
        self.ready().await?;
        validate_handler_name(handler)?;
        if version == 0 {
            return Err(JobError::permanent("job.version"));
        }
        let encoded =
            serde_json::to_vec(payload).map_err(|_| JobError::permanent("job.payload.invalid"))?;
        if encoded.len() > MAX_PAYLOAD_BYTES {
            return Err(JobError::permanent("job.payload.large"));
        }
        let json: Json = serde_json::from_slice(&encoded)
            .map_err(|_| JobError::permanent("job.payload.invalid"))?;
        if !json.is_object() {
            return Err(JobError::permanent("job.payload.invalid"));
        }
        reject_sensitive(&json)?;
        let max_attempts = when.max_attempts.unwrap_or(self.policy.max_attempts);
        if !(1..=MAX_ATTEMPTS).contains(&max_attempts) {
            return Err(JobError::message(
                "job.attempts",
                "max_attempts muss zwischen 1 und 20 liegen.",
            ));
        }
        let interval = normalize_interval(when.interval_secs, when.schedule_key.as_deref())?;
        let schedule_key = when.schedule_key.as_deref().map(validate_key).transpose()?;
        let cron = normalize_cron(
            when.cron_expression.as_deref(),
            when.cron_zone.as_deref(),
            schedule_key.as_deref(),
            interval.is_some(),
        )?;
        if interval.is_some() && schedule_key.is_none() {
            return Err(JobError::message(
                "job.schedule",
                "Ein Intervall braucht einen schedule_key.",
            ));
        }
        let now = self.clock.unix()?;
        let available_at = match &cron {
            Some((expression, zone)) if when.available_at.is_none() => {
                crate::schedule::next_after(expression, zone, now)?
            }
            _ => when.available_at.unwrap_or(now),
        };
        let idempotency = match when.idempotency_key.as_deref() {
            Some(key) => Some(validate_key(key)?),
            None => schedule_key
                .as_ref()
                .map(|key| format!("recur:{key}:{available_at}")),
        };
        let idempotency = idempotency.as_deref().map(validate_key).transpose()?;
        let transaction = self.db.begin().await.map_err(database)?;
        if let Some(key) = &idempotency {
            if let Some(existing) = find_by_column(&transaction, "idempotency_key", key).await? {
                return Ok(EnqueueOutcome::Existing(existing.id));
            }
        }
        if let Some(key) = &schedule_key {
            if let Some(existing) = find_by_column(&transaction, "schedule_key", key).await? {
                return Ok(EnqueueOutcome::Existing(existing.id));
            }
        }
        let id = Uuid::new_v4();
        let payload =
            String::from_utf8(encoded).map_err(|_| JobError::permanent("job.payload.invalid"))?;
        let inserted = insert_job(
            &transaction,
            &NewJob {
                id,
                handler,
                version,
                payload: &payload,
                available_at,
                now,
                max_attempts,
                idempotency: idempotency.as_deref(),
                interval,
                schedule_key: schedule_key.as_deref(),
                cron_expression: cron.as_ref().map(|(expression, _)| expression.as_str()),
                cron_zone: cron.as_ref().map(|(_, zone)| zone.as_str()),
            },
        )
        .await;
        if let Err(error) = inserted {
            drop(transaction);
            if error.code() == "job.conflict" {
                if let Some(key) = idempotency.as_deref() {
                    if let Some(existing) = find_by_column(&self.db, "idempotency_key", key).await?
                    {
                        return Ok(EnqueueOutcome::Existing(existing.id));
                    }
                }
                if let Some(key) = schedule_key.as_deref() {
                    if let Some(existing) = find_by_column(&self.db, "schedule_key", key).await? {
                        return Ok(EnqueueOutcome::Existing(existing.id));
                    }
                }
            }
            return Err(error);
        }
        transaction.commit().await.map_err(database)?;
        Ok(EnqueueOutcome::Created(id))
    }

    pub async fn get(&self, id: Uuid) -> Result<JobRecord, JobError> {
        self.ready().await?;
        load(&self.db, id)
            .await?
            .ok_or_else(|| JobError::message("job.not_found", "Der Job ist nicht vorhanden."))
    }

    pub async fn list(&self, limit: u32) -> Result<Vec<JobSummary>, JobError> {
        self.ready().await?;
        let limit = limit.clamp(1, 100);
        let mut query = Query::select();
        query
            .columns([
                Alias::new("id"),
                Alias::new("handler"),
                Alias::new("status"),
                Alias::new("attempts"),
                Alias::new("max_attempts"),
                Alias::new("available_at"),
                Alias::new("error_code"),
            ])
            .from(Alias::new(JOBS))
            .order_by(Alias::new("created_at"), Order::Desc)
            .order_by(Alias::new("id"), Order::Asc)
            .limit(u64::from(limit));
        let mut rows = Vec::new();
        for row in all(&self.db, &query).await? {
            let record = summary_from_row(&row)?;
            rows.push(record);
        }
        Ok(rows)
    }

    pub async fn claim(
        &self,
        worker_id: &str,
        handlers: &[&str],
        lease_secs: i64,
    ) -> Result<Option<(JobRecord, String)>, JobError> {
        self.ready().await?;
        if handlers.is_empty() {
            return Ok(None);
        }
        for name in handlers {
            validate_handler_name(name)?;
        }
        let now = self.clock.unix()?;
        let mut query = Query::select();
        query
            .column(Alias::new("id"))
            .from(Alias::new(JOBS))
            .and_where(Expr::col(Alias::new("handler")).is_in(handlers.iter().copied()))
            .and_where(Expr::expr(due(now)).or(expired(now)))
            .order_by(Alias::new("available_at"), Order::Asc)
            .order_by(Alias::new("id"), Order::Asc)
            .limit(8);
        let candidates = all(&self.db, &query).await?;
        for row in candidates {
            let id = required_uuid(&row, "id")?;
            let token = Uuid::new_v4().to_string();
            let expires = now.saturating_add(lease_secs.max(1));
            let mut update = Query::update();
            update
                .table(Alias::new(JOBS))
                .values([
                    (Alias::new("status"), text(JobStatus::Running.as_str())),
                    (Alias::new("lease_owner"), text(worker_id)),
                    (Alias::new("lease_token"), text(&token)),
                    (Alias::new("lease_expires_at"), number(expires)),
                    (
                        Alias::new("attempts"),
                        Expr::col(Alias::new("attempts")).add(1),
                    ),
                ])
                .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
                .and_where(Expr::expr(due(now)).or(expired(now)));
            if execute_rows(&self.db, &update).await? == 1 {
                let record = load(&self.db, id).await?.ok_or_else(|| {
                    JobError::message("job.database", "Der beanspruchte Job ist nicht lesbar.")
                })?;
                if record.lease_token.as_deref() == Some(token.as_str()) {
                    return Ok(Some((record, token)));
                }
            }
        }
        Ok(None)
    }

    pub async fn finish_success(
        &self,
        id: Uuid,
        token: &str,
        record: &JobRecord,
    ) -> Result<Finish, JobError> {
        let now = self.clock.unix()?;
        let transaction = self.db.begin().await.map_err(database)?;
        if conditional_status(
            &transaction,
            FinishUpdate {
                id,
                token,
                now,
                status: JobStatus::Succeeded,
                error_code: None,
                finished_at: Some(now),
                next_attempt_at: None,
                clear_schedule: true,
            },
        )
        .await?
            != Finish::Stored
        {
            return Ok(Finish::LostLease);
        }
        if let (Some(interval), Some(schedule_key)) =
            (record.interval_secs, record.schedule_key.clone())
        {
            let next_at = next_slot(record.available_at, interval, now);
            let idempotency = format!("recur:{schedule_key}:{next_at}");
            if find_by_column(&transaction, "idempotency_key", &idempotency)
                .await?
                .is_none()
            {
                insert_job(
                    &transaction,
                    &NewJob {
                        id: Uuid::new_v4(),
                        handler: &record.handler,
                        version: record.payload_version,
                        payload: &record.payload,
                        available_at: next_at,
                        now,
                        max_attempts: record.max_attempts,
                        idempotency: Some(&idempotency),
                        interval: Some(interval),
                        schedule_key: Some(&schedule_key),
                        cron_expression: None,
                        cron_zone: None,
                    },
                )
                .await?;
            }
        } else if let (Some(expression), Some(zone), Some(schedule_key)) = (
            record.cron_expression.clone(),
            record.cron_zone.clone(),
            record.schedule_key.clone(),
        ) {
            let next_at =
                crate::schedule::next_after(&expression, &zone, now.max(record.available_at))?;
            let idempotency = format!("recur:{schedule_key}:{next_at}");
            if find_by_column(&transaction, "idempotency_key", &idempotency)
                .await?
                .is_none()
            {
                insert_job(
                    &transaction,
                    &NewJob {
                        id: Uuid::new_v4(),
                        handler: &record.handler,
                        version: record.payload_version,
                        payload: &record.payload,
                        available_at: next_at,
                        now,
                        max_attempts: record.max_attempts,
                        idempotency: Some(&idempotency),
                        interval: None,
                        schedule_key: Some(&schedule_key),
                        cron_expression: Some(&expression),
                        cron_zone: Some(&zone),
                    },
                )
                .await?;
            }
        }
        transaction.commit().await.map_err(database)?;
        Ok(Finish::Stored)
    }

    pub async fn finish_retry(
        &self,
        id: Uuid,
        token: &str,
        attempts: u32,
        max_attempts: u32,
        code: &str,
    ) -> Result<Finish, JobError> {
        let now = self.clock.unix()?;
        if attempts >= max_attempts {
            return self.finish_failed(id, token, "job.attempts").await;
        }
        let next_at = now.saturating_add(delay_secs(attempts, now, self.policy.jitter));
        conditional_status(
            &self.db,
            FinishUpdate {
                id,
                token,
                now,
                status: JobStatus::Retry,
                error_code: Some(owned_error(code)),
                finished_at: None,
                next_attempt_at: Some(next_at),
                clear_schedule: false,
            },
        )
        .await
    }

    pub async fn finish_failed(
        &self,
        id: Uuid,
        token: &str,
        code: &str,
    ) -> Result<Finish, JobError> {
        let now = self.clock.unix()?;
        conditional_status(
            &self.db,
            FinishUpdate {
                id,
                token,
                now,
                status: JobStatus::Failed,
                error_code: Some(owned_error(code)),
                finished_at: Some(now),
                next_attempt_at: None,
                clear_schedule: false,
            },
        )
        .await
    }

    pub async fn retry(&self, id: Uuid) -> Result<(), JobError> {
        self.ready().await?;
        let now = self.clock.unix()?;
        let mut update = Query::update();
        update
            .table(Alias::new(JOBS))
            .values([
                (Alias::new("status"), text(JobStatus::Pending.as_str())),
                (Alias::new("attempts"), number(0)),
                (Alias::new("available_at"), number(now)),
                (Alias::new("next_attempt_at"), null_number()),
                (Alias::new("lease_owner"), null_text()),
                (Alias::new("lease_token"), null_text()),
                (Alias::new("lease_expires_at"), null_number()),
                (Alias::new("error_code"), null_text()),
                (Alias::new("finished_at"), null_number()),
            ])
            .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
            .and_where(
                Expr::col(Alias::new("status"))
                    .eq(JobStatus::Failed.as_str())
                    .or(Expr::col(Alias::new("status")).eq(JobStatus::Cancelled.as_str())),
            );
        if execute_rows(&self.db, &update).await? == 1 {
            Ok(())
        } else {
            Err(JobError::message(
                "job.terminal",
                "Nur fehlgeschlagene oder abgebrochene Jobs können erneut gestartet werden.",
            ))
        }
    }

    pub async fn cancel(&self, id: Uuid) -> Result<(), JobError> {
        self.ready().await?;
        let now = self.clock.unix()?;
        let mut update = Query::update();
        update
            .table(Alias::new(JOBS))
            .values([
                (Alias::new("status"), text(JobStatus::Cancelled.as_str())),
                (Alias::new("finished_at"), number(now)),
                (Alias::new("lease_owner"), null_text()),
                (Alias::new("lease_token"), null_text()),
                (Alias::new("lease_expires_at"), null_number()),
                (Alias::new("error_code"), text("job.cancelled")),
            ])
            .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
            .and_where(
                Expr::col(Alias::new("status"))
                    .eq(JobStatus::Pending.as_str())
                    .or(Expr::col(Alias::new("status")).eq(JobStatus::Retry.as_str())),
            );
        if execute_rows(&self.db, &update).await? == 1 {
            Ok(())
        } else if load(&self.db, id)
            .await?
            .is_some_and(|job| job.status == JobStatus::Running)
        {
            Err(JobError::message(
                "job.running",
                "Ein laufender Job kann nicht abgebrochen werden.",
            ))
        } else {
            Err(JobError::message(
                "job.terminal",
                "Nur wartende Jobs können abgebrochen werden.",
            ))
        }
    }

    pub async fn retain(&self) -> Result<u64, JobError> {
        if !table_ready(&self.db).await? {
            return Ok(0);
        }
        let now = self.clock.unix()?;
        let cutoff = now.saturating_sub(self.policy.retention_secs.max(3600));
        let mut inner = Query::select();
        inner
            .column(Alias::new("id"))
            .from(Alias::new(JOBS))
            .and_where(Expr::col(Alias::new("status")).is_in([
                JobStatus::Succeeded.as_str(),
                JobStatus::Failed.as_str(),
                JobStatus::Cancelled.as_str(),
            ]))
            .and_where(Expr::col(Alias::new("finished_at")).lt(cutoff))
            .order_by(Alias::new("finished_at"), Order::Asc)
            .limit(100);
        let ids = all(&self.db, &inner).await?;
        if ids.is_empty() {
            return Ok(0);
        }
        let mut values = Vec::new();
        for row in ids {
            values.push(required_text(&row, "id")?);
        }
        let mut delete = Query::delete();
        delete
            .from_table(Alias::new(JOBS))
            .and_where(Expr::col(Alias::new("id")).is_in(values));
        execute_rows(&self.db, &delete).await
    }

    /// Endgültig fehlgeschlagene Jobs. Die Aufbewahrung löscht sie später mit.
    /// Das ist eine Inspektion, keine zweite Zustellung.
    pub async fn dead_letters(&self, limit: u32) -> Result<Vec<JobDiagnosis>, JobError> {
        self.ready().await?;
        let limit = limit.clamp(1, 100);
        let mut query = Query::select();
        query
            .columns([
                Alias::new("id"),
                Alias::new("handler"),
                Alias::new("status"),
                Alias::new("attempts"),
                Alias::new("max_attempts"),
                Alias::new("available_at"),
                Alias::new("error_code"),
                Alias::new("next_attempt_at"),
                Alias::new("finished_at"),
            ])
            .from(Alias::new(JOBS))
            .and_where(Expr::col(Alias::new("status")).eq(JobStatus::Failed.as_str()))
            .order_by(Alias::new("finished_at"), Order::Desc)
            .order_by(Alias::new("id"), Order::Asc)
            .limit(u64::from(limit));
        let mut rows = Vec::new();
        for row in all(&self.db, &query).await? {
            rows.push(JobDiagnosis {
                id: required_uuid(&row, "id")?,
                handler: required_text(&row, "handler")?,
                status: JobStatus::parse(&required_text(&row, "status")?)?,
                attempts: u32_column(&row, "attempts")?,
                max_attempts: u32_column(&row, "max_attempts")?,
                error_code: optional_text(&row, "error_code")?,
                available_at: required_i64(&row, "available_at")?,
                next_attempt_at: optional_i64(&row, "next_attempt_at")?,
                finished_at: optional_i64(&row, "finished_at")?,
            });
        }
        Ok(rows)
    }

    /// Schreibt in die Verbindung des Aufrufers. Ein Rollback entfernt den Job.
    /// Sichtbare Jobs bleiben at-least-once. Die Methode beginnt keine eigene Transaktion.
    /// Ein eindeutiger Konflikt bricht die Transaktion des Aufrufers ab. Der Aufrufer rollt zurück.
    pub async fn enqueue_json_on(
        &self,
        db: &impl ConnectionTrait,
        handler: &str,
        version: u32,
        payload: &impl Serialize,
        when: When,
    ) -> Result<EnqueueOutcome, JobError> {
        if !crate::schema::table_ready(db).await? {
            return Err(JobError::message(
                "job.migration.missing",
                "Die Job-Migrationen 0001_jobs und 0002_job_schedule sind nicht vollständig angewendet. ferrioxid migrate up legt sie an. Der Worker migriert nicht selbst.",
            ));
        }
        let planned = plan_enqueue(
            self.clock.as_ref(),
            self.policy.max_attempts,
            handler,
            version,
            payload,
            &when,
        )?;
        if let Some(key) = &planned.idempotency {
            if let Some(existing) = find_by_column(db, "idempotency_key", key).await? {
                return Ok(EnqueueOutcome::Existing(existing.id));
            }
        }
        if let Some(key) = &planned.schedule_key {
            if let Some(existing) = find_by_column(db, "schedule_key", key).await? {
                return Ok(EnqueueOutcome::Existing(existing.id));
            }
        }
        let id = Uuid::new_v4();
        insert_job(
            db,
            &NewJob {
                id,
                handler,
                version,
                payload: &planned.payload,
                available_at: planned.available_at,
                now: planned.now,
                max_attempts: planned.max_attempts,
                idempotency: planned.idempotency.as_deref(),
                interval: planned.interval,
                schedule_key: planned.schedule_key.as_deref(),
                cron_expression: planned.cron_expression.as_deref(),
                cron_zone: planned.cron_zone.as_deref(),
            },
        )
        .await?;
        Ok(EnqueueOutcome::Created(id))
    }
}

struct NewJob<'a> {
    id: Uuid,
    handler: &'a str,
    version: u32,
    payload: &'a str,
    available_at: i64,
    now: i64,
    max_attempts: u32,
    idempotency: Option<&'a str>,
    interval: Option<i64>,
    schedule_key: Option<&'a str>,
    cron_expression: Option<&'a str>,
    cron_zone: Option<&'a str>,
}

fn due(now: i64) -> sea_orm::sea_query::SimpleExpr {
    Expr::col(Alias::new("status"))
        .is_in([JobStatus::Pending.as_str(), JobStatus::Retry.as_str()])
        .and(Expr::col(Alias::new("available_at")).lte(now))
        .and(
            Expr::col(Alias::new("next_attempt_at"))
                .is_null()
                .or(Expr::col(Alias::new("next_attempt_at")).lte(now)),
        )
}

fn expired(now: i64) -> sea_orm::sea_query::SimpleExpr {
    Expr::col(Alias::new("status"))
        .eq(JobStatus::Running.as_str())
        .and(Expr::col(Alias::new("lease_expires_at")).lt(now))
}

struct FinishUpdate<'a> {
    id: Uuid,
    token: &'a str,
    now: i64,
    status: JobStatus,
    error_code: Option<String>,
    finished_at: Option<i64>,
    next_attempt_at: Option<i64>,
    clear_schedule: bool,
}

async fn conditional_status(
    db: &impl ConnectionTrait,
    update_job: FinishUpdate<'_>,
) -> Result<Finish, JobError> {
    let FinishUpdate {
        id,
        token,
        now,
        status,
        error_code,
        finished_at,
        next_attempt_at,
        clear_schedule,
    } = update_job;
    let mut update = Query::update();
    update
        .table(Alias::new(JOBS))
        .values([
            (Alias::new("status"), text(status.as_str())),
            (Alias::new("lease_owner"), null_text()),
            (Alias::new("lease_token"), null_text()),
            (Alias::new("lease_expires_at"), null_number()),
            (
                Alias::new("error_code"),
                match &error_code {
                    Some(code) => text(code),
                    None => null_text(),
                },
            ),
            (Alias::new("finished_at"), opt_number(finished_at)),
            (Alias::new("next_attempt_at"), opt_number(next_attempt_at)),
            (
                Alias::new("schedule_key"),
                if clear_schedule {
                    null_text()
                } else {
                    Expr::col(Alias::new("schedule_key")).into()
                },
            ),
        ])
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()))
        .and_where(Expr::col(Alias::new("lease_token")).eq(token))
        .and_where(Expr::col(Alias::new("status")).eq(JobStatus::Running.as_str()))
        .and_where(Expr::col(Alias::new("lease_expires_at")).gte(now));
    if execute_rows(db, &update).await? == 1 {
        Ok(Finish::Stored)
    } else {
        Ok(Finish::LostLease)
    }
}

async fn insert_job(db: &impl ConnectionTrait, job: &NewJob<'_>) -> Result<(), JobError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(JOBS))
        .columns([
            Alias::new("id"),
            Alias::new("handler"),
            Alias::new("payload_version"),
            Alias::new("payload"),
            Alias::new("status"),
            Alias::new("created_at"),
            Alias::new("available_at"),
            Alias::new("attempts"),
            Alias::new("max_attempts"),
            Alias::new("next_attempt_at"),
            Alias::new("lease_owner"),
            Alias::new("lease_token"),
            Alias::new("lease_expires_at"),
            Alias::new("error_code"),
            Alias::new("finished_at"),
            Alias::new("idempotency_key"),
            Alias::new("interval_secs"),
            Alias::new("schedule_key"),
            Alias::new("cron_expression"),
            Alias::new("cron_zone"),
        ])
        .values_panic([
            text(&job.id.to_string()),
            text(job.handler),
            number(i64::from(job.version)),
            text(job.payload),
            text(JobStatus::Pending.as_str()),
            number(job.now),
            number(job.available_at),
            number(0),
            number(i64::from(job.max_attempts)),
            null_number(),
            null_text(),
            null_text(),
            null_number(),
            null_text(),
            null_number(),
            match job.idempotency {
                Some(value) => text(value),
                None => null_text(),
            },
            opt_number(job.interval),
            match job.schedule_key {
                Some(value) => text(value),
                None => null_text(),
            },
            match job.cron_expression {
                Some(value) => text(value),
                None => null_text(),
            },
            match job.cron_zone {
                Some(value) => text(value),
                None => null_text(),
            },
        ]);
    match execute_rows(db, &query).await {
        Ok(_) => Ok(()),
        Err(error) if error.code() == "job.conflict" => Err(JobError::message(
            "job.conflict",
            "Der Job ist bereits eingetragen.",
        )),
        Err(error) => Err(error),
    }
}

async fn find_by_column(
    db: &impl ConnectionTrait,
    column: &str,
    value: &str,
) -> Result<Option<JobRecord>, JobError> {
    if column != "idempotency_key" && column != "schedule_key" {
        return Err(JobError::message(
            "job.database",
            "Die Spalte ist nicht für eine Suche freigegeben.",
        ));
    }
    let mut query = Query::select();
    query
        .columns(job_columns())
        .from(Alias::new(JOBS))
        .and_where(Expr::col(Alias::new(column)).eq(value));
    one(db, &query)
        .await?
        .map(|row| record_from_row(&row))
        .transpose()
}

async fn load(db: &impl ConnectionTrait, id: Uuid) -> Result<Option<JobRecord>, JobError> {
    let mut query = Query::select();
    query
        .columns(job_columns())
        .from(Alias::new(JOBS))
        .and_where(Expr::col(Alias::new("id")).eq(id.to_string()));
    one(db, &query)
        .await?
        .map(|row| record_from_row(&row))
        .transpose()
}

fn job_columns() -> [Alias; 20] {
    [
        Alias::new("id"),
        Alias::new("handler"),
        Alias::new("payload_version"),
        Alias::new("payload"),
        Alias::new("status"),
        Alias::new("created_at"),
        Alias::new("available_at"),
        Alias::new("attempts"),
        Alias::new("max_attempts"),
        Alias::new("next_attempt_at"),
        Alias::new("lease_owner"),
        Alias::new("lease_token"),
        Alias::new("lease_expires_at"),
        Alias::new("error_code"),
        Alias::new("finished_at"),
        Alias::new("idempotency_key"),
        Alias::new("interval_secs"),
        Alias::new("schedule_key"),
        Alias::new("cron_expression"),
        Alias::new("cron_zone"),
    ]
}

fn record_from_row(row: &QueryResult) -> Result<JobRecord, JobError> {
    Ok(JobRecord {
        id: required_uuid(row, "id")?,
        handler: required_text(row, "handler")?,
        payload_version: u32_column(row, "payload_version")?,
        payload: required_text(row, "payload")?,
        status: JobStatus::parse(&required_text(row, "status")?)?,
        created_at: required_i64(row, "created_at")?,
        available_at: required_i64(row, "available_at")?,
        attempts: u32_column(row, "attempts")?,
        max_attempts: u32_column(row, "max_attempts")?,
        next_attempt_at: optional_i64(row, "next_attempt_at")?,
        lease_owner: optional_text(row, "lease_owner")?,
        lease_token: optional_text(row, "lease_token")?,
        lease_expires_at: optional_i64(row, "lease_expires_at")?,
        error_code: optional_text(row, "error_code")?,
        finished_at: optional_i64(row, "finished_at")?,
        idempotency_key: optional_text(row, "idempotency_key")?,
        interval_secs: optional_i64(row, "interval_secs")?,
        schedule_key: optional_text(row, "schedule_key")?,
        cron_expression: optional_text(row, "cron_expression")?,
        cron_zone: optional_text(row, "cron_zone")?,
    })
}

fn summary_from_row(row: &QueryResult) -> Result<JobSummary, JobError> {
    Ok(JobSummary {
        id: required_uuid(row, "id")?,
        handler: required_text(row, "handler")?,
        status: JobStatus::parse(&required_text(row, "status")?)?,
        attempts: u32_column(row, "attempts")?,
        max_attempts: u32_column(row, "max_attempts")?,
        available_at: required_i64(row, "available_at")?,
        error_code: optional_text(row, "error_code")?,
    })
}

fn next_slot(available_at: i64, interval: i64, now: i64) -> i64 {
    let mut slot = available_at;
    let mut guard = 0;
    while slot <= now && guard < 10_000 {
        slot = slot.saturating_add(interval);
        guard += 1;
    }
    if slot <= now {
        now.saturating_add(interval)
    } else {
        slot
    }
}

pub fn delay_secs(attempt: u32, now: i64, jitter: bool) -> i64 {
    let shift = attempt.saturating_sub(1).min(8);
    let base = (1_i64 << shift).min(300);
    if !jitter {
        return base;
    }
    let spread = u64::try_from(base).unwrap_or(1);
    let extra = now.unsigned_abs() % spread.saturating_add(1);
    base.saturating_add(i64::try_from(extra).unwrap_or(0))
}

fn normalize_cron(
    expression: Option<&str>,
    zone: Option<&str>,
    schedule_key: Option<&str>,
    has_interval: bool,
) -> Result<Option<(String, String)>, JobError> {
    match (expression, zone) {
        (None, None) => Ok(None),
        (Some(expression), Some(zone)) if !has_interval && schedule_key.is_some() => {
            let _parsed = crate::schedule::parse_cron(expression)?;
            let parsed_zone = crate::schedule::parse_zone(zone)?;
            Ok(Some((
                expression.trim().to_owned(),
                parsed_zone.canonical(),
            )))
        }
        _ => Err(JobError::message(
            "job.schedule",
            "Cron braucht Ausdruck, Zeitzone und schedule_key und kein Intervall.",
        )),
    }
}

struct PlannedEnqueue {
    payload: String,
    available_at: i64,
    now: i64,
    max_attempts: u32,
    idempotency: Option<String>,
    interval: Option<i64>,
    schedule_key: Option<String>,
    cron_expression: Option<String>,
    cron_zone: Option<String>,
}

fn plan_enqueue(
    clock: &dyn crate::clock::Clock,
    policy_attempts: u32,
    handler: &str,
    version: u32,
    payload: &impl Serialize,
    when: &When,
) -> Result<PlannedEnqueue, JobError> {
    validate_handler_name(handler)?;
    if version == 0 {
        return Err(JobError::permanent("job.version"));
    }
    let encoded =
        serde_json::to_vec(payload).map_err(|_| JobError::permanent("job.payload.invalid"))?;
    if encoded.len() > MAX_PAYLOAD_BYTES {
        return Err(JobError::permanent("job.payload.large"));
    }
    let json: Json =
        serde_json::from_slice(&encoded).map_err(|_| JobError::permanent("job.payload.invalid"))?;
    if !json.is_object() {
        return Err(JobError::permanent("job.payload.invalid"));
    }
    reject_sensitive(&json)?;
    let max_attempts = when.max_attempts.unwrap_or(policy_attempts);
    if !(1..=MAX_ATTEMPTS).contains(&max_attempts) {
        return Err(JobError::message(
            "job.attempts",
            "max_attempts muss zwischen 1 und 20 liegen.",
        ));
    }
    let interval = normalize_interval(when.interval_secs, when.schedule_key.as_deref())?;
    let schedule_key = when
        .schedule_key
        .clone()
        .map(|key| validate_key(&key))
        .transpose()?;
    let cron = normalize_cron(
        when.cron_expression.as_deref(),
        when.cron_zone.as_deref(),
        schedule_key.as_deref(),
        interval.is_some(),
    )?;
    if interval.is_some() && schedule_key.is_none() {
        return Err(JobError::message(
            "job.schedule",
            "Ein Intervall braucht einen schedule_key.",
        ));
    }
    let now = clock.unix()?;
    let available_at = match &cron {
        Some((expression, zone)) if when.available_at.is_none() => {
            crate::schedule::next_after(expression, zone, now)?
        }
        _ => when.available_at.unwrap_or(now),
    };
    let idempotency = match when.idempotency_key.as_deref() {
        Some(key) => Some(validate_key(key)?),
        None => schedule_key
            .as_ref()
            .map(|key| format!("recur:{key}:{available_at}")),
    };
    let idempotency = idempotency.map(|key| validate_key(&key)).transpose()?;
    Ok(PlannedEnqueue {
        payload: String::from_utf8(encoded)
            .map_err(|_| JobError::permanent("job.payload.invalid"))?,
        available_at,
        now,
        max_attempts,
        idempotency,
        interval,
        schedule_key,
        cron_expression: cron.as_ref().map(|(expression, _)| expression.clone()),
        cron_zone: cron.as_ref().map(|(_, zone)| zone.clone()),
    })
}

fn normalize_interval(
    interval: Option<i64>,
    schedule: Option<&str>,
) -> Result<Option<i64>, JobError> {
    match (interval, schedule) {
        (None, _) => Ok(None),
        (Some(seconds), Some(_)) if (1..=2_592_000).contains(&seconds) => Ok(Some(seconds)),
        _ => Err(JobError::message(
            "job.schedule",
            "Ein Intervall braucht einen Schlüssel und liegt zwischen 1 und 2592000 Sekunden.",
        )),
    }
}

fn validate_handler_name(name: &str) -> Result<(), JobError> {
    let mut chars = name.chars();
    let ok = matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && (1..=64).contains(&name.len());
    if ok {
        Ok(())
    } else {
        Err(JobError::message(
            "job.handler",
            "Der Handlername besteht aus Kleinbuchstaben, Ziffern und Unterstrichen.",
        ))
    }
}

fn validate_key(value: &str) -> Result<String, JobError> {
    let ok = !value.is_empty()
        && value.len() <= 80
        && value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, ':' | '_' | '-'));
    if ok {
        Ok(value.to_owned())
    } else {
        Err(JobError::message(
            "job.handler",
            "Schlüssel bestehen aus Buchstaben, Ziffern, Doppelpunkt, Unterstrich und Bindestrich.",
        ))
    }
}

fn reject_sensitive(value: &Json) -> Result<(), JobError> {
    match value {
        Json::Object(map) => {
            for (key, child) in map {
                if sensitive_key(key) {
                    return Err(JobError::permanent("job.payload.secret"));
                }
                reject_sensitive(child)?;
            }
            Ok(())
        }
        Json::Array(items) => {
            for item in items {
                reject_sensitive(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "password"
            | "token"
            | "secret"
            | "reset_url"
            | "smtp_password"
            | "authorization"
            | "cookie"
            | "password_hash"
    ) || key.contains("password")
        || key.contains("secret")
        || key.contains("token")
}

fn owned_error(code: &str) -> String {
    let error = JobError::permanent(code);
    error.code().to_owned()
}

fn database(error: impl ToString) -> JobError {
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("unique") || text.contains("duplicate") {
        JobError::message("job.conflict", "Der Job ist bereits eingetragen.")
    } else if text.contains("locked") || text.contains("busy") {
        JobError::message(
            "job.locked",
            "Die Datenbank ist gerade gesperrt. Der Vorgang wurde nicht als Erfolg gewertet.",
        )
    } else {
        JobError::message(
            "job.database",
            "Die Job-Tabelle konnte nicht geändert werden.",
        )
    }
}

fn text(value: &str) -> sea_orm::sea_query::SimpleExpr {
    Expr::value(value)
}

fn number(value: i64) -> sea_orm::sea_query::SimpleExpr {
    Expr::value(value)
}

fn opt_number(value: Option<i64>) -> sea_orm::sea_query::SimpleExpr {
    match value {
        Some(value) => Expr::value(value),
        None => Expr::value(Value::BigInt(None)),
    }
}

fn null_number() -> sea_orm::sea_query::SimpleExpr {
    Expr::value(Value::BigInt(None))
}

fn null_text() -> sea_orm::sea_query::SimpleExpr {
    Expr::value(Value::String(None))
}

async fn execute_rows(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<u64, JobError> {
    let result = db.execute(statement(db, query)?).await.map_err(database)?;
    Ok(result.rows_affected())
}

async fn one(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Option<QueryResult>, JobError> {
    db.query_one(statement(db, query)?).await.map_err(database)
}

async fn all(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Vec<QueryResult>, JobError> {
    db.query_all(statement(db, query)?).await.map_err(database)
}

fn statement(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Statement, JobError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(JobError::message(
                "job.database",
                "MySQL wird nicht unterstützt.",
            ))
        }
    };
    Ok(Statement::from_sql_and_values(backend, sql, values))
}

fn required_text(row: &QueryResult, column: &str) -> Result<String, JobError> {
    row.try_get::<String>("", column).map_err(|_| {
        JobError::message(
            "job.database",
            "Eine Job-Spalte hat nicht den erwarteten Typ.",
        )
    })
}

fn optional_text(row: &QueryResult, column: &str) -> Result<Option<String>, JobError> {
    match row.try_get::<Option<String>>("", column) {
        Ok(Some(value)) if !value.is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(_) => Err(JobError::message(
            "job.database",
            "Eine Job-Spalte hat nicht den erwarteten Typ.",
        )),
    }
}

fn required_i64(row: &QueryResult, column: &str) -> Result<i64, JobError> {
    if let Ok(value) = row.try_get::<i64>("", column) {
        return Ok(value);
    }
    row.try_get::<i32>("", column).map(i64::from).map_err(|_| {
        JobError::message(
            "job.database",
            "Eine Job-Spalte hat nicht den erwarteten Typ.",
        )
    })
}

fn optional_i64(row: &QueryResult, column: &str) -> Result<Option<i64>, JobError> {
    if let Ok(value) = row.try_get::<Option<i64>>("", column) {
        return Ok(value);
    }
    row.try_get::<Option<i32>>("", column)
        .map(|value| value.map(i64::from))
        .map_err(|_| {
            JobError::message(
                "job.database",
                "Eine Job-Spalte hat nicht den erwarteten Typ.",
            )
        })
}

fn u32_column(row: &QueryResult, column: &str) -> Result<u32, JobError> {
    let value = required_i64(row, column)?;
    u32::try_from(value).map_err(|_| JobError::permanent("job.payload.invalid"))
}

fn required_uuid(row: &QueryResult, column: &str) -> Result<Uuid, JobError> {
    if let Ok(value) = row.try_get::<Uuid>("", column) {
        return Ok(value);
    }
    Uuid::parse_str(&required_text(row, column)?)
        .map_err(|_| JobError::message("job.database", "Die Job-ID ist kein UUID."))
}
