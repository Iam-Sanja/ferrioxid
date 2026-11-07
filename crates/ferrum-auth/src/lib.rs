//! Authentifizierung, Sitzungen und Berechtigungen für Ferrum.
//!
//! Die Crate hängt nicht vom Admin-Frontend ab. Schemaänderungen laufen über die Migrations-Engine.

mod config;
mod cookie;
mod error;
mod password;
mod policy;
mod routes;
mod schema;
mod service;
mod store;

#[cfg(test)]
mod sqlite_tests;

pub use config::{AuthConfig, CookieSameSite, Environment};
pub use error::AuthError;
pub use password::{hash_password, verify_password};
pub use policy::ObjectPolicy;
pub use routes::{mount, AppState, ClientIp};
pub use schema::{apply_migrations, migration_file, schema};
pub use service::{
    csrf_header_name, normalize_email, AuthService, AuthUser, IssuedSession, PublicUser,
};
