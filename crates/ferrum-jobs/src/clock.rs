//! UTC-Zeitpunkte als Unix-Sekunden. Tests setzen eine Uhr, ohne eine globale Registry.

use crate::error::JobError;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    fn unix(&self) -> Result<i64, JobError>;
}

#[derive(Clone, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn unix(&self) -> Result<i64, JobError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| JobError::message("job.clock", "Die Systemuhr liegt vor 1970."))?;
        i64::try_from(duration.as_secs()).map_err(|_| {
            JobError::message("job.clock", "Die Systemuhr liegt außerhalb des Bereichs.")
        })
    }
}

#[derive(Clone, Debug)]
pub struct ManualClock {
    now: Arc<Mutex<i64>>,
}

impl ManualClock {
    pub fn new(now: i64) -> Self {
        Self {
            now: Arc::new(Mutex::new(now)),
        }
    }

    pub fn set(&self, now: i64) {
        *self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = now;
    }

    pub fn advance(&self, seconds: i64) {
        let mut guard = self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = guard.saturating_add(seconds);
    }
}

impl Clock for ManualClock {
    fn unix(&self) -> Result<i64, JobError> {
        Ok(*self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }
}
