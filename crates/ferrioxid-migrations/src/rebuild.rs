//! Controlled rebuild. SQLite copies rows with `INSERT INTO ... SELECT`; PostgreSQL alters columns directly.
//!
//! SQLite cannot add a foreign key with `ALTER TABLE`. The runner therefore turns
//! `PRAGMA foreign_keys` off on one physical connection, runs this rebuild in that
//! connection's transaction, and turns enforcement back on before the connection
//! returns to the pool. A failed restore closes the connection instead of returning
//! it with enforcement disabled. Inside the transaction, `PRAGMA defer_foreign_keys = ON`
//! and `PRAGMA foreign_key_check` still run. This module does not change `PRAGMA foreign_keys`.
//! CREATE, INSERT, DROP, and RENAME run in the migration transaction and are rolled back on errors.

use crate::diff::{alter_is_supported, backfill_literal_for, BackfillLiteral, MigrationOperation};
use crate::error::MigrationError;
use crate::snapshot::{
    is_identifier, ColumnSnapshot, ForeignKeySnapshot, LogicalType, OnDelete, SchemaSnapshot,
    TableSnapshot,
};
use ferrioxid_core::{DatabaseKind, ReferentialAction};
use ferrioxid_db::{render_create_index, render_table_bundle, ForeignKeyClause, PhysicalColumn};
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
            "Changing `{table}.{}` from {:?} to {:?} is not supported.",
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
        .ok_or_else(|| {
            MigrationError::new(format!("Table `{table}` is missing before rebuild."))
        })?;
    if !to.nullable && from.nullable {
        let nulls = count_where(db, table, &format!("\"{}\" IS NULL", from.name)).await?;
        if nulls > 0 {
            return Err(MigrationError::new(format!(
                "Column `{table}.{}` contains {nulls} NULL values. NOT NULL is not applied, and no replacement values are inserted.",
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
    let mut target = source.clone();
    target.columns.clone_from(&target_columns);
    let selection = source
        .columns
        .iter()
        .map(|column| copy_expression(column, from, to))
        .collect::<Result<Vec<_>, _>>()?;
    reconstruct_sqlite(db, &target, &selection).await
}

pub fn backfill_parts(
    operation: &MigrationOperation,
) -> Option<(
    &str,
    &ColumnSnapshot,
    &BackfillLiteral,
    Option<&ForeignKeySnapshot>,
)> {
    match operation {
        MigrationOperation::AddColumn {
            table,
            column,
            foreign_key,
            backfill: Some(backfill),
        } => Some((table.as_str(), column, backfill, foreign_key.as_ref())),
        _ => None,
    }
}

pub fn added_foreign_key(operation: &MigrationOperation) -> Option<(&str, &ForeignKeySnapshot)> {
    match operation {
        MigrationOperation::AddForeignKey { table, foreign_key } => {
            Some((table.as_str(), foreign_key))
        }
        _ => None,
    }
}

pub async fn apply_backfill_column(
    db: &impl ConnectionTrait,
    kind: DatabaseKind,
    before: &SchemaSnapshot,
    table: &str,
    column: &ColumnSnapshot,
    backfill: &BackfillLiteral,
    foreign_key: Option<&ForeignKeySnapshot>,
) -> Result<(), MigrationError> {
    if column.nullable || column.default.is_some() || column.primary_key {
        return Err(MigrationError::new(format!(
            "Column `{table}.{}` does not accept an explicit backfill.",
            column.name
        )));
    }
    let source = required_table(before, table)?;
    if source
        .columns
        .iter()
        .any(|candidate| candidate.name == column.name)
    {
        return Err(MigrationError::new(format!(
            "Column `{table}.{}` already exists.",
            column.name
        )));
    }
    match kind {
        DatabaseKind::Sqlite => {
            let mut target = source.clone();
            target.columns.push(column.clone());
            if let Some(foreign_key) = foreign_key {
                target.foreign_keys.push(foreign_key.clone());
            }
            let mut selection = source
                .columns
                .iter()
                .map(|candidate| format!("\"{}\"", candidate.name))
                .collect::<Vec<_>>();
            selection.push(backfill_literal_for(
                DatabaseKind::Sqlite,
                column,
                backfill,
            )?);
            reconstruct_sqlite(db, &target, &selection).await
        }
        DatabaseKind::Postgres => {
            add_backfill_postgres(db, table, column, backfill).await?;
            if let Some(foreign_key) = foreign_key {
                add_foreign_key_postgres(db, table, foreign_key).await?;
            }
            Ok(())
        }
    }?;
    assert_no_nulls(db, table, &column.name).await
}

pub async fn apply_add_foreign_key(
    db: &impl ConnectionTrait,
    kind: DatabaseKind,
    before: &SchemaSnapshot,
    table: &str,
    foreign_key: &ForeignKeySnapshot,
) -> Result<(), MigrationError> {
    let source = required_table(before, table)?;
    if source
        .foreign_keys
        .iter()
        .any(|candidate| candidate.name == foreign_key.name)
    {
        return Err(MigrationError::new(format!(
            "Foreign key `{}` already exists on `{table}`.",
            foreign_key.name
        )));
    }
    reject_orphan_references(db, table, foreign_key).await?;
    match kind {
        DatabaseKind::Sqlite => {
            let mut target = source.clone();
            target.foreign_keys.push(foreign_key.clone());
            let selection = target
                .columns
                .iter()
                .map(|column| format!("\"{}\"", column.name))
                .collect::<Vec<_>>();
            reconstruct_sqlite(db, &target, &selection).await
        }
        DatabaseKind::Postgres => add_foreign_key_postgres(db, table, foreign_key).await,
    }
}

pub fn dropped_foreign_key(operation: &MigrationOperation) -> Option<(&str, &str)> {
    match operation {
        MigrationOperation::DropForeignKey { table, name, .. } => {
            Some((table.as_str(), name.as_str()))
        }
        _ => None,
    }
}

pub async fn apply_drop_foreign_key(
    db: &impl ConnectionTrait,
    kind: DatabaseKind,
    before: &SchemaSnapshot,
    table: &str,
    name: &str,
) -> Result<(), MigrationError> {
    let source = required_table(before, table)?;
    if !source
        .foreign_keys
        .iter()
        .any(|candidate| candidate.name == name)
    {
        return Err(MigrationError::new(format!(
            "Foreign key `{name}` is missing on `{table}`."
        )));
    }
    if !is_identifier(table) || !is_identifier(name) {
        return Err(MigrationError::new(format!(
            "Foreign key `{name}` on `{table}` is not a valid identifier."
        )));
    }
    match kind {
        DatabaseKind::Sqlite => {
            let mut target = source.clone();
            target
                .foreign_keys
                .retain(|candidate| candidate.name != name);
            let selection = target
                .columns
                .iter()
                .map(|column| format!("\"{}\"", column.name))
                .collect::<Vec<_>>();
            reconstruct_sqlite(db, &target, &selection).await
        }
        DatabaseKind::Postgres => {
            execute(
                db,
                DatabaseBackend::Postgres,
                &format!("ALTER TABLE \"{table}\" DROP CONSTRAINT \"{name}\""),
            )
            .await
        }
    }
}

async fn assert_no_nulls(
    db: &impl ConnectionTrait,
    table: &str,
    column: &str,
) -> Result<(), MigrationError> {
    if !is_identifier(table) || !is_identifier(column) {
        return Err(MigrationError::new(format!(
            "Column `{table}.{column}` is not a valid identifier."
        )));
    }
    let remaining = count_where(db, table, &format!("\"{column}\" IS NULL")).await?;
    if remaining != 0 {
        return Err(MigrationError::new(format!(
            "Column `{table}.{column}` still has {remaining} NULL rows. The NOT NULL constraint was not applied and the migration is rolled back."
        )));
    }
    Ok(())
}

async fn reconstruct_sqlite(
    db: &impl ConnectionTrait,
    target: &TableSnapshot,
    selection: &[String],
) -> Result<(), MigrationError> {
    if !is_identifier(&target.name) {
        return Err(MigrationError::new(format!(
            "Invalid table name `{}`.",
            target.name
        )));
    }
    if selection.len() != target.columns.len() {
        return Err(MigrationError::new(format!(
            "Rebuild of `{}` copied {} values for {} columns.",
            target.name,
            selection.len(),
            target.columns.len()
        )));
    }
    assert_no_views(db, &target.name).await?;
    let temporary = format!("ferrioxid_rebuild_{}", target.name);
    if !is_identifier(&temporary) {
        return Err(MigrationError::new(format!(
            "Temporary table name `{temporary}` is invalid."
        )));
    }
    let triggers = trigger_sql(db, &target.name).await?;
    let before = count_where(db, &target.name, "1 = 1").await?;
    execute(
        db,
        DatabaseBackend::Sqlite,
        "PRAGMA defer_foreign_keys = ON",
    )
    .await?;
    let foreign_keys = foreign_key_clauses(target)?;
    let mut create = render_table_bundle(
        DatabaseKind::Sqlite,
        &temporary,
        &physical_columns(&target.columns),
        &[],
        &foreign_keys,
        &[],
    );
    create[0] = ferrioxid_db::with_checks(
        std::mem::take(&mut create[0]),
        &choice_checks(&target.columns),
    );
    execute(db, DatabaseBackend::Sqlite, &create[0]).await?;
    execute(
        db,
        DatabaseBackend::Sqlite,
        &format!(
            "INSERT INTO \"{temporary}\" ({}) SELECT {} FROM \"{}\"",
            quote_columns(&target.columns),
            selection.join(", "),
            target.name
        ),
    )
    .await?;
    let copied = count_where(db, &temporary, "1 = 1").await?;
    if copied != before {
        return Err(MigrationError::new(format!(
            "Nach dem Kopieren hat `{temporary}` {copied} Zeilen, die Quelle hatte {before}."
        )));
    }
    execute(
        db,
        DatabaseBackend::Sqlite,
        &format!("DROP TABLE \"{}\"", target.name),
    )
    .await?;
    execute(
        db,
        DatabaseBackend::Sqlite,
        &format!("ALTER TABLE \"{temporary}\" RENAME TO \"{}\"", target.name),
    )
    .await?;
    let restored = count_where(db, &target.name, "1 = 1").await?;
    if restored != before {
        return Err(MigrationError::new(
            "Row count does not match after replacing the table.".to_owned(),
        ));
    }
    for unique in &target.unique_constraints {
        let columns = unique
            .columns
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        execute(
            db,
            DatabaseBackend::Sqlite,
            &render_create_index(
                DatabaseKind::Sqlite,
                &unique.name,
                &target.name,
                &columns,
                true,
            ),
        )
        .await?;
    }
    for index in &target.indexes {
        let columns = index.columns.iter().map(String::as_str).collect::<Vec<_>>();
        execute(
            db,
            DatabaseBackend::Sqlite,
            &render_create_index(
                DatabaseKind::Sqlite,
                &index.name,
                &target.name,
                &columns,
                false,
            ),
        )
        .await?;
    }
    for trigger in &triggers {
        execute(db, DatabaseBackend::Sqlite, trigger).await?;
    }
    let integrity = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA integrity_check",
        ))
        .await
        .map_err(|error| MigrationError::new(format!("integrity_check fehlgeschlagen: {error}")))?
        .ok_or_else(|| MigrationError::new("integrity_check lieferte keine Zeile."))?;
    let status: String = integrity
        .try_get("", "integrity_check")
        .map_err(|error| MigrationError::new(format!("Could not read integrity_check: {error}")))?;
    if status != "ok" {
        return Err(MigrationError::new(format!(
            "integrity_check returned `{status}`. The transaction is rolled back."
        )));
    }
    let violations = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_key_check",
        ))
        .await
        .map_err(|error| {
            MigrationError::new(format!("foreign_key_check fehlgeschlagen: {error}"))
        })?;
    if !violations.is_empty() {
        return Err(MigrationError::new(
            "foreign_key_check reported violations. The transaction is rolled back and no rows were changed."
                .to_owned(),
        ));
    }
    Ok(())
}

async fn add_backfill_postgres(
    db: &impl ConnectionTrait,
    table: &str,
    column: &ColumnSnapshot,
    backfill: &BackfillLiteral,
) -> Result<(), MigrationError> {
    let mut nullable = column.clone();
    nullable.nullable = true;
    nullable.default = None;
    execute(
        db,
        DatabaseBackend::Postgres,
        &ferrioxid_db::render_add_column(
            DatabaseKind::Postgres,
            table,
            &PhysicalColumn {
                name: nullable.name.as_str(),
                kind: nullable.logical_type.into(),
                nullable: true,
                primary_key: false,
                default_bool: None,
            },
        ),
    )
    .await?;
    let literal = backfill_literal_for(DatabaseKind::Postgres, column, backfill)?;
    execute(
        db,
        DatabaseBackend::Postgres,
        &format!(
            "UPDATE \"{table}\" SET \"{}\" = {literal} WHERE \"{}\" IS NULL",
            column.name, column.name
        ),
    )
    .await?;
    let missing = count_where(db, table, &format!("\"{}\" IS NULL", column.name)).await?;
    if missing > 0 {
        return Err(MigrationError::new(format!(
            "Backfill for `{table}.{}` left {missing} NULL values. No replacement was invented.",
            column.name
        )));
    }
    execute(
        db,
        DatabaseBackend::Postgres,
        &format!(
            "ALTER TABLE \"{table}\" ALTER COLUMN \"{}\" SET NOT NULL",
            column.name
        ),
    )
    .await
}

async fn add_foreign_key_postgres(
    db: &impl ConnectionTrait,
    table: &str,
    foreign_key: &ForeignKeySnapshot,
) -> Result<(), MigrationError> {
    let column = foreign_key.columns.first().ok_or_else(|| {
        MigrationError::new(format!(
            "Foreign key `{}` must reference exactly one column.",
            foreign_key.name
        ))
    })?;
    let referenced = foreign_key.referenced_columns.first().ok_or_else(|| {
        MigrationError::new(format!(
            "Foreign key `{}` must reference exactly one column.",
            foreign_key.name
        ))
    })?;
    execute(
        db,
        DatabaseBackend::Postgres,
        &format!(
            "ALTER TABLE \"{table}\" ADD CONSTRAINT \"{}\" FOREIGN KEY (\"{column}\") REFERENCES \"{}\" (\"{referenced}\") ON DELETE {} ON UPDATE NO ACTION",
            foreign_key.name,
            foreign_key.referenced_table,
            on_delete_sql(foreign_key.on_delete)
        ),
    )
    .await
}

fn on_delete_sql(action: OnDelete) -> &'static str {
    match action {
        OnDelete::Restrict => "RESTRICT",
        OnDelete::Cascade => "CASCADE",
        OnDelete::SetNull => "SET NULL",
    }
}

fn foreign_key_clauses(table: &TableSnapshot) -> Result<Vec<ForeignKeyClause<'_>>, MigrationError> {
    let mut clauses = Vec::new();
    for foreign_key in &table.foreign_keys {
        let (Some(column), Some(referenced)) = (
            foreign_key.columns.first(),
            foreign_key.referenced_columns.first(),
        ) else {
            return Err(MigrationError::new(format!(
                "Foreign key `{}` must reference exactly one column.",
                foreign_key.name
            )));
        };
        clauses.push(ForeignKeyClause {
            name: foreign_key.name.as_str(),
            column: column.as_str(),
            referenced_table: foreign_key.referenced_table.as_str(),
            referenced_column: referenced.as_str(),
            on_delete: match foreign_key.on_delete {
                OnDelete::Restrict => ReferentialAction::Restrict,
                OnDelete::Cascade => ReferentialAction::Cascade,
                OnDelete::SetNull => ReferentialAction::SetNull,
            },
        });
    }
    Ok(clauses)
}

fn required_table<'a>(
    schema: &'a SchemaSnapshot,
    table: &str,
) -> Result<&'a TableSnapshot, MigrationError> {
    schema
        .tables
        .iter()
        .find(|candidate| candidate.name == table)
        .ok_or_else(|| MigrationError::new(format!("Table `{table}` is missing before rebuild.")))
}

async fn reject_orphan_references(
    db: &impl ConnectionTrait,
    table: &str,
    foreign_key: &ForeignKeySnapshot,
) -> Result<(), MigrationError> {
    let column = foreign_key.columns.first().ok_or_else(|| {
        MigrationError::new(format!(
            "Foreign key `{}` must reference exactly one column.",
            foreign_key.name
        ))
    })?;
    let referenced = foreign_key.referenced_columns.first().ok_or_else(|| {
        MigrationError::new(format!(
            "Foreign key `{}` must reference exactly one column.",
            foreign_key.name
        ))
    })?;
    if !is_identifier(table)
        || !is_identifier(column)
        || !is_identifier(&foreign_key.referenced_table)
        || !is_identifier(referenced)
    {
        return Err(MigrationError::new(
            "Foreign-key validation received an invalid identifier.".to_owned(),
        ));
    }
    let sql = format!(
        "SELECT COUNT(*) AS total FROM \"{table}\" WHERE \"{column}\" IS NOT NULL AND NOT EXISTS (SELECT 1 FROM \"{}\" AS referenced WHERE referenced.\"{referenced}\" = \"{table}\".\"{column}\")",
        foreign_key.referenced_table
    );
    let row = db
        .query_one(Statement::from_string(db.get_database_backend(), sql))
        .await
        .map_err(|error| {
            MigrationError::new(format!(
                "Could not validate `{}.{}` against `{}.{}`: {error}. The foreign key was not added.",
                table, column, foreign_key.referenced_table, referenced
            ))
        })?
        .ok_or_else(|| MigrationError::new("Foreign-key validation returned no row.".to_owned()))?;
    let orphans = row
        .try_get::<i64>("", "total")
        .or_else(|_| row.try_get::<i32>("", "total").map(i64::from))
        .map_err(|error| MigrationError::new(format!("Could not read orphan count: {error}")))?;
    if orphans > 0 {
        return Err(MigrationError::new(format!(
            "{orphans} existing values in `{table}.{column}` do not match `{}.{}`. The foreign key was not added and no rows were changed.",
            foreign_key.referenced_table, referenced
        )));
    }
    Ok(())
}

async fn trigger_sql(
    db: &impl ConnectionTrait,
    table: &str,
) -> Result<Vec<String>, MigrationError> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = ? AND sql IS NOT NULL",
            [Value::from(table)],
        ))
        .await
        .map_err(|error| MigrationError::new(format!("Could not read triggers: {error}")))?;
    let mut statements = Vec::new();
    for row in rows {
        let sql: String = row
            .try_get("", "sql")
            .map_err(|error| MigrationError::new(format!("Could not read trigger SQL: {error}")))?;
        if sql.trim().is_empty() {
            return Err(MigrationError::new(format!(
                "Table `{table}` has a trigger without SQL. The rebuild was not applied."
            )));
        }
        statements.push(sql);
    }
    Ok(statements)
}

async fn assert_no_views(db: &impl ConnectionTrait, table: &str) -> Result<(), MigrationError> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name, sql FROM sqlite_master WHERE type = 'view' AND sql IS NOT NULL",
        ))
        .await
        .map_err(|error| MigrationError::new(format!("Could not read views: {error}")))?;
    for row in rows {
        let name: String = row
            .try_get("", "name")
            .unwrap_or_else(|_| "unknown".to_owned());
        let sql: String = row.try_get("", "sql").unwrap_or_default();
        let mentions = sql
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .any(|token| token == table);
        if mentions {
            return Err(MigrationError::new(format!(
                "Table `{table}` has view `{name}`. Rebuilding it would drop that view, so the migration was not applied."
            )));
        }
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
        if from.logical_type == LogicalType::String && to.logical_type == LogicalType::Choice {
            let list = to
                .choices
                .iter()
                .map(|choice| format!("'{}'", choice.value))
                .collect::<Vec<_>>()
                .join(", ");
            execute(
                db,
                DatabaseBackend::Postgres,
                &format!(
                    "ALTER TABLE \"{table}\" ADD CONSTRAINT \"ck_{table}_{}\" CHECK (\"{}\" IN ({list}))",
                    to.name, to.name
                ),
            )
            .await?;
        } else {
            let sql_type = match to.logical_type {
            LogicalType::String | LogicalType::Uuid => "text",
            LogicalType::I64 => "bigint",
            LogicalType::Bool => "boolean",
            LogicalType::Date => "date",
            LogicalType::Time => "time",
            LogicalType::DateTime => "timestamptz",
            LogicalType::Decimal | LogicalType::Choice | LogicalType::Json => {
                return Err(MigrationError::new(format!(
                    "Changing `{}` to {:?} is not an automatic column rewrite. Add a new column and backfill it explicitly.",
                    to.name, to.logical_type
                )))
            }
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
            "Invalid table name `{table}`."
        )));
    }
    assert_no_views(db, table).await
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
            "The retrieved row count does not match COUNT(*). No rebuild was written.".to_owned(),
        ));
    }
    for (index, row) in rows.iter().enumerate() {
        let cell = read_cell(row, "value", from.logical_type)?;
        if to.logical_type == LogicalType::Choice {
            if let Cell::Text(text) = &cell {
                if !to.choices.iter().any(|choice| choice.value == *text) {
                    return Err(MigrationError::new(format!(
                        "Row {index} in `{table}.{}` has `{text}`, which is not a stored choice. No rows were rewritten.",
                        from.name
                    )));
                }
            }
        }
        let converted =
            convert_cell(cell, from.logical_type, to.logical_type).map_err(|error| {
                MigrationError::new(format!(
                    "Row {index} in `{table}.{}` cannot be converted: {error}",
                    from.name
                ))
            })?;
        if !to.nullable && matches!(converted, Cell::Null) {
            return Err(MigrationError::new(format!(
                "Row {index} in `{table}.{}` is NULL.",
                from.name
            )));
        }
    }
    Ok(())
}

fn choice_checks(columns: &[crate::snapshot::ColumnSnapshot]) -> Vec<String> {
    columns
        .iter()
        .filter(|column| column.logical_type == crate::snapshot::LogicalType::Choice)
        .map(|column| {
            let list = column
                .choices
                .iter()
                .map(|choice| format!("'{}'", choice.value))
                .collect::<Vec<_>>()
                .join(", ");
            format!("CHECK (\"{}\" IN ({list}))", column.name)
        })
        .collect()
}

fn copy_expression(
    column: &ColumnSnapshot,
    from: &ColumnSnapshot,
    to: &ColumnSnapshot,
) -> Result<String, MigrationError> {
    if column.name != from.name || from.logical_type == to.logical_type {
        return Ok(format!("\"{}\"", column.name));
    }
    if from.logical_type == LogicalType::String && to.logical_type == LogicalType::Choice {
        return Ok(format!("\"{}\"", column.name));
    }
    let sql_type = match to.logical_type {
        LogicalType::String | LogicalType::Date | LogicalType::Time | LogicalType::DateTime => {
            "TEXT"
        }
        LogicalType::I64 => "INTEGER",
        LogicalType::Uuid
        | LogicalType::Bool
        | LogicalType::Decimal
        | LogicalType::Choice
        | LogicalType::Json => {
            return Err(MigrationError::new(format!(
                "Changing `{}` from {:?} to {:?} is not supported.",
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
        | LogicalType::DateTime
        | LogicalType::Choice
        | LogicalType::Json => match row.try_get::<Option<String>>("", name) {
            Ok(Some(text)) => Ok(Cell::Text(text)),
            Ok(None) => Ok(Cell::Null),
            Err(error) => Err(MigrationError::new(format!(
                "Could not read text value in `{name}`: {error}"
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
                "Could not read numeric value in `{name}`."
            )))
        }
        LogicalType::Decimal => match row.try_get::<Option<Vec<u8>>>("", name) {
            Ok(Some(bytes)) => Ok(Cell::Text(
                bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
            )),
            Ok(None) => Ok(Cell::Null),
            Err(error) => Err(MigrationError::new(format!(
                "Could not read decimal value in `{name}`: {error}"
            ))),
        },
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
                Err(format!("{number} is not a boolean 0/1 value"))
            }
            (LogicalType::I64, Cell::Integer(number)) => Ok(Cell::Integer(number)),
            (
                LogicalType::String
                | LogicalType::Uuid
                | LogicalType::Date
                | LogicalType::Time
                | LogicalType::DateTime
                | LogicalType::Choice
                | LogicalType::Decimal,
                Cell::Text(text),
            ) => Ok(Cell::Text(text)),
            _ => Err("stored value does not match the source type".to_owned()),
        };
    }
    match (from, to, value) {
        (LogicalType::I64, LogicalType::String, Cell::Integer(number)) => {
            Ok(Cell::Text(number.to_string()))
        }
        (LogicalType::String, LogicalType::I64, Cell::Text(text)) => {
            parse_canonical_i64(&text).map(Cell::Integer)
        }
        (LogicalType::String, LogicalType::Choice, Cell::Text(text)) => Ok(Cell::Text(text)),
        _ => Err(format!(
            "keine verlustfreie Konvertierung von {from:?} nach {to:?}"
        )),
    }
}

fn parse_canonical_i64(text: &str) -> Result<i64, String> {
    let parsed = text
        .parse::<i64>()
        .map_err(|_| format!("`{text}` is not an integer"))?;
    if parsed.to_string() != text {
        return Err(format!(
            "`{text}` would change when converting to `{parsed}`"
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
            MigrationError::new(format!("Could not count rows in `{table}`: {error}"))
        })?
        .ok_or_else(|| {
            MigrationError::new(format!("Row count query for `{table}` returned no rows."))
        })?;
    row.try_get::<i64>("", "total")
        .or_else(|_| row.try_get::<i32>("", "total").map(i64::from))
        .map_err(|error| MigrationError::new(format!("Could not read COUNT(*): {error}")))
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

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseBackend;

    #[tokio::test]
    async fn null_rows_block_the_not_null_check() {
        let db = ferrioxid_db::connect("sqlite::memory:").await.unwrap();
        execute(
            &db,
            DatabaseBackend::Sqlite,
            "CREATE TABLE ferrioxid_null_gate (id text PRIMARY KEY NOT NULL, stock bigint)",
        )
        .await
        .unwrap();
        execute(
            &db,
            DatabaseBackend::Sqlite,
            "INSERT INTO ferrioxid_null_gate (id, stock) VALUES ('one', NULL)",
        )
        .await
        .unwrap();
        let blocked = assert_no_nulls(&db, "ferrioxid_null_gate", "stock").await;
        assert!(blocked.is_err());
        execute(
            &db,
            DatabaseBackend::Sqlite,
            "UPDATE ferrioxid_null_gate SET stock = 0 WHERE stock IS NULL",
        )
        .await
        .unwrap();
        assert_no_nulls(&db, "ferrioxid_null_gate", "stock")
            .await
            .unwrap();
    }
}
