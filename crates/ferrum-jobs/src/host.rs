//! Start eines Anwendungs-Workers. Es gibt keine zweite Queue.

use crate::error::JobError;
use crate::mail::{MailNoticeHandler, MAIL_HANDLER};
use crate::queue::{JobPolicy, Queue};
use crate::registry::{JobHandler, JobRegistry};
use crate::worker::{stop_flag, Worker};
use crate::SystemClock;
use ferrum_mail::MailService;
use ferrum_migrations::ProjectConfig;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Einstellungen, die der Worker aus dem Projekt oder von Hand erhält.
#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub database_url: String,
    pub enabled: bool,
    pub poll_ms: u64,
    pub concurrency: u16,
    pub policy: JobPolicy,
}

impl WorkerConfig {
    pub fn from_project(project: &ProjectConfig) -> Self {
        Self {
            database_url: project.resolved_database_url(),
            enabled: project.jobs_enabled,
            poll_ms: project.jobs_poll_ms,
            concurrency: project.jobs_concurrency,
            policy: JobPolicy {
                max_attempts: project.jobs_max_attempts,
                retention_secs: i64::from(project.jobs_retention_hours) * 3600,
                jitter: project.jobs_jitter,
                lease_secs: i64::try_from(project.jobs_lease_secs).unwrap_or(60),
            },
        }
    }
}

/// Baut Registry, Mail-Handler und Worker. Der Aufruf startet keinen Webserver und keine Migration.
pub struct WorkerHost {
    config: WorkerConfig,
    registry: JobRegistry,
    mail: Option<MailService>,
}

impl WorkerHost {
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            config,
            registry: JobRegistry::new(),
            mail: None,
        }
    }

    pub fn from_project(project: &ProjectConfig) -> Self {
        Self::new(WorkerConfig::from_project(project))
    }

    pub fn with_mail(mut self, mail: MailService) -> Self {
        self.mail = Some(mail);
        self
    }

    pub fn registry_mut(&mut self) -> &mut JobRegistry {
        &mut self.registry
    }

    pub fn register(&mut self, handler: impl JobHandler) -> Result<(), JobError> {
        self.registry.register(handler)
    }

    /// SQLite bleibt bei einer Verbindung. Ein höherer Wert wird auf 1 gesetzt.
    pub fn effective_concurrency(&self) -> u16 {
        let requested = self.config.concurrency.clamp(1, 4);
        if self.config.database_url.starts_with("sqlite") {
            1
        } else {
            requested
        }
    }

    pub async fn prepare(self) -> Result<Worker, JobError> {
        if !self.config.enabled {
            return Err(JobError::message(
                "job.disabled",
                "Jobs sind ausgeschaltet. ferrum dev und der Webserver starten keinen Worker.",
            ));
        }
        ensure_sqlite_exists(&self.config.database_url)?;
        let concurrency = self.effective_concurrency();
        if self.config.concurrency > 1 && self.config.database_url.starts_with("sqlite") {
            tracing::warn!(
                code = "job.sqlite.concurrency",
                "sqlite worker stays on one connection"
            );
        }
        let db = ferrum_db::connect(&self.config.database_url)
            .await
            .map_err(|_| {
                JobError::message("job.database", "Die Datenbank ist nicht erreichbar.")
            })?;
        let lease_secs = self.config.policy.lease_secs;
        let queue = Queue::new(db, Arc::new(SystemClock), self.config.policy);
        queue.ready().await?;
        let mut registry = self.registry;
        if let Some(mail) = self.mail {
            if !registry.contains(MAIL_HANDLER) {
                registry.register(MailNoticeHandler::new(mail))?;
            }
        }
        Ok(Worker::new(
            queue,
            registry,
            lease_secs,
            self.config.poll_ms,
            concurrency,
        ))
    }

    /// Läuft bis Ctrl+C. Der laufende Handler wird nicht mitten im Effekt abgebrochen.
    pub fn run_until_ctrl_c(self) -> Result<(), JobError> {
        let runtime = tokio::runtime::Runtime::new().map_err(|_| {
            JobError::message("job.runtime", "Die Laufzeitumgebung konnte nicht starten.")
        })?;
        runtime.block_on(async move {
            let stop = stop_flag();
            let flag = stop.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    flag.store(true, Ordering::SeqCst);
                }
            });
            let worker = self.prepare().await?;
            worker.run(&stop).await
        })
    }
}

fn ensure_sqlite_exists(url: &str) -> Result<(), JobError> {
    let Some(path) = sqlite_file(url) else {
        return Ok(());
    };
    if Path::new(path).is_file() {
        Ok(())
    } else {
        Err(JobError::message(
            "job.database",
            "Die SQLite-Datei fehlt. Es wird keine Datei angelegt.",
        ))
    }
}

fn sqlite_file(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("sqlite://")?;
    let path = rest.split('?').next().unwrap_or(rest);
    if path.is_empty() || path == ":memory:" {
        None
    } else {
        Some(path)
    }
}
