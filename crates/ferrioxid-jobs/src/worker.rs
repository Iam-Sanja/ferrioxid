//! Worker-Schleife. Sie startet keinen Webserver und wendet keine Migration an.

use crate::error::JobError;
use crate::queue::{Finish, Queue};
use crate::registry::{JobContext, JobRegistry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub struct Worker {
    queue: Queue,
    registry: JobRegistry,
    id: String,
    lease_secs: i64,
    poll: Duration,
    concurrency: usize,
}

impl Worker {
    pub fn new(
        queue: Queue,
        registry: JobRegistry,
        lease_secs: i64,
        poll_ms: u64,
        concurrency: u16,
    ) -> Self {
        Self {
            queue,
            registry,
            id: Uuid::new_v4().to_string(),
            lease_secs: lease_secs.clamp(5, 600),
            poll: Duration::from_millis(poll_ms.clamp(50, 60_000)),
            concurrency: usize::from(concurrency.clamp(1, 4)),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Holt Jobs, bis `stop` gesetzt ist. Ein bereits geholter Handler läuft zu Ende
    /// oder bis zu seinem Timeout. Danach wird nichts Neues mehr geholt.
    pub async fn run(&self, stop: &AtomicBool) -> Result<(), JobError> {
        self.queue.ready().await?;
        while !stop.load(Ordering::SeqCst) {
            let mut progressed = false;
            for _ in 0..self.concurrency {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match self.run_once().await {
                    Ok(true) => progressed = true,
                    Ok(false) => break,
                    Err(error) if error.code() == "job.locked" => {
                        tracing::warn!(code = "job.locked", "job claim deferred");
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
            if self.queue.retain().await.is_err() {
                tracing::warn!(code = "job.database", "job retention skipped");
            }
            if stop.load(Ordering::SeqCst) || progressed {
                continue;
            }
            tokio::time::sleep(self.poll).await;
        }
        Ok(())
    }

    pub async fn run_once(&self) -> Result<bool, JobError> {
        let names = self.registry.names();
        let claimed = self
            .queue
            .claim(&self.id, &names, self.lease_for(&names))
            .await?;
        let Some((record, token)) = claimed else {
            return Ok(false);
        };
        let now = self.queue.clock().unix()?;
        let ctx = JobContext {
            id: record.id,
            attempt: record.attempts,
            now,
            idempotency_key: record.idempotency_key.clone(),
        };
        let timeout = self.registry.timeout_of(&record.handler);
        let outcome = tokio::time::timeout(timeout, self.registry.execute(&record, &ctx)).await;
        let finish = match outcome {
            Ok(Ok(())) => {
                self.queue
                    .finish_success(record.id, &token, &record)
                    .await?
            }
            Ok(Err(error)) if error.is_retry() => {
                self.queue
                    .finish_retry(
                        record.id,
                        &token,
                        record.attempts,
                        record.max_attempts,
                        error.code(),
                    )
                    .await?
            }
            Ok(Err(error)) => {
                self.queue
                    .finish_failed(record.id, &token, error.code())
                    .await?
            }
            Err(_) => {
                self.queue
                    .finish_retry(
                        record.id,
                        &token,
                        record.attempts,
                        record.max_attempts,
                        "job.timeout",
                    )
                    .await?
            }
        };
        if finish == Finish::LostLease {
            tracing::warn!(
                code = "job.lease.lost",
                "stale worker did not finish the job"
            );
        }
        Ok(true)
    }

    fn lease_for(&self, names: &[&str]) -> i64 {
        let mut lease = self.lease_secs;
        for name in names {
            let timeout = i64::try_from(self.registry.timeout_of(name).as_secs()).unwrap_or(30);
            lease = lease.max(timeout.saturating_add(5));
        }
        lease.min(600)
    }
}

pub fn stop_flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
