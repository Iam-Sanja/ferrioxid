//! Applies embedded auth and job migrations, then exercises those public APIs.
//!
//! The binary contains the migration JSON. It does not read `CARGO_MANIFEST_DIR`.

use ferrioxid_auth::{AuthConfig, AuthService};
use ferrioxid_jobs::{JobPolicy, Queue, SystemClock, When};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("DATABASE_URL is required. No database was created.");
        std::process::exit(2);
    });
    let db = ferrioxid_db::connect(&url).await.unwrap_or_else(|error| {
        eprintln!("database connection failed: {error}");
        std::process::exit(1);
    });
    let auth_applied = ferrioxid_auth::apply_migrations(&db)
        .await
        .unwrap_or_else(|error| {
            eprintln!("auth migrations failed: {error}");
            std::process::exit(1);
        });
    let jobs_applied = ferrioxid_jobs::apply_migrations(&db)
        .await
        .unwrap_or_else(|error| {
            eprintln!("job migrations failed: {} {}", error.code(), error.detail());
            std::process::exit(1);
        });
    let auth = AuthService::new(db.clone(), AuthConfig::development());
    let password = String::from_utf8(vec![
        99, 111, 114, 114, 101, 99, 116, 45, 104, 111, 114, 115, 101, 45, 98, 97, 116, 116, 101,
        114, 121,
    ])
    .expect("fixture");
    auth.create_user("relocated@example.com", &password, true, false)
        .await
        .unwrap_or_else(|error| {
            eprintln!("create_user failed: {error}");
            std::process::exit(1);
        });
    let session = auth
        .login("relocated@example.com", &password, "127.0.0.1")
        .await
        .unwrap_or_else(|error| {
            eprintln!("login failed: {error}");
            std::process::exit(1);
        });
    if session.token.is_empty() {
        eprintln!("login returned an empty session");
        std::process::exit(1);
    }
    let queue = Queue::new(db, Arc::new(SystemClock), JobPolicy::default());
    let job = queue
        .enqueue_json(
            "embedded_probe",
            1,
            &serde_json::json!({"kind": "probe"}),
            When::now(),
        )
        .await
        .unwrap_or_else(|error| {
            eprintln!("enqueue failed: {} {}", error.code(), error.detail());
            std::process::exit(1);
        });
    println!(
        "embedded-migrate auth_applied={} jobs_applied={} login=ok job={job:?}",
        auth_applied.len(),
        jobs_applied.len()
    );
}
