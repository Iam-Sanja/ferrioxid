//! Ferrioxid model-first framework (experimental v0.1).
extern crate self as ferrioxid;

pub use async_trait;
pub use chrono;
pub use ferrioxid_admin as admin;
pub use ferrioxid_api::{apps, pages, urls, views, web, App};
pub use ferrioxid_auth as auth;
pub use ferrioxid_core as core;
pub use ferrioxid_db as db;
pub use ferrioxid_forms as forms;
pub use ferrioxid_macros::{model_fields, FerrioxidChoice, FerrioxidModel};

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
            pub created_at: ::ferrioxid::chrono::DateTime<::ferrioxid::chrono::Utc>,
            #[field(managed = "updated")]
            pub updated_at: ::ferrioxid::chrono::DateTime<::ferrioxid::chrono::Utc>,
        }
    };
}
pub use ferrioxid_jobs as jobs;
pub use ferrioxid_mail as mail;
pub use ferrioxid_middleware as middleware;
pub use ferrioxid_migrations as migrations;

pub mod management {
    pub use ferrioxid_migrations::{
        is_command_name, ArgSpec, CommandError, CommandSpec, Invocation, ManagementCommand,
        Registry,
    };
}
pub use ferrioxid_static as static_files;
pub use ferrioxid_storage as storage;
pub use sea_orm;
pub use serde;
pub use serde_json;
pub use uuid;

/// Baut den Mail-Dienst aus `ferrioxid.toml`. Das SMTP-Passwort bleibt in der Umgebung.
pub fn mail_for(project: &ferrioxid_migrations::ProjectConfig) -> ferrioxid_mail::MailService {
    ferrioxid_mail::MailService::open(&ferrioxid_mail::MailConfig {
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
    pub use crate::{App, FerrioxidModel};
}
