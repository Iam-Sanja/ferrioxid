use super::*;
use async_trait::async_trait;
use fexu_db::driver::{ConnectionTrait, TransactionTrait};
use fexu_mail::{MailService, Notice};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    label: String,
}

struct CountJob {
    hits: Arc<AtomicU32>,
    retries_left: Arc<AtomicU32>,
    permanent: bool,
}

#[async_trait]
impl JobHandler for CountJob {
    const NAME: &'static str = "notes_count";
    const VERSION: u32 = 1;
    type Payload = Label;

    async fn run(&self, payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        assert!(!payload.label.contains("rm "));
        self.hits.fetch_add(1, Ordering::SeqCst);
        let left = self.retries_left.load(Ordering::SeqCst);
        if left > 0 {
            self.retries_left.fetch_sub(1, Ordering::SeqCst);
            return if self.permanent {
                Err(JobError::permanent("notes.denied"))
            } else {
                Err(JobError::retry("notes.busy"))
            };
        }
        Ok(())
    }
}

struct SlowJob;

#[async_trait]
impl JobHandler for SlowJob {
    const NAME: &'static str = "notes_slow";
    const VERSION: u32 = 1;
    type Payload = Label;

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(20)
    }

    async fn run(&self, _payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        Ok(())
    }
}

fn temp_url() -> (String, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "fexu-jobs-{}-{}.sqlite",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let rendered = path.to_string_lossy().replace('\\', "/");
    (format!("sqlite://{rendered}?mode=rwc"), path)
}

async fn started() -> (Queue, ManualClock, std::path::PathBuf) {
    let (url, path) = temp_url();
    let db = fexu_db::connect(&url).await.expect("sqlite");
    apply_migrations(&db).await.expect("migration");
    let clock = ManualClock::new(1_700_000_000);
    let queue = Queue::new(db, Arc::new(clock.clone()), JobPolicy::default());
    (queue, clock, path)
}

fn registry(hits: Arc<AtomicU32>, retries: Arc<AtomicU32>, permanent: bool) -> JobRegistry {
    let mut registry = JobRegistry::new();
    registry
        .register(CountJob {
            hits,
            retries_left: retries,
            permanent,
        })
        .expect("register");
    registry
}

fn worker(queue: Queue, registry: JobRegistry) -> Worker {
    Worker::new(queue, registry, 60, 50, 1)
}

#[tokio::test]
async fn create_run_and_succeed() {
    let (queue, _clock, path) = started().await;
    let hits = Arc::new(AtomicU32::new(0));
    let id = match queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "now".into(),
            },
            When::now(),
        )
        .await
        .expect("enqueue")
    {
        EnqueueOutcome::Created(id) => id,
        EnqueueOutcome::Existing(_) => panic!("new job"),
    };
    let ran = worker(
        queue.clone(),
        registry(hits.clone(), Arc::new(AtomicU32::new(0)), false),
    )
    .run_once()
    .await
    .expect("run");
    assert!(ran);
    let stored = queue.get(id).await.expect("get");
    assert_eq!(stored.status, JobStatus::Succeeded);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(stored.attempts, 1);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn temporary_failure_waits_for_the_clock_then_succeeds() {
    let (queue, clock, path) = started().await;
    let hits = Arc::new(AtomicU32::new(0));
    let id = created(
        &queue,
        &Label {
            label: "later".into(),
        },
        When::now(),
    )
    .await;
    let runner = worker(
        queue.clone(),
        registry(hits.clone(), Arc::new(AtomicU32::new(1)), false),
    );
    runner.run_once().await.expect("first");
    let waiting = queue.get(id).await.expect("waiting");
    assert_eq!(waiting.status, JobStatus::Retry);
    assert_eq!(waiting.error_code.as_deref(), Some("notes.busy"));
    assert!(!runner.run_once().await.expect("too early"));
    clock.advance(2);
    runner.run_once().await.expect("second");
    assert_eq!(
        queue.get(id).await.expect("done").status,
        JobStatus::Succeeded
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn permanent_failure_does_not_retry() {
    let (queue, clock, path) = started().await;
    let id = created(&queue, &Label { label: "no".into() }, When::now()).await;
    let runner = worker(
        queue.clone(),
        registry(
            Arc::new(AtomicU32::new(0)),
            Arc::new(AtomicU32::new(1)),
            true,
        ),
    );
    runner.run_once().await.expect("fail");
    clock.advance(10);
    assert!(!runner.run_once().await.expect("idle"));
    let stored = queue.get(id).await.expect("stored");
    assert_eq!(stored.status, JobStatus::Failed);
    assert_eq!(stored.error_code.as_deref(), Some("notes.denied"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn delayed_job_stays_until_the_deadline() {
    let (queue, clock, path) = started().await;
    let start = clock.unix().expect("now");
    let id = created(
        &queue,
        &Label {
            label: "delay".into(),
        },
        When::at(start + 100),
    )
    .await;
    let runner = worker(
        queue.clone(),
        registry(
            Arc::new(AtomicU32::new(0)),
            Arc::new(AtomicU32::new(0)),
            false,
        ),
    );
    assert!(!runner.run_once().await.expect("early"));
    clock.set(start + 100);
    assert!(runner.run_once().await.expect("due"));
    assert_eq!(
        queue.get(id).await.expect("done").status,
        JobStatus::Succeeded
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn recurring_job_plans_one_successor() {
    let (queue, clock, path) = started().await;
    let when = When::every("notes-daily", 60);
    let first = created(
        &queue,
        &Label {
            label: "repeat".into(),
        },
        when.clone(),
    )
    .await;
    let again = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "repeat".into(),
            },
            when,
        )
        .await
        .expect("second");
    assert_eq!(again, EnqueueOutcome::Existing(first));
    let runner = worker(
        queue.clone(),
        registry(
            Arc::new(AtomicU32::new(0)),
            Arc::new(AtomicU32::new(0)),
            false,
        ),
    );
    runner.run_once().await.expect("run");
    let done = queue.get(first).await.expect("done");
    assert_eq!(done.status, JobStatus::Succeeded);
    assert!(done.schedule_key.is_none());
    let listed = queue.list(20).await.expect("list");
    let pending = listed
        .iter()
        .filter(|job| job.status == JobStatus::Pending)
        .count();
    assert_eq!(pending, 1);
    assert!(!runner.run_once().await.expect("not due"));
    clock.advance(60);
    assert!(runner.run_once().await.expect("next"));
    let pending = queue
        .list(20)
        .await
        .expect("list")
        .into_iter()
        .filter(|job| job.status == JobStatus::Pending)
        .count();
    assert_eq!(pending, 1);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn two_workers_claim_one_job() {
    let (url, path) = temp_url();
    let clock = ManualClock::new(1_700_000_000);
    let first_db = fexu_db::connect(&url).await.expect("db");
    apply_migrations(&first_db).await.expect("migration");
    let second_db = fexu_db::connect(&url).await.expect("db2");
    let policy = JobPolicy::default();
    let left = Queue::new(first_db, Arc::new(clock.clone()), policy.clone());
    let right = Queue::new(second_db, Arc::new(clock), policy);
    created(
        &left,
        &Label {
            label: "once".into(),
        },
        When::now(),
    )
    .await;
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let left_barrier = barrier.clone();
    let right_barrier = barrier;
    let (left_claim, right_claim) = tokio::join!(
        async {
            left_barrier.wait().await;
            left.claim("left", &[CountJob::NAME], 60).await
        },
        async {
            right_barrier.wait().await;
            right.claim("right", &[CountJob::NAME], 60).await
        }
    );
    let claims = [left_claim.expect("left"), right_claim.expect("right")];
    let owned = claims.iter().filter(|claim| claim.is_some()).count();
    assert_eq!(owned, 1);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn expired_lease_can_be_reclaimed_and_the_old_token_cannot_finish() {
    let (queue, clock, path) = started().await;
    let id = created(
        &queue,
        &Label {
            label: "crash".into(),
        },
        When::now(),
    )
    .await;
    let (record, token) = queue
        .claim("old", &[CountJob::NAME], 30)
        .await
        .expect("claim")
        .expect("row");
    clock.advance(31);
    let (reclaimed, new_token) = queue
        .claim("new", &[CountJob::NAME], 30)
        .await
        .expect("reclaim")
        .expect("row");
    assert_eq!(reclaimed.attempts, 2);
    assert_ne!(token, new_token);
    let lost = queue
        .finish_success(id, &token, &record)
        .await
        .expect("stale");
    assert_eq!(lost, Finish::LostLease);
    let stored = queue.get(id).await.expect("stored");
    assert_eq!(stored.status, JobStatus::Running);
    assert_eq!(stored.lease_token.as_deref(), Some(new_token.as_str()));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn idempotency_key_returns_the_same_job() {
    let (queue, _clock, path) = started().await;
    let mut when = When::now();
    when.idempotency_key = Some("order-17".into());
    let first = created(
        &queue,
        &Label {
            label: "same".into(),
        },
        when.clone(),
    )
    .await;
    let second = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "same".into(),
            },
            when,
        )
        .await
        .expect("again");
    assert_eq!(second, EnqueueOutcome::Existing(first));
    assert_eq!(queue.list(10).await.expect("list").len(), 1);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn missing_migration_is_reported_without_creating_a_table() {
    let (url, path) = temp_url();
    let db = fexu_db::connect(&url).await.expect("db");
    let queue = Queue::new(db, Arc::new(ManualClock::new(10)), JobPolicy::default());
    let error = queue.ready().await.expect_err("missing");
    assert_eq!(error.code(), "job.migration.missing");
    assert!(!table_ready(queue.database()).await.expect("probe"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn unknown_handler_version_secret_and_size_are_rejected() {
    let (queue, _clock, path) = started().await;
    let id = created(
        &queue,
        &Label {
            label: "kept".into(),
        },
        When::now(),
    )
    .await;
    let runner = Worker::new(queue.clone(), JobRegistry::new(), 60, 50, 1);
    assert!(!runner.run_once().await.expect("skip"));
    assert_eq!(
        queue.get(id).await.expect("still").status,
        JobStatus::Pending
    );
    let secret = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &serde_json::json!({"password": "hidden"}),
            When::now(),
        )
        .await
        .expect_err("secret");
    assert_eq!(secret.code(), "job.payload.secret");
    let version = queue
        .enqueue_json(CountJob::NAME, 2, &Label { label: "x".into() }, When::now())
        .await
        .expect("stored version");
    let EnqueueOutcome::Created(wrong) = version else {
        panic!("created");
    };
    let mut registry = JobRegistry::new();
    registry
        .register(CountJob {
            hits: Arc::new(AtomicU32::new(0)),
            retries_left: Arc::new(AtomicU32::new(0)),
            permanent: false,
        })
        .expect("register");
    let runner = Worker::new(queue.clone(), registry, 60, 50, 1);
    for _ in 0..4 {
        if !runner.run_once().await.expect("version run") {
            break;
        }
    }
    let failed = queue.list(10).await.expect("list");
    assert!(failed.iter().any(|job| job.id == wrong
        && job.status == JobStatus::Failed
        && job.error_code.as_deref() == Some("job.version")));
    let huge = "x".repeat(MAX_PAYLOAD_BYTES);
    let large = queue
        .enqueue_json(CountJob::NAME, 1, &Label { label: huge }, When::now())
        .await
        .expect_err("large");
    assert_eq!(large.code(), "job.payload.large");
    let attempts = When {
        max_attempts: Some(50),
        ..When::now()
    };
    assert_eq!(
        queue
            .enqueue_json(CountJob::NAME, 1, &Label { label: "n".into() }, attempts)
            .await
            .expect_err("attempts")
            .code(),
        "job.attempts"
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn manual_retry_and_cancel_are_conditional() {
    let (queue, _clock, path) = started().await;
    let id = created(
        &queue,
        &Label {
            label: "stop".into(),
        },
        When::now(),
    )
    .await;
    queue.cancel(id).await.expect("cancel");
    assert_eq!(
        queue.get(id).await.expect("cancelled").status,
        JobStatus::Cancelled
    );
    queue.retry(id).await.expect("retry");
    assert_eq!(
        queue.get(id).await.expect("pending").status,
        JobStatus::Pending
    );
    let _ = queue
        .claim("worker", &[CountJob::NAME], 60)
        .await
        .expect("claim");
    assert_eq!(
        queue.cancel(id).await.expect_err("running").code(),
        "job.running"
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn timeout_becomes_a_retry_and_shutdown_claims_nothing() {
    let (queue, _clock, path) = started().await;
    let id = queue
        .enqueue_json(
            SlowJob::NAME,
            1,
            &Label {
                label: "slow".into(),
            },
            When::now(),
        )
        .await
        .expect("enqueue");
    let EnqueueOutcome::Created(id) = id else {
        panic!("created")
    };
    let mut registry = JobRegistry::new();
    registry.register(SlowJob).expect("slow");
    let runner = Worker::new(queue.clone(), registry, 60, 50, 1);
    runner.run_once().await.expect("timeout");
    assert_eq!(queue.get(id).await.expect("retry").status, JobStatus::Retry);
    let stop = stop_flag();
    stop.store(true, Ordering::SeqCst);
    runner.run(&stop).await.expect("stopped");
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn mail_notice_is_delivered_and_reset_tokens_are_refused() {
    let (queue, _clock, path) = started().await;
    let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
    let refused = enqueue_notice(
        &queue,
        &Notice::PasswordResetRequested {
            to: "person@example.com".into(),
            locale: "de".into(),
            reset_url: "http://127.0.0.1:3000/auth/password-reset/confirm?token=raw-token".into(),
            minutes: 30,
        },
        When::now(),
    )
    .await
    .expect_err("refused");
    assert_eq!(refused.code(), "job.mail.reset_refused");
    assert!(inbox.messages().is_empty());
    let id = enqueue_notice(
        &queue,
        &Notice::PasswordChanged {
            to: "person@example.com".into(),
            locale: "de".into(),
        },
        When::now(),
    )
    .await
    .expect("notice");
    let EnqueueOutcome::Created(id) = id else {
        panic!("created")
    };
    let stored = queue.get(id).await.expect("payload");
    assert!(!stored.payload.contains("raw-token"));
    assert!(!stored.payload.contains("reset_url"));
    assert!(stored.payload.contains("password_changed"));
    let mut registry = JobRegistry::new();
    registry
        .register(MailNoticeHandler::new(mail))
        .expect("mail");
    Worker::new(queue.clone(), registry, 60, 50, 1)
        .run_once()
        .await
        .expect("send");
    assert_eq!(inbox.messages().len(), 1);
    assert_eq!(
        queue.get(id).await.expect("sent").status,
        JobStatus::Succeeded
    );
    assert!(!format!("{:?}", inbox.messages()[0].text).contains("smtp"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn existing_reset_outbox_is_not_sent() {
    let (url, path) = temp_url();
    let db = fexu_db::connect(&url).await.expect("db");
    fexu_auth::apply_migrations(&db).await.expect("auth");
    apply_migrations(&db).await.expect("jobs");
    db.execute(fexu_db::driver::Statement::from_string(
        db.get_database_backend(),
        "INSERT INTO _fexu_mail_outbox (id, kind, recipient, reset_id, created_at, sent_at, failed_at, attempts, error_code) VALUES ('11111111-1111-1111-1111-111111111111', 'password_reset', 'person@example.com', NULL, 10, NULL, NULL, 0, NULL)",
    ))
    .await
    .expect("outbox");
    let queue = Queue::new(
        db.clone(),
        Arc::new(ManualClock::new(20)),
        JobPolicy::default(),
    );
    let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
    let mut registry = JobRegistry::new();
    registry
        .register(MailNoticeHandler::new(mail))
        .expect("mail");
    assert!(!Worker::new(queue, registry, 60, 50, 1)
        .run_once()
        .await
        .expect("idle"));
    assert!(inbox.messages().is_empty());
    let row = db
        .query_one(fexu_db::driver::Statement::from_string(
            db.get_database_backend(),
            "SELECT error_code FROM _fexu_mail_outbox",
        ))
        .await
        .expect("read")
        .expect("row");
    let code: Option<String> = row.try_get("", "error_code").expect("code");
    assert!(code.is_none());
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn retention_removes_old_finished_jobs_only() {
    let (queue, clock, path) = started().await;
    let id = created(
        &queue,
        &Label {
            label: "old".into(),
        },
        When::now(),
    )
    .await;
    let runner = worker(
        queue.clone(),
        registry(
            Arc::new(AtomicU32::new(0)),
            Arc::new(AtomicU32::new(0)),
            false,
        ),
    );
    runner.run_once().await.expect("done");
    clock.advance(8 * 24 * 60 * 60);
    let removed = queue.retain().await.expect("retain");
    assert_eq!(removed, 1);
    assert_eq!(
        queue.get(id).await.expect_err("gone").code(),
        "job.not_found"
    );
    let live = created(
        &queue,
        &Label {
            label: "live".into(),
        },
        When::now(),
    )
    .await;
    assert_eq!(
        queue.get(live).await.expect("kept").status,
        JobStatus::Pending
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn unsafe_error_codes_are_replaced() {
    assert_eq!(JobError::retry("password=secret").code(), "job.error");
    assert!(!JobError::retry("password=secret")
        .detail()
        .contains("secret"));
    let hidden = JobError::message("job.error", "password=hunter2");
    assert!(!hidden.detail().contains("hunter2"));
    assert!(!hidden.detail().contains("password"));
    assert_eq!(JobError::permanent("notes.busy").code(), "notes.busy");
    assert_eq!(delay_secs(1, 10, false), 1);
    assert!(delay_secs(3, 10, true) >= 4);
}

struct SplitJob {
    before: Arc<AtomicU32>,
    after: Arc<AtomicU32>,
}

#[async_trait]
impl JobHandler for SplitJob {
    const NAME: &'static str = "notes_split";
    const VERSION: u32 = 1;
    type Payload = Label;

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(30)
    }

    async fn run(&self, _payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        self.before.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        self.after.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct AttemptJob;

#[async_trait]
impl JobHandler for AttemptJob {
    const NAME: &'static str = "notes_attempt";
    const VERSION: u32 = 1;
    type Payload = Label;

    async fn run(&self, _payload: Self::Payload, ctx: &JobContext) -> Result<(), JobError> {
        if ctx.attempt < 2 {
            return Err(JobError::retry("notes.temporary"));
        }
        Ok(())
    }
}

#[tokio::test]
async fn timeout_drops_the_future_and_keeps_the_earlier_effect() {
    let (queue, _clock, path) = started().await;
    let before = Arc::new(AtomicU32::new(0));
    let after = Arc::new(AtomicU32::new(0));
    let id = queue
        .enqueue_json(
            SplitJob::NAME,
            1,
            &Label {
                label: "split".into(),
            },
            When::now(),
        )
        .await
        .expect("enqueue");
    let EnqueueOutcome::Created(id) = id else {
        panic!("created");
    };
    let mut registry = JobRegistry::new();
    registry
        .register(SplitJob {
            before: before.clone(),
            after: after.clone(),
        })
        .expect("split");
    let runner = worker(queue.clone(), registry);
    runner.run_once().await.expect("timeout");
    assert_eq!(before.load(Ordering::SeqCst), 1);
    assert_eq!(after.load(Ordering::SeqCst), 0);
    assert_eq!(queue.get(id).await.expect("job").status, JobStatus::Retry);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn retry_survives_a_new_worker_process_connection() {
    let (url, path) = temp_url();
    let db = fexu_db::connect(&url).await.expect("sqlite");
    apply_migrations(&db).await.expect("migration");
    let clock = ManualClock::new(1_700_000_000);
    let shared = Arc::new(clock.clone());
    let queue = Queue::new(
        db,
        shared.clone(),
        JobPolicy {
            jitter: false,
            ..JobPolicy::default()
        },
    );
    let id = queue
        .enqueue_json(
            AttemptJob::NAME,
            1,
            &Label {
                label: "again".into(),
            },
            When::now(),
        )
        .await
        .expect("enqueue");
    let EnqueueOutcome::Created(id) = id else {
        panic!("created");
    };
    let mut registry = JobRegistry::new();
    registry.register(AttemptJob).expect("attempt");
    let first = Worker::new(queue, registry, 60, 50, 1);
    assert!(first.run_once().await.expect("retry"));
    drop(first);
    clock.advance(2);
    let db = fexu_db::connect(&url).await.expect("reopen");
    let queue = Queue::new(
        db,
        shared,
        JobPolicy {
            jitter: false,
            ..JobPolicy::default()
        },
    );
    let mut registry = JobRegistry::new();
    registry.register(AttemptJob).expect("second");
    let second = Worker::new(queue.clone(), registry, 60, 50, 1);
    assert!(second.run_once().await.expect("done"));
    assert_eq!(
        queue.get(id).await.expect("stored").status,
        JobStatus::Succeeded
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn worker_host_runs_without_a_server_and_reports_a_missing_migration() {
    let (url, path) = temp_url();
    let missing = WorkerHost::new(host_config(&url, 4));
    let error = match missing.prepare().await {
        Err(error) => error,
        Ok(_) => panic!("missing file"),
    };
    assert_eq!(error.code(), "job.database");
    assert!(!path.exists());

    let db = fexu_db::connect(&url).await.expect("sqlite");
    drop(db);
    let bare = WorkerHost::new(host_config(&url, 1));
    let error = match bare.prepare().await {
        Err(error) => error,
        Ok(_) => panic!("migration"),
    };
    assert_eq!(error.code(), "job.migration.missing");
    let db = fexu_db::connect(&url).await.expect("check");
    assert!(!table_ready(&db).await.expect("ready"));
    drop(db);

    apply_migrations(&fexu_db::connect(&url).await.expect("sqlite"))
        .await
        .expect("migration");
    let setup = Queue::new(
        fexu_db::connect(&url).await.expect("sqlite"),
        Arc::new(SystemClock),
        JobPolicy::default(),
    );
    let id = setup
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "host".into(),
            },
            When::now(),
        )
        .await
        .expect("enqueue");
    let EnqueueOutcome::Created(id) = id else {
        panic!("created");
    };
    drop(setup);
    let hits = Arc::new(AtomicU32::new(0));
    let mut host = WorkerHost::new(host_config(&url, 4));
    host.register(CountJob {
        hits: hits.clone(),
        retries_left: Arc::new(AtomicU32::new(0)),
        permanent: false,
    })
    .expect("register");
    let duplicate = host
        .register(CountJob {
            hits: hits.clone(),
            retries_left: Arc::new(AtomicU32::new(0)),
            permanent: false,
        })
        .expect_err("duplicate");
    assert_eq!(duplicate.code(), "job.handler");
    assert_eq!(host.effective_concurrency(), 1);
    let worker = host.prepare().await.expect("prepare");
    assert!(worker.run_once().await.expect("run"));
    drop(worker);
    let stored = Queue::new(
        fexu_db::connect(&url).await.expect("sqlite"),
        Arc::new(SystemClock),
        JobPolicy::default(),
    );
    assert_eq!(
        stored.get(id).await.expect("job").status,
        JobStatus::Succeeded
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
    let mailed = enqueue_notice(
        &stored,
        &Notice::PasswordChanged {
            to: "person@example.com".into(),
            locale: "de".into(),
        },
        When::now(),
    )
    .await
    .expect("mail");
    let EnqueueOutcome::Created(mailed) = mailed else {
        panic!("mail created");
    };
    drop(stored);
    let host = WorkerHost::new(host_config(&url, 1)).with_mail(mail);
    let worker = host.prepare().await.expect("mail worker");
    assert!(worker.run_once().await.expect("deliver"));
    assert_eq!(inbox.messages().len(), 1);
    let delivered = Queue::new(
        fexu_db::connect(&url).await.expect("sqlite"),
        Arc::new(SystemClock),
        JobPolicy::default(),
    );
    assert_eq!(
        delivered.get(mailed).await.expect("mail job").status,
        JobStatus::Succeeded
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn describe_job_hides_payload_and_lease_token() {
    let job = JobRecord {
        id: uuid::Uuid::nil(),
        handler: "notes_count".into(),
        payload_version: 1,
        payload: r#"{"to":"person@example.com"}"#.into(),
        status: JobStatus::Retry,
        created_at: 10,
        available_at: 11,
        attempts: 2,
        max_attempts: 5,
        next_attempt_at: Some(12),
        lease_owner: Some("worker".into()),
        lease_token: Some("secret-lease".into()),
        lease_expires_at: Some(13),
        error_code: Some("notes.temporary".into()),
        finished_at: None,
        idempotency_key: None,
        interval_secs: None,
        schedule_key: None,
        cron_expression: None,
        cron_zone: None,
    };
    let hidden = describe_job(&job, false);
    assert!(hidden.contains("status retry"));
    assert!(hidden.contains("handler notes_count"));
    assert!(hidden.contains("version 1"));
    assert!(hidden.contains("attempts 2/5"));
    assert!(hidden.contains("error_code notes.temporary"));
    assert!(hidden.contains("[redigiert"));
    assert!(!hidden.contains("person@example.com"));
    assert!(!hidden.contains("secret-lease"));
    let shown = describe_job(&job, true);
    assert!(shown.contains("person@example.com"));
    assert!(!shown.contains("secret-lease"));
}

fn host_config(url: &str, concurrency: u16) -> WorkerConfig {
    WorkerConfig {
        database_url: url.to_owned(),
        enabled: true,
        poll_ms: 50,
        concurrency,
        policy: JobPolicy::default(),
    }
}

async fn created(queue: &Queue, payload: &Label, when: When) -> uuid::Uuid {
    match queue
        .enqueue_json(CountJob::NAME, 1, payload, when)
        .await
        .expect("enqueue")
    {
        EnqueueOutcome::Created(id) | EnqueueOutcome::Existing(id) => id,
    }
}

#[tokio::test]
async fn crash_retry_and_two_workers_do_not_claim_the_same_job() {
    let (queue, clock, path) = started().await;
    let id = created(
        &queue,
        &Label {
            label: "once".into(),
        },
        When::now(),
    )
    .await;
    let first = queue
        .claim("worker-a", &[CountJob::NAME], 30)
        .await
        .expect("claim");
    let second = queue
        .claim("worker-b", &[CountJob::NAME], 30)
        .await
        .expect("busy");
    assert!(first.is_some());
    assert!(second.is_none());
    let (record, token) = first.expect("lease");
    clock.advance(31);
    let recovered = queue
        .claim("worker-c", &[CountJob::NAME], 30)
        .await
        .expect("recover")
        .expect("expired lease");
    assert_eq!(recovered.0.attempts, 2);
    assert_ne!(recovered.1, token);
    assert_eq!(
        queue
            .finish_success(id, &token, &record)
            .await
            .expect("stale"),
        Finish::LostLease
    );
    assert_eq!(
        queue
            .finish_success(id, &recovered.1, &recovered.0)
            .await
            .expect("owner"),
        Finish::Stored
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn stop_flag_leaves_the_following_job_pending() {
    let (queue, _clock, path) = started().await;
    let _first = created(
        &queue,
        &Label {
            label: "stop".into(),
        },
        When::now(),
    )
    .await;
    let second = created(
        &queue,
        &Label {
            label: "later".into(),
        },
        When::now(),
    )
    .await;
    let stop = crate::stop_flag();
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut registry = JobRegistry::new();
    registry
        .register(CountJob {
            hits: Arc::new(AtomicU32::new(0)),
            retries_left: Arc::new(AtomicU32::new(0)),
            permanent: false,
        })
        .expect("register");
    let worker = Worker::new(queue.clone(), registry, 60, 20, 1);
    worker.run(stop.as_ref()).await.expect("drain");
    assert_eq!(
        queue.get(second).await.expect("pending").status,
        JobStatus::Pending
    );
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn rollback_removes_the_job_and_failure_is_inspectable() {
    let (queue, clock, path) = started().await;
    let transaction = queue.database().begin().await.expect("begin");
    let created = queue
        .enqueue_json_on(
            &transaction,
            CountJob::NAME,
            1,
            &Label {
                label: "rolled".into(),
            },
            When::now(),
        )
        .await
        .expect("staged");
    let EnqueueOutcome::Created(id) = created else {
        panic!("created");
    };
    transaction.rollback().await.expect("rollback");
    assert_eq!(
        queue.get(id).await.expect_err("gone").code(),
        "job.not_found"
    );
    let failed = queue
        .enqueue_json(
            BoomJob::NAME,
            1,
            &Label {
                label: "boom".into(),
            },
            When {
                max_attempts: Some(1),
                ..When::now()
            },
        )
        .await
        .expect("boom");
    let EnqueueOutcome::Created(failed) = failed else {
        panic!("boom");
    };
    clock.advance(1);
    let mut registry = JobRegistry::new();
    registry.register(BoomJob).expect("boom");
    let worker = Worker::new(queue.clone(), registry, 60, 20, 1);
    assert!(worker.run_once().await.expect("fail"));
    let dead = queue.dead_letters(10).await.expect("dead");
    assert!(dead
        .iter()
        .any(|job| job.id == failed && job.error_code.as_deref() == Some("notes.boom")));
    assert_eq!(crate::DELIVERY, "at-least-once");
    let when = When::cron("weekday-morning", "0 9 * * 1-5", "utc").expect("cron");
    let scheduled = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "morning".into(),
            },
            when,
        )
        .await
        .expect("schedule");
    let EnqueueOutcome::Created(scheduled) = scheduled else {
        panic!("schedule");
    };
    let job = queue.get(scheduled).await.expect("cron job");
    assert!(job.available_at > clock.unix().expect("now"));
    assert_eq!(job.cron_zone.as_deref(), Some("utc"));
    let _ = std::fs::remove_file(path);
}

struct BoomJob;

#[async_trait]
impl JobHandler for BoomJob {
    const NAME: &'static str = "notes_boom";
    const VERSION: u32 = 1;
    type Payload = Label;

    async fn run(&self, _payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        Err(JobError::permanent("notes.boom"))
    }
}

#[tokio::test]
async fn parallel_claims_and_idempotent_enqueue_keep_one_job() {
    let (queue, _clock, path) = started().await;
    let id = created(
        &queue,
        &Label {
            label: "shared".into(),
        },
        When::now(),
    )
    .await;
    let hits = Arc::new(AtomicU32::new(0));
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let queue = queue.clone();
        let hits = hits.clone();
        tasks.push(tokio::spawn(async move {
            worker(queue, registry(hits, Arc::new(AtomicU32::new(0)), false))
                .run_once()
                .await
        }));
    }
    let mut ran = 0;
    for task in tasks {
        if task.await.expect("join").expect("worker") {
            ran += 1;
        }
    }
    assert_eq!(ran, 1);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    let stored = queue.get(id).await.expect("finished");
    assert_eq!(stored.status, JobStatus::Succeeded);
    assert_eq!(stored.attempts, 1);

    let mut tasks = Vec::new();
    for _ in 0..12 {
        let queue = queue.clone();
        tasks.push(tokio::spawn(async move {
            queue
                .enqueue_json(
                    CountJob::NAME,
                    1,
                    &Label {
                        label: "same".into(),
                    },
                    When {
                        idempotency_key: Some("parallel-once".into()),
                        ..When::now()
                    },
                )
                .await
        }));
    }
    let mut created_count = 0;
    let mut ids = std::collections::HashSet::new();
    for task in tasks {
        match task.await.expect("join").expect("enqueue") {
            EnqueueOutcome::Created(id) => {
                created_count += 1;
                ids.insert(id);
            }
            EnqueueOutcome::Existing(id) => {
                ids.insert(id);
            }
        }
    }
    assert_eq!(created_count, 1);
    assert_eq!(ids.len(), 1);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn schedule_errors_and_long_keys_leave_a_consistent_queue() {
    let (queue, _clock, path) = started().await;
    assert!(When::cron("bad", "61 * * * *", "utc").is_err());
    assert!(When::cron("bad", "0 9 * * *", "Mars/Base").is_err());
    let mixed = When {
        interval_secs: Some(60),
        schedule_key: Some("both".into()),
        cron_expression: Some("0 9 * * *".into()),
        cron_zone: Some("utc".into()),
        ..When::now()
    };
    assert_eq!(
        queue
            .enqueue_json(
                CountJob::NAME,
                1,
                &Label {
                    label: "mixed".into(),
                },
                mixed,
            )
            .await
            .expect_err("interval and cron")
            .code(),
        "job.schedule"
    );
    assert!(queue.list(10).await.expect("empty").is_empty());
    let long_key = "k".repeat(70);
    let scheduled = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "long".into(),
            },
            When::every(&long_key, 60),
        )
        .await
        .expect("long schedule key");
    assert!(matches!(scheduled, EnqueueOutcome::Created(_)));
    assert!(queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "too-long".into(),
            },
            When::every("k".repeat(81), 60),
        )
        .await
        .is_err());
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a running PostgreSQL database"]
async fn postgres_job_claim_and_rollback() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let db = fexu_db::connect(&url).await.expect("postgres");
    apply_migrations(&db).await.expect("jobs");
    let clock = std::sync::Arc::new(ManualClock::new(1_700_000_000));
    let queue = Queue::new(db, clock.clone(), JobPolicy::default());
    let transaction = queue.database().begin().await.expect("begin");
    let staged = queue
        .enqueue_json_on(
            &transaction,
            CountJob::NAME,
            1,
            &Label { label: "pg".into() },
            When::now(),
        )
        .await
        .expect("stage");
    let EnqueueOutcome::Created(id) = staged else {
        panic!("created");
    };
    transaction.rollback().await.expect("rollback");
    assert!(queue.get(id).await.is_err());
    let kept = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "kept".into(),
            },
            When::now(),
        )
        .await
        .expect("keep");
    let EnqueueOutcome::Created(kept) = kept else {
        panic!("kept");
    };
    let claimed = queue
        .claim("pg-worker", &[CountJob::NAME], 30)
        .await
        .expect("claim")
        .expect("row");
    assert!(queue.cancel(kept).await.is_err());
    queue
        .finish_success(kept, &claimed.1, &claimed.0)
        .await
        .expect("finish");
    assert!(table_ready(queue.database()).await.expect("ready"));
    let morning = When::cron("pg-morning", "0 9 * * 1-5", "europe/berlin").expect("cron");
    let scheduled = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "morning".into(),
            },
            morning,
        )
        .await
        .expect("cron");
    let EnqueueOutcome::Created(scheduled) = scheduled else {
        panic!("cron created");
    };
    let row = queue.get(scheduled).await.expect("cron row");
    assert_eq!(row.cron_zone.as_deref(), Some("europe/berlin"));
    assert!(row.available_at > clock.unix().expect("now"));
    assert!(When::cron("pg-morning", "0 9 * * 1-5", "America/New_York").is_err());
    let again = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "morning".into(),
            },
            When::cron("pg-morning", "0 9 * * 1-5", "utc").expect("utc"),
        )
        .await
        .expect("existing schedule");
    assert!(matches!(again, EnqueueOutcome::Existing(_)));
    let once = queue
        .enqueue_json(
            BoomJob::NAME,
            1,
            &Label {
                label: "boom".into(),
            },
            When {
                max_attempts: Some(1),
                idempotency_key: Some("pg-boom-once".into()),
                ..When::now()
            },
        )
        .await
        .expect("boom");
    let EnqueueOutcome::Created(once) = once else {
        panic!("boom created");
    };
    let duplicate = queue
        .enqueue_json(
            BoomJob::NAME,
            1,
            &Label {
                label: "boom".into(),
            },
            When {
                idempotency_key: Some("pg-boom-once".into()),
                ..When::now()
            },
        )
        .await
        .expect("duplicate");
    assert_eq!(duplicate, EnqueueOutcome::Existing(once));
    let claimed = queue
        .claim("pg-boom", &[BoomJob::NAME], 30)
        .await
        .expect("claim boom")
        .expect("boom row");
    queue
        .finish_retry(
            once,
            &claimed.1,
            claimed.0.attempts,
            claimed.0.max_attempts,
            "notes.boom",
        )
        .await
        .expect("dead letter");
    let dead = queue.dead_letters(10).await.expect("dead");
    assert!(dead
        .iter()
        .any(|job| job.id == once && job.status == JobStatus::Failed));
    let waiting = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "retry".into(),
            },
            When::now(),
        )
        .await
        .expect("retry job");
    let EnqueueOutcome::Created(waiting) = waiting else {
        panic!("retry created");
    };
    let claimed = queue
        .claim("pg-retry", &[CountJob::NAME], 30)
        .await
        .expect("claim retry")
        .expect("retry row");
    let before = clock.unix().expect("now");
    queue
        .finish_retry(
            waiting,
            &claimed.1,
            claimed.0.attempts,
            claimed.0.max_attempts,
            "notes.temporary",
        )
        .await
        .expect("backoff");
    let retried = queue.get(waiting).await.expect("retried");
    assert_eq!(retried.status, JobStatus::Retry);
    assert!(retried.available_at > before || retried.next_attempt_at.unwrap_or(0) > before);
    let parked = queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "parked".into(),
            },
            When::now(),
        )
        .await
        .expect("parked");
    let EnqueueOutcome::Created(parked) = parked else {
        panic!("parked");
    };
    let stop = crate::stop_flag();
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut registry = JobRegistry::new();
    registry
        .register(CountJob {
            hits: Arc::new(AtomicU32::new(0)),
            retries_left: Arc::new(AtomicU32::new(0)),
            permanent: false,
        })
        .expect("register");
    Worker::new(queue.clone(), registry, 60, 20, 1)
        .run(stop.as_ref())
        .await
        .expect("stopped");
    assert_eq!(
        queue.get(parked).await.expect("still pending").status,
        JobStatus::Pending
    );
    let first = queue.database().begin().await.expect("first");
    queue
        .enqueue_json_on(
            &first,
            CountJob::NAME,
            1,
            &Label {
                label: "race".into(),
            },
            When {
                idempotency_key: Some("pg-race".into()),
                ..When::now()
            },
        )
        .await
        .expect("race insert");
    let second = queue.database().begin().await.expect("second");
    let racing = queue.clone();
    let pending = tokio::spawn(async move {
        let outcome = racing
            .enqueue_json_on(
                &second,
                CountJob::NAME,
                1,
                &Label {
                    label: "race".into(),
                },
                When {
                    idempotency_key: Some("pg-race".into()),
                    ..When::now()
                },
            )
            .await;
        let follow = racing
            .enqueue_json_on(
                &second,
                CountJob::NAME,
                1,
                &Label {
                    label: "follow".into(),
                },
                When {
                    idempotency_key: Some("pg-race-follow".into()),
                    ..When::now()
                },
            )
            .await;
        let committed = second.commit().await;
        (outcome, follow, committed)
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    first.commit().await.expect("commit race");
    let (outcome, follow, committed) = pending.await.expect("join race");
    match outcome.expect("conflict stays inside the savepoint") {
        EnqueueOutcome::Existing(_) => {}
        EnqueueOutcome::Created(_) => panic!("duplicate job became visible"),
    }
    assert!(matches!(
        follow.expect("same transaction stays usable"),
        EnqueueOutcome::Created(_)
    ));
    committed.expect("commit follow");
    let mut parallel_ids = Vec::new();
    for index in 0..6 {
        let created = queue
            .enqueue_json(
                "notes_parallel",
                1,
                &Label {
                    label: format!("parallel-{index}"),
                },
                When {
                    idempotency_key: Some(format!("pg-parallel-{index}")),
                    ..When::now()
                },
            )
            .await
            .expect("parallel job");
        let EnqueueOutcome::Created(id) = created else {
            panic!("parallel created");
        };
        parallel_ids.push(id);
    }
    let mut workers = Vec::new();
    for worker_index in 0..8 {
        let queue = queue.clone();
        workers.push(tokio::spawn(async move {
            let mut claimed = Vec::new();
            loop {
                match queue
                    .claim(
                        &format!("pg-worker-{worker_index}"),
                        &["notes_parallel"],
                        60,
                    )
                    .await
                {
                    Ok(Some((record, token))) => {
                        queue
                            .finish_success(record.id, &token, &record)
                            .await
                            .expect("finish");
                        claimed.push(record.id);
                    }
                    Ok(None) => break,
                    Err(error) => panic!("claim failed: {}", error.code()),
                }
            }
            claimed
        }));
    }
    let mut seen = std::collections::HashSet::new();
    for worker in workers {
        for id in worker.await.expect("worker join") {
            assert!(seen.insert(id), "two workers finished the same job");
        }
    }
    assert_eq!(seen.len(), parallel_ids.len());
    for id in parallel_ids {
        assert!(seen.contains(&id));
        assert_eq!(
            queue.get(id).await.expect("claimed").status,
            JobStatus::Succeeded
        );
        assert_eq!(queue.get(id).await.expect("claimed").attempts, 1);
    }
    queue
        .enqueue_json(
            CountJob::NAME,
            1,
            &Label {
                label: "after-abort".into(),
            },
            When {
                idempotency_key: Some("pg-after-abort".into()),
                ..When::now()
            },
        )
        .await
        .expect("connection recovered");
    clock.advance(8 * 24 * 60 * 60);
    let removed = queue.retain().await.expect("retain");
    assert!(
        removed >= 1,
        "retention must delete terminal PostgreSQL rows"
    );
    assert!(queue.get(kept).await.is_err());
    assert_eq!(
        queue.get(parked).await.expect("pending stays").status,
        JobStatus::Pending
    );
}
