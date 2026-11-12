//! Authentifizierung, Sitzungen und Berechtigungen für Ferrioxid.
//!
//! Die Crate hängt nicht vom Admin-Frontend ab. Schemaänderungen laufen über die Migrations-Engine.

mod anonymous;
mod config;
mod cookie;
mod error;
mod password;
mod policy;
mod recovery;
mod routes;
mod schema;
mod service;
mod store;

#[cfg(test)]
mod anonymous_tests;
#[cfg(test)]
mod reset_tests;
#[cfg(test)]
mod sqlite_tests;

pub use anonymous::{AnonymousSession, BrowserActor};
pub use config::{AuthConfig, CookieSameSite, Environment};
pub use error::AuthError;
pub use password::{hash_password, validate_new_password, verify_password};
pub use policy::ObjectPolicy;
pub use recovery::{
    complete_reset, exchange_token, request_reset, CompleteError, IssuedExchange,
    ResetAcknowledgement,
};
pub use routes::{mount, AppState, ClientIp};
pub use schema::{
    apply_migrations, embedded_migrations, migration_file, migration_status, migrations_dir,
    schema, ADMIN_AUDIT, ADMIN_DRAFTS,
};
pub use service::{
    csrf_header_name, normalize_email, AuthService, AuthUser, IssuedSession, PublicUser,
};
