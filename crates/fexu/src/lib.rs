//! Fexu model-first framework (experimental v0.1).
extern crate self as fexu;

pub use async_trait;
pub use chrono;
pub use fexu_admin as admin;
pub use fexu_api::{
    apps, openapi, pages, require_browser, serve, testing, urls, views, web, App, OpenApiOperation,
    OpenApiOptions,
};
pub use fexu_auth as auth;
pub use fexu_cache as cache;
pub use fexu_core as core;
pub use fexu_db as db;
pub use fexu_forms as forms;
pub use fexu_macros::{model_fields, FexuChoice, FexuForm, FexuModel};

#[macro_export]
macro_rules! with_timestamps {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($fields:tt)*
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $($fields)*
            #[field(managed = "created")]
            pub created_at: ::fexu::chrono::DateTime<::fexu::chrono::Utc>,
            #[field(managed = "updated")]
            pub updated_at: ::fexu::chrono::DateTime<::fexu::chrono::Utc>,
        }
    };
}
pub use fexu_jobs as jobs;
pub use fexu_mail as mail;
pub use fexu_middleware as middleware;
pub use fexu_migrations as migrations;

pub mod management {
    pub use fexu_migrations::{
        is_command_name, ArgSpec, CommandError, CommandSpec, Invocation, ManagementCommand,
        Registry,
    };
}
pub use fexu_db::driver as sea_orm;
pub use fexu_static as static_files;
pub use fexu_storage as storage;
pub use serde;
pub use serde_json;
pub use uuid;

/// Baut den Mail-Dienst aus `fexu.toml`. Das SMTP-Passwort bleibt in der Umgebung.
pub fn mail_for(project: &fexu_migrations::ProjectConfig) -> fexu_mail::MailService {
    fexu_mail::MailService::open(&fexu_mail::MailConfig {
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
    pub use crate::core::Decimal;
    pub use crate::uuid::Uuid;
    pub use crate::{App, FexuModel};
}
