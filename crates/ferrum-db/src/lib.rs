//! Typed SeaORM-backed repository contract implemented by the model derive.

mod sql;

use ferrum_core::{FerrumError, FerrumModel};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use serde::{de::DeserializeOwned, Serialize};
pub use sql::{
    delete_record, find_by_column, get_record, insert_record, kind_of, list_records,
    query_admin_page, query_labels, render_add_column, render_add_column_referencing,
    render_create_index, render_create_table, render_drop_column, render_drop_index,
    render_drop_table, render_rename_column, render_rename_table, render_schema,
    render_table_bundle, update_record, AdminPage, AdminPageRequest, ForeignKeyClause,
    PhysicalColumn, ValuePage,
};

#[derive(Debug, Serialize)]
pub struct ListPage<T> {
    pub data: Vec<T>,
    pub limit: usize,
    pub truncated: bool,
}

/// Drei Zustände eines PATCH-Feldes.
///
/// `Missing` entsteht nur über `#[serde(default)]`, wenn der Schlüssel fehlt.
/// Ein JSON-`null` wird zu `Null`. Ein Wert wird zu `Value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatchField<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<'de, T> serde::Deserialize<'de> for PatchField<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let present = Option::<T>::deserialize(deserializer)?;
        Ok(match present {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

/// Ergebnis von `DELETE`, ohne den gelöschten Datensatz erneut zu laden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    Missing,
}

#[async_trait::async_trait]
pub trait ModelRepository: FerrumModel + Serialize + DeserializeOwned {
    type Create: DeserializeOwned + Send;
    type Update: DeserializeOwned + Send;

    async fn list(db: &DatabaseConnection) -> Result<ListPage<Self>, FerrumError>;
    async fn get(db: &DatabaseConnection, id: uuid::Uuid) -> Result<Option<Self>, FerrumError>;
    async fn create(db: &DatabaseConnection, input: Self::Create) -> Result<Self, FerrumError>;
    async fn update(
        db: &DatabaseConnection,
        id: uuid::Uuid,
        input: Self::Update,
    ) -> Result<Self, FerrumError>;
    async fn delete(db: &DatabaseConnection, id: uuid::Uuid) -> Result<DeleteOutcome, FerrumError>;
    async fn list_scoped(
        db: &DatabaseConnection,
        scope: &ferrum_core::QueryScope,
    ) -> Result<ListPage<Self>, FerrumError>;
    async fn get_scoped(
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &ferrum_core::QueryScope,
    ) -> Result<Option<Self>, FerrumError>;
    async fn update_scoped(
        db: &DatabaseConnection,
        id: uuid::Uuid,
        input: Self::Update,
        scope: &ferrum_core::QueryScope,
    ) -> Result<Self, FerrumError>;
    async fn delete_scoped(
        db: &DatabaseConnection,
        id: uuid::Uuid,
        scope: &ferrum_core::QueryScope,
    ) -> Result<DeleteOutcome, FerrumError>;
    async fn get_tx(
        db: &DatabaseTransaction,
        id: uuid::Uuid,
        scope: &ferrum_core::QueryScope,
    ) -> Result<Option<Self>, FerrumError>;
    async fn create_tx(db: &DatabaseTransaction, input: Self::Create) -> Result<Self, FerrumError>;
    async fn update_tx(
        db: &DatabaseTransaction,
        id: uuid::Uuid,
        input: Self::Update,
        scope: &ferrum_core::QueryScope,
    ) -> Result<Self, FerrumError>;
    async fn delete_tx(
        db: &DatabaseTransaction,
        id: uuid::Uuid,
        scope: &ferrum_core::QueryScope,
    ) -> Result<DeleteOutcome, FerrumError>;
}

/// SQLite file in the process working directory. Created on first connect.
pub const DEFAULT_SQLITE_URL: &str = "sqlite://db.sqlite3?mode=rwc";

pub async fn connect(url: &str) -> Result<DatabaseConnection, sea_orm::DbErr> {
    let mut options = sea_orm::ConnectOptions::new(url.to_owned());
    if url.starts_with("sqlite:") {
        // One connection keeps in-memory databases and file locks predictable.
        options.max_connections(1);
        options.map_sqlx_sqlite_opts(|sqlite| sqlite.pragma("foreign_keys", "ON"));
    }
    sea_orm::Database::connect(options).await
}

#[cfg(test)]
mod tests {
    use super::PatchField;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Sample {
        #[serde(default)]
        name: PatchField<String>,
        #[serde(default)]
        active: PatchField<bool>,
        #[serde(default)]
        note: PatchField<String>,
    }

    #[test]
    fn patch_field_distinguishes_missing_null_and_value() {
        let missing: Sample = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.name, PatchField::Missing);
        assert_eq!(missing.active, PatchField::Missing);
        assert_eq!(missing.note, PatchField::Missing);

        let nulls: Sample =
            serde_json::from_str(r#"{"name":null,"active":null,"note":null}"#).unwrap();
        assert_eq!(nulls.name, PatchField::Null);
        assert_eq!(nulls.active, PatchField::Null);
        assert_eq!(nulls.note, PatchField::Null);

        let values: Sample =
            serde_json::from_str(r#"{"name":"Laptop","active":false,"note":"desk"}"#).unwrap();
        assert_eq!(values.name, PatchField::Value("Laptop".to_owned()));
        assert_eq!(values.active, PatchField::Value(false));
        assert_eq!(values.note, PatchField::Value("desk".to_owned()));
    }
}
