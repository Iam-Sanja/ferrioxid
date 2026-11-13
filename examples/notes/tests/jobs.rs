use ferrioxid::jobs::{
    enqueue_notice, Clock, EnqueueOutcome, JobPolicy, JobRegistry, JobStatus, ManualClock, Queue,
    When, Worker,
};
use ferrioxid::mail::{MailService, Notice};
use notes::jobs::{register, NotePulse, NotePulseHandler};
use std::sync::Arc;

#[tokio::test]
async fn notes_runs_immediate_delayed_recurring_and_mail_jobs() {
    let path = std::env::temp_dir().join(format!(
        "ferrioxid-notes-jobs-{}-{}.sqlite",
        std::process::id(),
        ferrioxid::uuid::Uuid::new_v4()
    ));
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    );
    let db = ferrioxid::db::connect(&url).await.expect("sqlite");
    ferrioxid::jobs::apply_migrations(&db)
        .await
        .expect("migration");
    let clock = ManualClock::new(1_800_000_000);
    let queue = Queue::new(db, Arc::new(clock.clone()), JobPolicy::default());
    let handler = NotePulseHandler::new();
    let hits = handler.hits.clone();
    let mut registry = JobRegistry::new();
    register(&mut registry, handler).expect("register");
    let (mail, inbox) = MailService::memory("noreply@example.com", "http://127.0.0.1:3000");
    registry
        .register(ferrioxid::jobs::MailNoticeHandler::new(mail))
        .expect("mail");
    let runner = Worker::new(queue.clone(), registry, 60, 50, 1);

    let now = created(&queue, "sofort", When::now()).await;
    assert!(runner.run_once().await.expect("immediate"));
    assert_eq!(
        queue.get(now).await.expect("now").status,
        JobStatus::Succeeded
    );

    let start = clock.unix().expect("now");
    let later = created(&queue, "spaeter", When::at(start + 30)).await;
    assert!(!runner.run_once().await.expect("early"));
    clock.advance(30);
    assert!(runner.run_once().await.expect("delayed"));
    assert_eq!(
        queue.get(later).await.expect("later").status,
        JobStatus::Succeeded
    );

    let repeating = created(&queue, "wieder", When::every("notes-pulse", 60)).await;
    assert!(runner.run_once().await.expect("series"));
    assert_eq!(
        queue.get(repeating).await.expect("series").status,
        JobStatus::Succeeded
    );
    let pending = queue
        .list(20)
        .await
        .expect("list")
        .into_iter()
        .filter(|job| job.status == JobStatus::Pending)
        .count();
    assert_eq!(pending, 1);
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 3);

    let mailed = enqueue_notice(
        &queue,
        &Notice::AccountDeactivated {
            to: "person@example.com".into(),
            locale: "en".into(),
        },
        When::now(),
    )
    .await
    .expect("mail job");
    let EnqueueOutcome::Created(mailed) = mailed else {
        panic!("mail created");
    };
    assert!(runner.run_once().await.expect("mail"));
    assert_eq!(
        queue.get(mailed).await.expect("mail").status,
        JobStatus::Succeeded
    );
    assert_eq!(inbox.messages().len(), 1);
    assert!(!inbox.messages()[0].text.contains("token"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn notes_accepts_a_weekday_cron_and_a_process_local_cache() {
    let when = When::cron("notes-morning", "0 9 * * 1-5", "europe/berlin").expect("cron");
    assert_eq!(when.cron_zone.as_deref(), Some("europe/berlin"));
    assert!(When::cron("notes-morning", "0 9 * * 1-5", "America/New_York").is_err());
    let cache = ferrioxid::cache::TypedCache::new(
        ferrioxid::cache::MemoryCache::new(ferrioxid::cache::MemoryLimits {
            max_entries: 8,
            max_bytes: 4096,
        })
        .expect("limits"),
        1,
    );
    cache.set("notes", "home", &"ok", None).await.expect("set");
    let value: Option<String> = cache.get("notes", "home").await.expect("get");
    assert_eq!(value.as_deref(), Some("ok"));
    let other = ferrioxid::cache::TypedCache::new(
        ferrioxid::cache::MemoryCache::new(ferrioxid::cache::MemoryLimits {
            max_entries: 8,
            max_bytes: 4096,
        })
        .expect("limits"),
        1,
    );
    let missing: Option<String> = other.get("notes", "home").await.expect("other");
    assert!(missing.is_none());
}

async fn created(queue: &Queue, label: &str, when: When) -> ferrioxid::uuid::Uuid {
    match queue
        .enqueue::<NotePulseHandler>(
            &NotePulse {
                label: label.into(),
            },
            when,
        )
        .await
        .expect("enqueue")
    {
        EnqueueOutcome::Created(id) | EnqueueOutcome::Existing(id) => id,
    }
}
