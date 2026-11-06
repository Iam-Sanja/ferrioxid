//! Tabelle `_ferrum_migrations`. Ein Eintrag entsteht nur nach erfolgreicher Transaktion.

use crate::error::MigrationError;
use ferrum_core::{ColumnKind, DatabaseKind};
use ferrum_db::{kind_of, render_create_table, PhysicalColumn};
use sea_orm::sea_query::{
    Alias, Order, PostgresQueryBuilder, Query, SimpleExpr, SqliteQueryBuilder,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, Value};

pub const HISTORY_TABLE: &str = "_ferrum_migrations";
pub const AUTH_HISTORY_TABLE: &str = "_ferrum_auth_migrations";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: String,
    pub name: String,
    pub checksum: String,
    pub applied_at_unix: i64,
}

pub async fn history_exists(db: &DatabaseConnection) -> Result<bool, MigrationError> {
    history_exists_in(db, HISTORY_TABLE).await
}

pub async fn history_exists_in(
    db: &DatabaseConnection,
    history_table: &str,
) -> Result<bool, MigrationError> {
    table_exists(db, history_table).await
}

pub async fn table_exists(db: &DatabaseConnection, table: &str) -> Result<bool, MigrationError> {
    if !crate::snapshot::is_identifier(table) {
        return Err(MigrationError::new(format!(
            "Ungültiger Tabellenname `{table}`."
        )));
    }
    let backend = db.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Sqlite => "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = ?",
        DatabaseBackend::Postgres => {
            "SELECT 1 AS present FROM information_schema.tables WHERE table_schema = 'public' AND table_name = $1"
        }
        DatabaseBackend::MySql => {
            return Err(MigrationError::new("MySQL wird nicht unterstützt."));
        }
    };
    let statement = Statement::from_sql_and_values(backend, sql, [Value::from(table)]);
    let row = db.query_one(statement).await.map_err(|error| {
        MigrationError::new(format!("Schemaprüfung ist fehlgeschlagen: {error}"))
    })?;
    Ok(row.is_some())
}

pub async fn ensure_history(db: &impl ConnectionTrait) -> Result<(), MigrationError> {
    ensure_history_in(db, HISTORY_TABLE).await
}

pub async fn ensure_history_in(
    db: &impl ConnectionTrait,
    history_table: &str,
) -> Result<(), MigrationError> {
    if !crate::snapshot::is_identifier(history_table) {
        return Err(MigrationError::new(format!(
            "Ungültiger Historienname `{history_table}`."
        )));
    }
    let backend = db.get_database_backend();
    let kind = kind_of(backend).map_err(|error| MigrationError::new(error.to_string()))?;
    let columns = [
        PhysicalColumn {
            name: "id",
            kind: ColumnKind::Text,
            nullable: false,
            primary_key: true,
            default_bool: None,
        },
        PhysicalColumn {
            name: "name",
            kind: ColumnKind::Text,
            nullable: false,
            primary_key: false,
            default_bool: None,
        },
        PhysicalColumn {
            name: "checksum",
            kind: ColumnKind::Text,
            nullable: false,
            primary_key: false,
            default_bool: None,
        },
        PhysicalColumn {
            name: "applied_at_unix",
            kind: ColumnKind::BigInt,
            nullable: false,
            primary_key: false,
            default_bool: None,
        },
    ];
    let sql = render_create_table(kind, history_table, &columns, true);
    db.execute(Statement::from_string(backend, sql))
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Migrationshistorie konnte nicht angelegt werden: {error}"
            ))
        })?;
    Ok(())
}

pub async fn read_history(db: &DatabaseConnection) -> Result<Vec<HistoryEntry>, MigrationError> {
    read_history_in(db, HISTORY_TABLE).await
}

pub async fn read_history_in(
    db: &DatabaseConnection,
    history_table: &str,
) -> Result<Vec<HistoryEntry>, MigrationError> {
    if !history_exists_in(db, history_table).await? {
        return Ok(Vec::new());
    }
    let backend = db.get_database_backend();
    let mut query = Query::select();
    query
        .columns([
            Alias::new("id"),
            Alias::new("name"),
            Alias::new("checksum"),
            Alias::new("applied_at_unix"),
        ])
        .from(Alias::new(history_table))
        .order_by(Alias::new("id"), Order::Asc);
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => return Err(MigrationError::new("MySQL wird nicht unterstützt.")),
    };
    let rows = db
        .query_all(Statement::from_sql_and_values(backend, sql, values.0))
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Migrationshistorie konnte nicht gelesen werden: {error}"
            ))
        })?;
    rows.iter()
        .map(|row| {
            Ok(HistoryEntry {
                id: row.try_get("", "id").map_err(db_error)?,
                name: row.try_get("", "name").map_err(db_error)?,
                checksum: row.try_get("", "checksum").map_err(db_error)?,
                applied_at_unix: row.try_get("", "applied_at_unix").map_err(db_error)?,
            })
        })
        .collect()
}

pub async fn insert_history(
    db: &impl ConnectionTrait,
    id: &str,
    name: &str,
    checksum: &str,
    applied_at_unix: i64,
) -> Result<(), MigrationError> {
    insert_history_in(db, HISTORY_TABLE, id, name, checksum, applied_at_unix).await
}

pub async fn insert_history_in(
    db: &impl ConnectionTrait,
    history_table: &str,
    id: &str,
    name: &str,
    checksum: &str,
    applied_at_unix: i64,
) -> Result<(), MigrationError> {
    let backend = db.get_database_backend();
    let mut query = Query::insert();
    let values: [SimpleExpr; 4] = [
        Value::from(id.to_owned()).into(),
        Value::from(name.to_owned()).into(),
        Value::from(checksum.to_owned()).into(),
        Value::from(applied_at_unix).into(),
    ];
    query
        .into_table(Alias::new(history_table))
        .columns([
            Alias::new("id"),
            Alias::new("name"),
            Alias::new("checksum"),
            Alias::new("applied_at_unix"),
        ])
        .values_panic(values);
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => return Err(MigrationError::new("MySQL wird nicht unterstützt.")),
    };
    db.execute(Statement::from_sql_and_values(backend, sql, values.0))
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Migrationshistorie konnte nicht geschrieben werden: {error}"
            ))
        })?;
    Ok(())
}

pub fn backend_name(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::Sqlite => "sqlite",
        DatabaseKind::Postgres => "postgres",
    }
}

fn db_error(error: sea_orm::DbErr) -> MigrationError {
    MigrationError::new(format!(
        "Migrationshistorie enthält eine unlesbare Zeile: {error}"
    ))
}
