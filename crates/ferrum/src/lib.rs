//! Ferrum model-first framework (experimental v0.1).
extern crate self as ferrum;

pub use async_trait;
pub use ferrum_api::App;
pub use ferrum_core as core;
pub use ferrum_db as db;
pub use ferrum_macros::FerrumModel;
pub use ferrum_migrations as migrations;
pub use sea_orm;
pub use serde;
pub use serde_json;
pub use uuid;

pub mod prelude {
    pub use crate::uuid::Uuid;
    pub use crate::{App, FerrumModel};
}
