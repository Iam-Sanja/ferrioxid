//! Typisierte Handler. Die Registry gehört dem Prozess, der den Worker startet.

use crate::error::JobError;
use crate::queue::JobRecord;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct JobContext {
    pub id: Uuid,
    pub attempt: u32,
    pub now: i64,
}

#[async_trait]
pub trait JobHandler: Send + Sync + 'static {
    const NAME: &'static str;
    const VERSION: u32;
    type Payload: DeserializeOwned + Send + 'static;

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    async fn run(&self, payload: Self::Payload, ctx: &JobContext) -> Result<(), JobError>;
}

#[async_trait]
trait ErasedHandler: Send + Sync {
    fn version(&self) -> u32;
    fn timeout(&self) -> Duration;
    async fn execute(&self, payload: &str, ctx: &JobContext) -> Result<(), JobError>;
}

struct HandlerAdapter<H> {
    handler: H,
}

#[async_trait]
impl<H> ErasedHandler for HandlerAdapter<H>
where
    H: JobHandler,
{
    fn version(&self) -> u32 {
        H::VERSION
    }

    fn timeout(&self) -> Duration {
        self.handler.timeout()
    }

    async fn execute(&self, payload: &str, ctx: &JobContext) -> Result<(), JobError> {
        let parsed = serde_json::from_str::<H::Payload>(payload)
            .map_err(|_| JobError::permanent("job.payload.invalid"))?;
        self.handler.run(parsed, ctx).await
    }
}

#[derive(Default)]
pub struct JobRegistry {
    handlers: BTreeMap<String, Arc<dyn ErasedHandler>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<H: JobHandler>(&mut self, handler: H) -> Result<(), JobError> {
        let name = H::NAME;
        validate_registered_name(name)?;
        if H::VERSION == 0 {
            return Err(JobError::permanent("job.version"));
        }
        let timeout = handler.timeout();
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(JobError::message(
                "job.handler",
                "Das Zeitlimit eines Handlers liegt zwischen 1 und 120 Sekunden.",
            ));
        }
        if self.handlers.contains_key(name) {
            return Err(JobError::message(
                "job.handler",
                "Der Handler ist bereits registriert.",
            ));
        }
        self.handlers
            .insert(name.to_owned(), Arc::new(HandlerAdapter { handler }));
        Ok(())
    }

    pub fn names(&self) -> Vec<&str> {
        self.handlers.keys().map(String::as_str).collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    pub(crate) fn timeout_of(&self, name: &str) -> Duration {
        self.handlers
            .get(name)
            .map(|handler| handler.timeout())
            .unwrap_or(Duration::from_secs(30))
    }

    pub(crate) async fn execute(
        &self,
        record: &JobRecord,
        ctx: &JobContext,
    ) -> Result<(), JobError> {
        let Some(handler) = self.handlers.get(&record.handler) else {
            return Err(JobError::permanent("job.unknown_handler"));
        };
        if record.payload_version != handler.version() {
            return Err(JobError::permanent("job.version"));
        }
        handler.execute(&record.payload, ctx).await
    }
}

impl crate::queue::Queue {
    pub async fn enqueue<H>(
        &self,
        payload: &H::Payload,
        when: crate::queue::When,
    ) -> Result<crate::queue::EnqueueOutcome, JobError>
    where
        H: JobHandler,
        H::Payload: serde::Serialize,
    {
        self.enqueue_json(H::NAME, H::VERSION, payload, when).await
    }
}

fn validate_registered_name(name: &str) -> Result<(), JobError> {
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
