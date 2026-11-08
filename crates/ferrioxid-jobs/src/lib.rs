//! Persistente Hintergrundjobs. Der Webserver startet den Worker nicht.
//!
//! Zustellung ist **at least once**. Ein Handler muss einen zweiten Lauf vertragen.
//! Externe Effekte wie SMTP kann eine Datenbanktransaktion nicht genau einmal garantieren.
//! Eine angenommene SMTP-Antwort ist keine Zustellbestätigung.

mod clock;
mod error;
mod host;
mod inspect;
mod mail;
mod queue;
mod registry;
mod schema;
mod worker;

pub use clock::{Clock, ManualClock, SystemClock};
pub use error::{payload_line, FailureKind, JobError};
pub use host::{WorkerConfig, WorkerHost};
pub use inspect::describe_job;
pub use mail::{enqueue_notice, MailNotice, MailNoticeHandler, MAIL_HANDLER};
pub use queue::{
    delay_secs, EnqueueOutcome, Finish, JobPolicy, JobRecord, JobStatus, JobSummary, Queue, When,
    MAX_ATTEMPTS, MAX_PAYLOAD_BYTES,
};
pub use registry::{JobContext, JobHandler, JobRegistry};
pub use schema::{
    apply_migrations, migrations_dir, render_migration_status, table_ready, JOBS, JOB_HISTORY,
};
pub use worker::{stop_flag, Worker};

#[cfg(test)]
mod tests;
