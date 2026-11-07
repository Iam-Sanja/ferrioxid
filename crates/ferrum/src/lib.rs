//! Ferrum model-first framework (experimental v0.1).
extern crate self as ferrum;

pub use async_trait;
pub use chrono;
pub use ferrum_admin as admin;
pub use ferrum_api::{pages, urls, views, web, App};
pub use ferrum_auth as auth;
pub use ferrum_core as core;
pub use ferrum_db as db;
pub use ferrum_forms as forms;
pub use ferrum_macros::FerrumModel;
pub use ferrum_mail as mail;
pub use ferrum_middleware as middleware;
pub use ferrum_migrations as migrations;
pub use ferrum_static as static_files;
pub use ferrum_storage as storage;
pub use sea_orm;
pub use serde;
pub use serde_json;
pub use uuid;

/// Baut den Mail-Dienst aus `ferrum.toml`. Das SMTP-Passwort bleibt in der Umgebung.
pub fn mail_for(project: &ferrum_migrations::ProjectConfig) -> ferrum_mail::MailService {
    ferrum_mail::MailService::open(&ferrum_mail::MailConfig {
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

pub mod prelude {
    pub use crate::chrono::{DateTime, NaiveDate, NaiveTime, Utc};
    pub use crate::uuid::Uuid;
    pub use crate::{App, FerrumModel};
}
