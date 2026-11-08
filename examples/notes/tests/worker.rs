use ferrum::apps::FerrumApplication;
use ferrum::jobs::{enqueue_notice, JobPolicy, JobStatus, Queue, SystemClock};
use ferrum::mail::Notice;
use notes::jobs::{NotePulse, NoteRetryHandler};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn worker_binary_starts_without_a_web_server() {
    let source = include_str!("../src/bin/worker.rs");
    assert!(source.contains("WorkerHost"));
    assert!(source.contains("register_jobs"));
    assert!(!source.contains("into_router"));
    assert!(!source.contains("apply_migrations"));
}

#[test]
fn duplicate_app_handlers_fail() {
    let mut registry = ferrum::jobs::JobRegistry::new();
    notes::NotesApp.register_jobs(&mut registry).expect("once");
    let error = notes::NotesApp
        .register_jobs(&mut registry)
        .expect_err("duplicate");
    assert_eq!(error.code, "job.handler");
}

#[tokio::test]
async fn notes_worker_binary_runs_restarts_and_leaves_unknown_jobs() {
    let root = temp_dir("ferrum-notes-worker");
    let path = root.join("jobs.sqlite");
    let url = sqlite_url(&path);
    std::fs::write(
        root.join("ferrum.toml"),
        format!(
            "migrations_dir = \"migrations\"\n[database]\nurl = \"{url}\"\n[jobs]\npoll_ms = 50\njitter = false\n[mail]\nbackend = \"memory\"\nfrom = \"noreply@example.com\"\npublic_origin = \"http://127.0.0.1:3000\"\n"
        ),
    )
    .expect("toml");

    let db = ferrum::db::connect(&url).await.expect("sqlite");
    drop(db);
    let child = spawn(&root, &url);
    let (status, stderr) = wait_exit(child);
    assert_eq!(status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("job.migration.missing"), "{stderr}");
    let db = ferrum::db::connect(&url).await.expect("sqlite");
    assert!(!ferrum::jobs::table_ready(&db).await.expect("ready"));
    ferrum::jobs::apply_migrations(&db)
        .await
        .expect("migration");
    drop(db);

    let pulse = enqueue_pulse(&url, "bereit").await;
    let mut running = spawn(&root, &url);
    wait_for(&url, pulse, JobStatus::Succeeded, &mut running).await;
    stop(&mut running);

    let retry = enqueue_retry(&url).await;
    let mut running = spawn(&root, &url);
    wait_for(&url, retry, JobStatus::Retry, &mut running).await;
    stop(&mut running);
    let mut running = spawn(&root, &url);
    wait_for(&url, retry, JobStatus::Succeeded, &mut running).await;
    stop(&mut running);

    let unknown = enqueue_unknown(&url).await;
    let mut running = spawn(&root, &url);
    tokio::time::sleep(Duration::from_millis(400)).await;
    stop(&mut running);
    let job = fetch(&url, unknown).await;
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.attempts, 0);

    let mailed = enqueue_mail(&url).await;
    let mut running = spawn(&root, &url);
    wait_for(&url, mailed, JobStatus::Succeeded, &mut running).await;
    stop(&mut running);
    let _ = std::fs::remove_dir_all(root);
}

fn temp_dir(prefix: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        ferrum::uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&path).expect("dir");
    path
}

fn sqlite_url(path: &Path) -> String {
    format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    )
}

fn spawn(root: &Path, url: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_worker"))
        .env("FERRUM_PROJECT", root)
        .env("DATABASE_URL", url)
        .env_remove("FERRUM_SMTP_PASSWORD")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("worker")
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_exit(mut child: Child) -> (ExitStatus, String) {
    let mut status = None;
    for _ in 0..100 {
        if let Some(finished) = child.try_wait().expect("wait") {
            status = Some(finished);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if status.is_none() {
        stop(&mut child);
    }
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        use std::io::Read;
        let _ = pipe.read_to_string(&mut stderr);
    }
    let Some(status) = status else {
        panic!("Der Worker ohne Migration ist nicht beendet. {stderr}");
    };
    (status, stderr)
}

async fn enqueue_pulse(url: &str, label: &str) -> ferrum::uuid::Uuid {
    let queue = queue(url).await;
    let outcome = queue
        .enqueue::<notes::jobs::NotePulseHandler>(
            &NotePulse {
                label: label.to_owned(),
            },
            ferrum::jobs::When::now(),
        )
        .await
        .expect("pulse");
    let ferrum::jobs::EnqueueOutcome::Created(id) = outcome else {
        panic!("pulse created");
    };
    id
}

async fn enqueue_retry(url: &str) -> ferrum::uuid::Uuid {
    let queue = queue(url).await;
    let outcome = queue
        .enqueue::<NoteRetryHandler>(
            &NotePulse {
                label: "erneut".into(),
            },
            ferrum::jobs::When::now(),
        )
        .await
        .expect("retry");
    let ferrum::jobs::EnqueueOutcome::Created(id) = outcome else {
        panic!("retry created");
    };
    id
}

async fn enqueue_unknown(url: &str) -> ferrum::uuid::Uuid {
    let queue = queue(url).await;
    let outcome = queue
        .enqueue_json(
            "notes_missing",
            1,
            &NotePulse {
                label: "unbekannt".into(),
            },
            ferrum::jobs::When::now(),
        )
        .await
        .expect("unknown");
    let ferrum::jobs::EnqueueOutcome::Created(id) = outcome else {
        panic!("unknown created");
    };
    id
}

async fn enqueue_mail(url: &str) -> ferrum::uuid::Uuid {
    let queue = queue(url).await;
    let outcome = enqueue_notice(
        &queue,
        &Notice::PasswordChanged {
            to: "person@example.com".into(),
            locale: "de".into(),
        },
        ferrum::jobs::When::now(),
    )
    .await
    .expect("mail");
    let ferrum::jobs::EnqueueOutcome::Created(id) = outcome else {
        panic!("mail created");
    };
    id
}

async fn queue(url: &str) -> Queue {
    Queue::new(
        ferrum::db::connect(url).await.expect("sqlite"),
        Arc::new(SystemClock),
        JobPolicy::default(),
    )
}

async fn fetch(url: &str, id: ferrum::uuid::Uuid) -> ferrum::jobs::JobRecord {
    for _ in 0..20 {
        let queue = queue(url).await;
        match queue.get(id).await {
            Ok(job) => return job,
            Err(error) if error.code() == "job.locked" => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => panic!("{} {}", error.code(), error.detail()),
        }
    }
    panic!("Die Job-Zeile blieb gesperrt.");
}

async fn wait_for(url: &str, id: ferrum::uuid::Uuid, status: JobStatus, child: &mut Child) {
    // CI machines can be busy running the rest of the workspace in parallel.
    // A deadline avoids turning slow worker startup into an arbitrary fixed
    // number of polls while keeping a bounded failure time.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    loop {
        let job = fetch(url, id).await;
        if job.status == status {
            return;
        }
        if let Some(exit) = child.try_wait().expect("check worker process") {
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                use std::io::Read;
                let _ = pipe.read_to_string(&mut stderr);
            }
            panic!(
                "Worker exited with {exit} before job {id} reached {}; status={} attempts={} next_attempt_at={:?} error={:?}; stderr: {stderr}",
                status.as_str(),
                job.status.as_str(),
                job.attempts,
                job.next_attempt_at,
                job.error_code,
            );
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "Job {id} remained {} instead of {} after 45s; attempts={} next_attempt_at={:?} error={:?}",
                job.status.as_str(),
                status.as_str(),
                job.attempts,
                job.next_attempt_at,
                job.error_code,
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
