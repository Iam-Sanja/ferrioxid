//! Kontrollierter Umbau. SQLite kopiert per `INSERT INTO ... SELECT`. PostgreSQL ändert die Spalte direkt.
//!
//! `PRAGMA foreign_keys` wird nicht verändert. Ein Umbau mit Fremdschlüsseln wird abgelehnt.
//! CREATE, INSERT, DROP und RENAME laufen in der Migrationstransaktion und werden bei Fehler zurückgerollt.

use crate::diff::{alter_is_supported, MigrationOperation};
use crate::error::MigrationError;
use crate::snapshot::{is_identifier, ColumnSnapshot, LogicalType, SchemaSnapshot, TableSnapshot};
use ferrioxid_core::DatabaseKind;
use ferrioxid_db::{render_create_table, PhysicalColumn};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Cell {
    Null,
    Text(String),
    Integer(i64),
}

pub async fn apply_alter(
    db: &impl ConnectionTrait,
    kind: DatabaseKind,
    before: &SchemaSnapshot,
    table: &str,
    from: &ColumnSnapshot,
    to: &ColumnSnapshot,
) -> Result<(), MigrationError> {
    if !alter_is_supported(from, to) {
        return Err(MigrationError::new(format!(
            "Änderung an `{table}.{}` von {:?} nach {:?} ist nicht unterstützt.",
            from.name, from.logical_type, to.logical_type
        )));
    }
    if matches!(kind, DatabaseKind::Sqlite) {
        assert_sqlite_rebuild_safe(db, table).await?;
    }
    let source = before
        .tables
        .iter()
        .find(|candidate| candidate.name == table)
        .ok_or_else(|| MigrationError::new(format!("Tabelle `{table}` fehlt vor dem Umbau.")))?;
    if !to.nullable && from.nullable {
        let nulls = count_where(db, table, &format!("\"{}\" IS NULL", from.name)).await?;
        if nulls > 0 {
            return Err(MigrationError::new(format!(
                "Spalte `{table}.{}` enthält {nulls} NULL-Werte. NOT NULL wird nicht gesetzt und kein Ersatzwert eingesetzt.",
                from.name
            )));
        }
    }
    match kind {
        DatabaseKind::Sqlite => {
            validate_changed_column(db, table, from, to).await?;
            rebuild_sqlite(db, source, from, to).await
        }
        DatabaseKind::Postgres => alter_postgres(db, table, from, to).await,
    }
}

pub fn alter_parts(
    operation: &MigrationOperation,
) -> Option<(&str, &ColumnSnapshot, &ColumnSnapshot)> {
    match operation {
        MigrationOperation::AlterColumn { table, from, to } => Some((table.as_str(), from, to)),
        _ => None,
    }
}

async fn rebuild_sqlite(
    db: &impl ConnectionTrait,
    source: &TableSnapshot,
    from: &ColumnSnapshot,
    to: &ColumnSnapshot,
) -> Result<(), MigrationError> {
    let target_columns = source
        .columns
        .iter()
        .map(|column| {
            if column.name == from.name {
                to.clone()
            } else {
                column.clone()
            }
        })
        .collect::<Vec<_>>();
    let temporary = format!("ferrioxid_rebuild_{}", source.name);
    if !is_identifier(&temporary) {
        return Err(MigrationError::new(format!(
            "Temporärer Tabellenname `{temporary}` ist ungültig."
        )));
    }
    let before = count_where(db, &source.name, "1 = 1").await?;
    let physical = physical_columns(&target_columns);
    execute(
        db,
        DatabaseBackend::Sqlite,
        &render_create_table(DatabaseKind::Sqlite, &temporary, &physical, false),
    )
    .await?;
    let selection = source
        .columns
        .iter()
        .map(|column| copy_expression(column, from, to))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    execute(
        db,
        DatabaseBackend::Sqlite,
        &format!(
            "INSERT INTO \"{temporary}\" ({}) SELECT {selection} FROM \"{}\"",
            quote_columns(&target_columns),
            source.name
        ),
    )
    .await?;
    let after = count_where(db, &temporary, "1 = 1").await?;
    if after != before {
        return Err(MigrationError::new(format!(
            "Nach dem Kopieren hat `{temporary}` {after} Zeilen, die Quelle hatte {before}."
        )));
    }
    execute(
        db,
        DatabaseBackend::Sqlite,
        &format!("DROP TABLE \"{}\"", source.name),
    )
    .await?;
    execute(
        db,
        DatabaseBackend::Sqlite,
        &format!("ALTER TABLE \"{temporary}\" RENAME TO \"{}\"", source.name),
    )
    .await?;
    let after_rename = count_where(db, &source.name, "1 = 1").await?;
    if after_rename != before {
        return Err(MigrationError::new(
            "Die Zeilenzahl nach dem Ersetzen der Tabelle stimmt nicht.",
        ));
    }
    let integrity = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA integrity_check",
        ))
        .await
        .map_err(|error| MigrationError::new(format!("integrity_check fehlgeschlagen: {error}")))?
        .ok_or_else(|| MigrationError::new("integrity_check lieferte keine Zeile."))?;
    let status: String = integrity.try_get("", "integrity_check").map_err(|error| {
        MigrationError::new(format!("integrity_check ist nicht lesbar: {error}"))
    })?;
    if status != "ok" {
        return Err(MigrationError::new(format!(
            "integrity_check meldet `{status}`. Die Transaktion wird zurückgerollt."
        )));
    }
    let foreign_keys = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_key_check",
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!("foreign_key_check fehlgeschlagen: {error}"))
        })?;
    if !foreign_keys.is_empty() {
        return Err(MigrationError::new(
            "foreign_key_check hat Verstöße gemeldet. Die Transaktion wird zurückgerollt.",
        ));
    }
    Ok(())
}

async fn alter_postgres(
    db: &impl ConnectionTrait,
    table: &str,
    from: &ColumnSnapshot,
    to: &ColumnSnapshot,
) -> Result<(), MigrationError> {
    if from.logical_type != to.logical_type {
        let sql_type = match to.logical_type {
            LogicalType::String | LogicalType::Uuid => "text",
            LogicalType::I64 => "bigint",
            LogicalType::Bool => "boolean",
            LogicalType::Date => "date",
            LogicalType::Time => "time",
            LogicalType::DateTime => "timestamptz",
        };
        execute(
            db,
            DatabaseBackend::Postgres,
            &format!(
                "ALTER TABLE \"{table}\" ALTER COLUMN \"{}\" TYPE {sql_type} USING \"{}\"::{sql_type}",
                from.name, from.name
            ),
        )
        .await?;
    }
    if from.default != to.default {
        let statement = match to.default {
            Some(true) => format!(
                "ALTER TABLE \"{table}\" ALTER COLUMN \"{}\" SET DEFAULT TRUE",
                to.name
            ),
            Some(false) => format!(
                "ALTER TABLE \"{table}\" ALTER COLUMN \"{}\" SET DEFAULT FALSE",
                to.name
            ),
            None => format!(
                "ALTER TABLE \"{table}\" ALTER COLUMN \"{}\" DROP DEFAULT",
                to.name
            ),
        };
        execute(db, DatabaseBackend::Postgres, &statement).await?;
    }
    if from.nullable != to.nullable {
        let verb = if to.nullable {
            "DROP NOT NULL"
        } else {
            "SET NOT NULL"
        };
        execute(
            db,
            DatabaseBackend::Postgres,
            &format!(
                "ALTER TABLE \"{table}\" ALTER COLUMN \"{}\" {verb}",
                to.name
            ),
        )
        .await?;
    }
    Ok(())
}

async fn assert_sqlite_rebuild_safe(
    db: &impl ConnectionTrait,
    table: &str,
) -> Result<(), MigrationError> {
    if !is_identifier(table) {
        return Err(MigrationError::new(format!(
            "Ungültiger Tabellenname `{table}`."
        )));
    }
    let attached = db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT type, name FROM sqlite_master WHERE tbl_name = ? AND sql IS NOT NULL AND type IN ('index', 'trigger', 'view')",
            [Value::from(table)],
        ))
        .await
        .map_err(|error| MigrationError::new(format!("Objektprüfung fehlgeschlagen: {error}")))?;
    if !attached.is_empty() {
        return Err(MigrationError::new(format!(
            "Tabelle `{table}` hat einen Index, Trigger oder eine View. Der Umbau würde sie verwerfen und wird blockiert."
        )));
    }
    assert_no_foreign_keys(db, table).await
}

async fn assert_no_foreign_keys(
    db: &impl ConnectionTrait,
    table: &str,
) -> Result<(), MigrationError> {
    let tables = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!("Fremdschlüsselprüfung fehlgeschlagen: {error}"))
        })?;
    for row in tables {
        let name: String = row.try_get("", "name").map_err(|error| {
            MigrationError::new(format!("Tabellenname ist nicht lesbar: {error}"))
        })?;
        if !is_identifier(&name) {
            continue;
        }
        let keys = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("PRAGMA foreign_key_list(\"{name}\")"),
            ))
            .await
            .map_err(|error| {
                MigrationError::new(format!("foreign_key_list fehlgeschlagen: {error}"))
            })?;
        if name == table && !keys.is_empty() {
            return Err(MigrationError::new(format!(
                "Tabelle `{table}` hat Fremdschlüssel. Ein Tabellenumbau wird deshalb blockiert."
            )));
        }
        for key in keys {
            let referenced: String = key.try_get("", "table").unwrap_or_default();
            if referenced == table {
                return Err(MigrationError::new(format!(
                    "Tabelle `{name}` verweist auf `{table}`. Ein Tabellenumbau wird deshalb blockiert."
                )));
            }
        }
    }
    Ok(())
}

async fn validate_changed_column(
    db: &impl ConnectionTrait,
    table: &str,
    from: &ColumnSnapshot,
    to: &ColumnSnapshot,
) -> Result<(), MigrationError> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT \"{}\" AS value FROM \"{table}\" ORDER BY rowid",
                from.name
            ),
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Lesen von `{table}.{}` fehlgeschlagen: {error}",
                from.name
            ))
        })?;
    let total = count_where(db, table, "1 = 1").await?;
    if rows.len() as i64 != total {
        return Err(MigrationError::new(
            "Die gelesene Zeilenzahl entspricht nicht COUNT(*). Der Umbau wurde nicht geschrieben."
                .to_owned(),
        ));
    }
    for (index, row) in rows.iter().enumerate() {
        let cell = read_cell(row, "value", from.logical_type)?;
        let converted =
            convert_cell(cell, from.logical_type, to.logical_type).map_err(|error| {
                MigrationError::new(format!(
                    "Zeile {index} in `{table}.{}` kann nicht übernommen werden: {error}",
                    from.name
                ))
            })?;
        if !to.nullable && matches!(converted, Cell::Null) {
            return Err(MigrationError::new(format!(
                "Zeile {index} in `{table}.{}` ist NULL.",
                from.name
            )));
        }
    }
    Ok(())
}

fn copy_expression(
    column: &ColumnSnapshot,
    from: &ColumnSnapshot,
    to: &ColumnSnapshot,
) -> Result<String, MigrationError> {
    if column.name != from.name || from.logical_type == to.logical_type {
        return Ok(format!("\"{}\"", column.name));
    }
    let sql_type = match to.logical_type {
        LogicalType::String | LogicalType::Date | LogicalType::Time | LogicalType::DateTime => {
            "TEXT"
        }
        LogicalType::I64 => "INTEGER",
        LogicalType::Uuid | LogicalType::Bool => {
            return Err(MigrationError::new(format!(
                "Änderung an `{}` von {:?} nach {:?} ist nicht unterstützt.",
                column.name, from.logical_type, to.logical_type
            )))
        }
    };
    Ok(format!("CAST(\"{}\" AS {sql_type})", column.name))
}

fn read_cell(
    row: &sea_orm::QueryResult,
    name: &str,
    logical: LogicalType,
) -> Result<Cell, MigrationError> {
    match logical {
        LogicalType::String
        | LogicalType::Uuid
        | LogicalType::Date
        | LogicalType::Time
        | LogicalType::DateTime => match row.try_get::<Option<String>>("", name) {
            Ok(Some(text)) => Ok(Cell::Text(text)),
            Ok(None) => Ok(Cell::Null),
            Err(error) => Err(MigrationError::new(format!(
                "Textwert in `{name}` ist nicht lesbar: {error}"
            ))),
        },
        LogicalType::I64 | LogicalType::Bool => {
            if let Ok(Some(number)) = row.try_get::<Option<i64>>("", name) {
                return Ok(Cell::Integer(number));
            }
            if let Ok(Some(number)) = row.try_get::<Option<i32>>("", name) {
                return Ok(Cell::Integer(i64::from(number)));
            }
            if let Ok(None) = row.try_get::<Option<i64>>("", name) {
                return Ok(Cell::Null);
            }
            Err(MigrationError::new(format!(
                "Zahlenwert in `{name}` ist nicht lesbar."
            )))
        }
    }
}

fn convert_cell(value: Cell, from: LogicalType, to: LogicalType) -> Result<Cell, String> {
    if matches!(value, Cell::Null) {
        return Ok(Cell::Null);
    }
    if from == to {
        return match (from, value) {
            (LogicalType::Bool, Cell::Integer(number @ (0 | 1))) => Ok(Cell::Integer(number)),
            (LogicalType::Bool, Cell::Integer(number)) => {
                Err(format!("{number} ist kein boolescher 0/1-Wert"))
            }
            (LogicalType::I64, Cell::Integer(number)) => Ok(Cell::Integer(number)),
            (
                LogicalType::String
                | LogicalType::Uuid
                | LogicalType::Date
                | LogicalType::Time
                | LogicalType::DateTime,
                Cell::Text(text),
            ) => Ok(Cell::Text(text)),
            _ => Err("gespeicherter Wert passt nicht zum Ausgangstyp".to_owned()),
        };
    }
    match (from, to, value) {
        (LogicalType::I64, LogicalType::String, Cell::Integer(number)) => {
            Ok(Cell::Text(number.to_string()))
        }
        (LogicalType::String, LogicalType::I64, Cell::Text(text)) => {
            parse_canonical_i64(&text).map(Cell::Integer)
        }
        _ => Err(format!(
            "keine verlustfreie Konvertierung von {from:?} nach {to:?}"
        )),
    }
}

fn parse_canonical_i64(text: &str) -> Result<i64, String> {
    let parsed = text
        .parse::<i64>()
        .map_err(|_| format!("`{text}` ist keine ganze Zahl"))?;
    if parsed.to_string() != text {
        return Err(format!(
            "`{text}` würde beim Umwandeln zu `{parsed}` verändert"
        ));
    }
    Ok(parsed)
}

fn physical_columns(columns: &[ColumnSnapshot]) -> Vec<PhysicalColumn<'_>> {
    columns
        .iter()
        .map(|column| PhysicalColumn {
            name: column.name.as_str(),
            kind: column.logical_type.into(),
            nullable: column.nullable,
            primary_key: column.primary_key,
            default_bool: column.default,
        })
        .collect()
}

fn quote_columns(columns: &[ColumnSnapshot]) -> String {
    columns
        .iter()
        .map(|column| format!("\"{}\"", column.name))
        .collect::<Vec<_>>()
        .join(", ")
}

async fn count_where(
    db: &impl ConnectionTrait,
    table: &str,
    predicate: &str,
) -> Result<i64, MigrationError> {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            format!("SELECT COUNT(*) AS total FROM \"{table}\" WHERE {predicate}"),
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!("Zählen in `{table}` fehlgeschlagen: {error}"))
        })?
        .ok_or_else(|| MigrationError::new(format!("Zählen in `{table}` lieferte keine Zeile.")))?;
    row.try_get::<i64>("", "total")
        .or_else(|_| row.try_get::<i32>("", "total").map(i64::from))
        .map_err(|error| MigrationError::new(format!("COUNT(*) ist nicht lesbar: {error}")))
}

async fn execute(
    db: &impl ConnectionTrait,
    backend: DatabaseBackend,
    sql: &str,
) -> Result<(), MigrationError> {
    db.execute(Statement::from_string(backend, sql))
        .await
        .map_err(|error| MigrationError::new(format!("SQL fehlgeschlagen: {error}")))?;
    Ok(())
}
