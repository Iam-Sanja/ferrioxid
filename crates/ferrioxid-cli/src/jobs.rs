//! Job commands. Read-only operations open SQLite using `mode=ro`.

use ferrioxid_jobs::{
    describe_job, render_migration_status, JobPolicy, Queue, SystemClock, WorkerConfig, WorkerHost,
};
use ferrioxid_mail::{MailConfig, MailService};
use ferrioxid_migrations::{readonly_database_url, ProjectConfig};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use uuid::Uuid;

pub fn confirm(yes: bool) -> Result<(), String> {
    if yes {
        Ok(())
    } else {
        Err(
            "job.confirm This command modifies data and requires --yes. No changes were made."
                .to_owned(),
        )
    }
}

pub fn worker(
    url: String,
    project: Option<&ProjectConfig>,
    concurrency: Option<u16>,
    poll_ms: Option<u64>,
) -> ExitCode {
    let mut config = match project {
        Some(project) => WorkerConfig::from_project(project),
        None => WorkerConfig {
            database_url: url.clone(),
            enabled: true,
            poll_ms: 1000,
            concurrency: 1,
            policy: JobPolicy::default(),
        },
    };
    config.database_url = url;
    if let Some(value) = poll_ms {
        config.poll_ms = value;
    }
    if let Some(value) = concurrency {
        config.concurrency = value;
    }
    if !config.enabled {
        eprintln!("job.disabled jobs.enabled is false. Neither ferrioxid dev nor the web server starts a worker.");
        return ExitCode::from(2);
    }
    if config.concurrency > 1 && config.database_url.starts_with("sqlite") {
        eprintln!("job.sqlite.concurrency SQLite uses one connection. Concurrency remains 1.");
        config.concurrency = 1;
    }
    println!("job.handlers.builtin The CLI worker runs only the built-in mail handler. Start app handlers with src/bin/worker.rs.");
    let host = WorkerHost::new(config).with_mail(mail_from(project));
    match host.run_until_ctrl_c() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{} {}", error.code(), error.detail());
            if error.code().starts_with("job.migration")
                || error.code() == "job.disabled"
                || error.code() == "job.database"
            {
                ExitCode::from(2)
            } else {
                ExitCode::from(1)
            }
        }
    }
}

pub fn list(url: String) -> ExitCode {
    read(url, |queue| async move {
        let rows = queue.list(50).await.map_err(show)?;
        if rows.is_empty() {
            println!("Jobs: none");
        }
        for row in rows {
            println!(
                "{} {} {} {}/{} verfuegbar {} {}",
                row.id,
                row.handler,
                row.status.as_str(),
                row.attempts,
                row.max_attempts,
                row.available_at,
                row.error_code.unwrap_or_default()
            );
        }
        Ok(())
    })
}

pub fn inspect(url: String, id: String, show_payload: bool) -> ExitCode {
    read(url, |queue| async move {
        let id = parse_id(&id)?;
        let job = queue.get(id).await.map_err(show)?;
        println!("{}", describe_job(&job, show_payload));
        if show_payload {
            eprintln!("job.payload.visible The requested payload may contain personal data.");
        }
        Ok(())
    })
}

pub fn retry(url: String, id: String, yes: bool) -> ExitCode {
    mutate(url, yes, |queue| async move {
        let id = parse_id(&id)?;
        queue.retry(id).await.map_err(show)?;
        println!("Job {id} has been scheduled for retry.");
        Ok(())
    })
}

pub fn cancel(url: String, id: String, yes: bool) -> ExitCode {
    mutate(url, yes, |queue| async move {
        let id = parse_id(&id)?;
        queue.cancel(id).await.map_err(show)?;
        println!("Job {id} has been cancelled.");
        Ok(())
    })
}

pub async fn print_status(db: &sea_orm::DatabaseConnection) -> Result<bool, String> {
    let (text, current) = render_migration_status(db)
        .await
        .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
    print!("{text}");
    Ok(current)
}

fn read<F, T>(url: String, action: F) -> ExitCode
where
    F: FnOnce(Queue) -> T,
    T: std::future::Future<Output = Result<(), String>>,
{
    let readonly = match readonly_database_url(&url) {
        Ok(url) => url,
        Err(error) => {
            eprintln!("{}", error.0);
            return ExitCode::from(2);
        }
    };
    finish(readonly, false, action)
}

fn mutate<F, T>(url: String, yes: bool, action: F) -> ExitCode
where
    F: FnOnce(Queue) -> T,
    T: std::future::Future<Output = Result<(), String>>,
{
    if let Err(error) = confirm(yes) {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    if let Err(error) = existing_sqlite(&url) {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    finish(url, true, action)
}

fn finish<F, T>(url: String, writable: bool, action: F) -> ExitCode
where
    F: FnOnce(Queue) -> T,
    T: std::future::Future<Output = Result<(), String>>,
{
    let _ = writable;
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("job.database {error}");
            return ExitCode::from(1);
        }
    };
    let result = runtime.block_on(async {
        let db = ferrioxid_db::connect(&url)
            .await
            .map_err(|error| error.to_string())?;
        let queue = Queue::new(db, Arc::new(SystemClock), JobPolicy::default());
        action(queue).await
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            if error.starts_with("job.migration") || error.starts_with("job.not_found") {
                ExitCode::from(2)
            } else {
                ExitCode::from(1)
            }
        }
    }
}

fn mail_from(project: Option<&ProjectConfig>) -> MailService {
    let Some(project) = project else {
        return MailService::disabled();
    };
    MailService::open(&MailConfig {
        backend: project.mail_backend.clone(),
        host: project.mail_host.clone(),
        port: project.mail_port,
        security: project.mail_security.clone(),
        from: project.mail_from.clone(),
        reply_to: project.mail_reply_to.clone(),
        timeout_secs: project.mail_timeout_secs,
        public_origin: project.mail_public_origin.clone(),
        reset_ttl_minutes: project.mail_reset_ttl_minutes,
    })
}

fn existing_sqlite(url: &str) -> Result<(), String> {
    let Some(path) = sqlite_file(url) else {
        return Ok(());
    };
    if Path::new(path).is_file() {
        Ok(())
    } else {
        Err("job.database The SQLite database file is missing. No file will be created.".to_owned())
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

fn parse_id(value: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|_| "job.not_found The job ID is not a valid UUID.".to_owned())
}

fn show(error: ferrioxid_jobs::JobError) -> String {
    format!("{} {}", error.code(), error.detail())
}

#[cfg(test)]
mod tests {
    use super::confirm;

    #[test]
    fn mutation_without_yes_is_refused() {
        let error = confirm(false).expect_err("refused");
        assert!(error.contains("job.confirm"));
        assert!(confirm(true).is_ok());
    }
}
