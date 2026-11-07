//! Beispiel-Jobs. Die App registriert sie nur in dem Prozess, der den Worker startet.

use ferrum::jobs::{JobContext, JobError, JobHandler};
use ferrum::serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotePulse {
    pub label: String,
}

pub struct NotePulseHandler {
    pub hits: Arc<AtomicU32>,
}

impl NotePulseHandler {
    pub fn new() -> Self {
        Self {
            hits: Arc::new(AtomicU32::new(0)),
        }
    }
}

impl Default for NotePulseHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[ferrum::async_trait::async_trait]
impl JobHandler for NotePulseHandler {
    const NAME: &'static str = "notes_pulse";
    const VERSION: u32 = 1;
    type Payload = NotePulse;

    async fn run(&self, payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        if payload.label.is_empty() {
            return Err(JobError::permanent("notes.empty"));
        }
        self.hits.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

pub struct NoteRetryHandler;

#[ferrum::async_trait::async_trait]
impl JobHandler for NoteRetryHandler {
    const NAME: &'static str = "notes_retry";
    const VERSION: u32 = 1;
    type Payload = NotePulse;

    async fn run(&self, _payload: Self::Payload, ctx: &JobContext) -> Result<(), JobError> {
        if ctx.attempt < 2 {
            return Err(JobError::retry("notes.temporary"));
        }
        Ok(())
    }
}

pub fn register(
    registry: &mut ferrum::jobs::JobRegistry,
    handler: impl JobHandler,
) -> Result<(), JobError> {
    registry.register(handler)
}
