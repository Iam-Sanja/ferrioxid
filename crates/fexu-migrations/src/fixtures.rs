//! Versionierte JSON-Fixtures.
//!
//! Das Format ist von Migrationen getrennt. Ein Datensatz führt keinen Code
//! aus. Der Import prüft Modelle, Felder und Referenzen, bevor er schreibt,
//! und hält alle Schreibvorgänge in einer Transaktion.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use fexu_core::{
    decode_decimal, encode_decimal, format_decimal, parse_date, parse_datetime_utc, parse_decimal,
    parse_time, ColumnKind,
};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement, TransactionTrait, Value,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::MigrationError;
use crate::snapshot::{is_identifier, SchemaSnapshot, TableSnapshot};

pub const FIXTURE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureFile {
    pub fexu: u32,
    pub records: Vec<FixtureRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureRecord {
    pub model: String,
    pub pk: String,
    pub fields: Map<String, Json>,
}

#[derive(Clone, Debug, Default)]
pub struct LoadRequest {
    pub tables: Option<BTreeSet<String>>,
    pub upsert: bool,
    pub truncate: bool,
    pub dry_run: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadReport {
    pub records: usize,
    pub tables: Vec<String>,
    pub written: bool,
}

pub fn parse_fixture(text: &str) -> Result<FixtureFile, MigrationError> {
    let file: FixtureFile = serde_json::from_str(text)
        .map_err(|error| MigrationError::new(format!("The fixture is not valid JSON: {error}")))?;
    if file.fexu != FIXTURE_VERSION {
        return Err(MigrationError::new(format!(
            "Fixture version {} is not supported. This release reads version {FIXTURE_VERSION}.",
            file.fexu
        )));
    }
    Ok(file)
}

pub fn fixture_to_json(file: &FixtureFile) -> Result<String, MigrationError> {
    serde_json::to_string_pretty(file)
        .map_err(|error| MigrationError::new(format!("Could not write the fixture: {error}")))
}

/// Prüft das Dokument, ohne eine Zeile zu schreiben.
pub fn prepare_records(
    schema: &SchemaSnapshot,
    file: &FixtureFile,
    tables: Option<&BTreeSet<String>>,
) -> Result<Vec<FixtureRecord>, MigrationError> {
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for record in &file.records {
        if let Some(tables) = tables {
            if !tables.contains(&record.model) {
                continue;
            }
        }
        let table = find_table(schema, &record.model).ok_or_else(|| {
            MigrationError::new(format!("Unknown fixture model '{}'.", record.model))
        })?;
        require_identifier(&record.model)?;
        require_id_key(table)?;
        if !is_identifier(&record.pk) && uuid::Uuid::parse_str(&record.pk).is_err() {
            return Err(MigrationError::new(format!(
                "Primary key '{}' on '{}' is neither a UUID nor an identifier.",
                record.pk, record.model
            )));
        }
        if !seen.insert((record.model.clone(), record.pk.clone())) {
            return Err(MigrationError::new(format!(
                "Fixture model '{}' repeats primary key '{}'.",
                record.model, record.pk
            )));
        }
        reject_composite_keys(table)?;
        for name in record.fields.keys() {
            if name == "id" {
                return Err(MigrationError::new(
                    "Put the primary key in pk, not in fields.".to_owned(),
                ));
            }
            if !table.columns.iter().any(|column| column.name == *name) {
                return Err(MigrationError::new(format!(
                    "Unknown field '{name}' on model '{}'.",
                    record.model
                )));
            }
        }
        for column in &table.columns {
            if column.name == "id" || column.primary_key {
                continue;
            }
            match record.fields.get(&column.name) {
                None if column.nullable || column.default.is_some() => {}
                None => {
                    return Err(MigrationError::new(format!(
                        "Model '{}' is missing required field '{}'.",
                        record.model, column.name
                    )));
                }
                Some(value) => validate_value(column, value)?,
            }
        }
        selected.push(record.clone());
    }
    order_records(schema, selected)
}

fn require_id_key(table: &TableSnapshot) -> Result<(), MigrationError> {
    let keys = table
        .columns
        .iter()
        .filter(|column| column.primary_key)
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    if keys == ["id"] {
        Ok(())
    } else {
        Err(MigrationError::new(format!(
            "Model '{}' needs a single primary key named id. Fixture loading stops before writing.",
            table.name
        )))
    }
}

fn reject_composite_keys(table: &TableSnapshot) -> Result<(), MigrationError> {
    for foreign_key in &table.foreign_keys {
        if foreign_key.columns.len() != 1 || foreign_key.referenced_columns != ["id"] {
            return Err(MigrationError::new(format!(
                "Model '{}' has a foreign key that does not reference a single id column. Fixture loading stops before writing.",
                table.name
            )));
        }
    }
    Ok(())
}

fn validate_value(
    column: &crate::snapshot::ColumnSnapshot,
    value: &Json,
) -> Result<(), MigrationError> {
    if value.is_null() {
        return if column.nullable {
            Ok(())
        } else {
            Err(MigrationError::new(format!(
                "Field '{}' does not accept null.",
                column.name
            )))
        };
    }
    match ColumnKind::from(column.logical_type) {
        ColumnKind::Decimal => {
            let text = value.as_str().ok_or_else(|| {
                MigrationError::new(format!(
                    "Decimal field '{}' must be a JSON string, for example \"12.50\".",
                    column.name
                ))
            })?;
            let (precision, scale) = decimal_meta(column)?;
            parse_decimal(text, precision, scale).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
        }
        ColumnKind::Json => {
            let text = serde_json::to_string(value).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
            fexu_core::canonical_json_text(&text)
                .map_err(|code| MigrationError::new(format!("Field '{}': {code}", column.name)))?;
        }
        ColumnKind::Choice => {
            let text = value.as_str().ok_or_else(|| {
                MigrationError::new(format!("Choice field '{}' must be a string.", column.name))
            })?;
            if !column.choices.iter().any(|choice| choice.value == text) {
                return Err(MigrationError::new(format!(
                    "Field '{}' rejects unknown choice '{text}'.",
                    column.name
                )));
            }
        }
        ColumnKind::BigInt => {
            if value.as_i64().is_none() {
                return Err(MigrationError::new(format!(
                    "Integer field '{}' must be a signed 64-bit JSON number.",
                    column.name
                )));
            }
        }
        ColumnKind::Boolean => {
            if !value.is_boolean() {
                return Err(MigrationError::new(format!(
                    "Boolean field '{}' must be true or false.",
                    column.name
                )));
            }
        }
        ColumnKind::Uuid | ColumnKind::Text => {
            if !value.is_string() {
                return Err(MigrationError::new(format!(
                    "Field '{}' must be a JSON string.",
                    column.name
                )));
            }
        }
        ColumnKind::Date => {
            require_temporal(column, value, parse_date)?;
        }
        ColumnKind::Time => {
            require_temporal(column, value, parse_time)?;
        }
        ColumnKind::DateTime => {
            require_temporal(column, value, parse_datetime_utc)?;
        }
    }
    Ok(())
}

fn require_temporal<T>(
    column: &crate::snapshot::ColumnSnapshot,
    value: &Json,
    parse: fn(&str) -> Result<T, &'static str>,
) -> Result<(), MigrationError> {
    let text = value.as_str().ok_or_else(|| {
        MigrationError::new(format!("Field '{}' must be a JSON string.", column.name))
    })?;
    parse(text)
        .map(|_| ())
        .map_err(|error| MigrationError::new(format!("Field '{}': {error}", column.name)))
}

fn order_records(
    schema: &SchemaSnapshot,
    records: Vec<FixtureRecord>,
) -> Result<Vec<FixtureRecord>, MigrationError> {
    let positions: HashMap<(String, String), usize> = records
        .iter()
        .enumerate()
        .map(|(index, record)| ((record.model.clone(), record.pk.clone()), index))
        .collect();
    let mut incoming = vec![0_usize; records.len()];
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); records.len()];
    for (index, record) in records.iter().enumerate() {
        let table = find_table(schema, &record.model).expect("model was checked");
        for foreign_key in &table.foreign_keys {
            let column = &foreign_key.columns[0];
            let Some(value) = record.fields.get(column) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let foreign = value.as_str().ok_or_else(|| {
                MigrationError::new(format!(
                    "Foreign key '{column}' on '{}' must be a string primary key.",
                    record.model
                ))
            })?;
            let parent_key = (foreign_key.referenced_table.clone(), foreign.to_owned());
            if let Some(&parent) = positions.get(&parent_key) {
                if parent == index {
                    return Err(MigrationError::new(format!(
                        "Record '{}'.'{}' references itself.",
                        record.model, record.pk
                    )));
                }
                edges[parent].push(index);
                incoming[index] += 1;
            }
        }
    }
    let mut ready: BTreeSet<(String, usize)> = incoming
        .iter()
        .enumerate()
        .filter(|(_, count)| **count == 0)
        .map(|(index, _)| (records[index].pk.clone(), index))
        .collect();
    let mut ordered = Vec::with_capacity(records.len());
    while let Some((_, index)) = ready.pop_first() {
        ordered.push(index);
        let mut next = edges[index].clone();
        next.sort_unstable();
        for child in next {
            incoming[child] -= 1;
            if incoming[child] == 0 {
                ready.insert((records[child].pk.clone(), child));
            }
        }
    }
    if ordered.len() != records.len() {
        return Err(MigrationError::new(
            "The fixture contains a foreign-key cycle. Split the cycle or insert one side as null and patch it afterwards."
                .to_owned(),
        ));
    }
    Ok(ordered
        .into_iter()
        .map(|index| records[index].clone())
        .collect())
}

pub async fn load_fixture(
    db: &DatabaseConnection,
    schema: &SchemaSnapshot,
    file: &FixtureFile,
    request: &LoadRequest,
) -> Result<LoadReport, MigrationError> {
    if request.truncate && request.upsert {
        return Err(MigrationError::new(
            "Choose either --upsert or --truncate, not both.".to_owned(),
        ));
    }
    let records = prepare_records(schema, file, request.tables.as_ref())?;
    let tables = records
        .iter()
        .map(|record| record.model.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if request.dry_run {
        check_references(db, schema, &records).await?;
        check_conflicts(db, &records, request).await?;
        return Ok(LoadReport {
            records: records.len(),
            tables,
            written: false,
        });
    }
    let backend = db.get_database_backend();
    let transaction = db.begin().await.map_err(map_db)?;
    let written = async {
        if request.truncate {
            for table in truncate_order(schema, &records)? {
                execute(
                    &transaction,
                    backend,
                    &format!("DELETE FROM {}", quote(&table)),
                    vec![],
                )
                .await?;
            }
        }
        check_references(&transaction, schema, &records).await?;
        check_conflicts(&transaction, &records, request).await?;
        for record in &records {
            write_record(&transaction, backend, schema, record, request.upsert).await?;
        }
        Ok::<(), MigrationError>(())
    }
    .await;
    match written {
        Ok(()) => transaction.commit().await.map_err(map_db)?,
        Err(error) => {
            let _ = transaction.rollback().await;
            return Err(error);
        }
    }
    Ok(LoadReport {
        records: records.len(),
        tables,
        written: true,
    })
}

fn truncate_order(
    schema: &SchemaSnapshot,
    records: &[FixtureRecord],
) -> Result<Vec<String>, MigrationError> {
    let names = records
        .iter()
        .map(|record| record.model.clone())
        .collect::<BTreeSet<_>>();
    let mut incoming: BTreeMap<String, usize> =
        names.iter().cloned().map(|name| (name, 0)).collect();
    let mut edges: BTreeMap<String, BTreeSet<String>> = names
        .iter()
        .cloned()
        .map(|name| (name, BTreeSet::new()))
        .collect();
    for name in &names {
        let Some(table) = find_table(schema, name) else {
            continue;
        };
        for foreign_key in &table.foreign_keys {
            if names.contains(&foreign_key.referenced_table)
                && foreign_key.referenced_table != *name
            {
                edges
                    .get_mut(&foreign_key.referenced_table)
                    .expect("parent")
                    .insert(name.clone());
                *incoming.get_mut(name).expect("child") += 1;
            }
        }
    }
    let mut ready: BTreeSet<String> = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(name, _)| name.clone())
        .collect();
    let mut forward = Vec::new();
    while let Some(name) = ready.pop_first() {
        forward.push(name.clone());
        for child in edges.get(&name).cloned().unwrap_or_default() {
            let count = incoming.get_mut(&child).expect("child");
            *count -= 1;
            if *count == 0 {
                ready.insert(child);
            }
        }
    }
    if forward.len() != names.len() {
        return Err(MigrationError::new(
            "The selected tables have a foreign-key cycle, so truncate refuses to delete them."
                .to_owned(),
        ));
    }
    Ok(forward.into_iter().rev().collect())
}

async fn check_conflicts(
    db: &impl ConnectionTrait,
    records: &[FixtureRecord],
    request: &LoadRequest,
) -> Result<(), MigrationError> {
    if request.upsert || request.truncate {
        return Ok(());
    }
    for record in records {
        if row_exists(db, &record.model, &record.pk).await? {
            return Err(MigrationError::new(format!(
                "Model '{}' already contains primary key '{}'. loaddata does not overwrite existing rows. Pass --upsert to update them.",
                record.model, record.pk
            )));
        }
    }
    Ok(())
}

async fn check_references(
    db: &impl ConnectionTrait,
    schema: &SchemaSnapshot,
    records: &[FixtureRecord],
) -> Result<(), MigrationError> {
    let present: BTreeSet<(String, String)> = records
        .iter()
        .map(|record| (record.model.clone(), record.pk.clone()))
        .collect();
    for record in records {
        let table = find_table(schema, &record.model).expect("model was checked");
        for foreign_key in &table.foreign_keys {
            let column = &foreign_key.columns[0];
            let Some(value) = record.fields.get(column) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let foreign = value.as_str().ok_or_else(|| {
                MigrationError::new(format!("Foreign key '{column}' must be a string."))
            })?;
            if present.contains(&(foreign_key.referenced_table.clone(), foreign.to_owned())) {
                continue;
            }
            if !row_exists(db, &foreign_key.referenced_table, foreign).await? {
                return Err(MigrationError::new(format!(
                    "Model '{}' row '{}' references missing {}.id '{}'.",
                    record.model, record.pk, foreign_key.referenced_table, foreign
                )));
            }
        }
    }
    Ok(())
}

async fn row_exists(
    db: &impl ConnectionTrait,
    table: &str,
    pk: &str,
) -> Result<bool, MigrationError> {
    require_identifier(table)?;
    let backend = db.get_database_backend();
    let sql = format!(
        "SELECT 1 AS present FROM {} WHERE id = {} LIMIT 1",
        quote(table),
        placeholder(backend, 0)
    );
    let row = db
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![pk_value(backend, pk)?],
        ))
        .await
        .map_err(map_db)?;
    Ok(row.is_some())
}

async fn write_record(
    db: &impl ConnectionTrait,
    backend: DatabaseBackend,
    schema: &SchemaSnapshot,
    record: &FixtureRecord,
    upsert: bool,
) -> Result<(), MigrationError> {
    let table = find_table(schema, &record.model).expect("model was checked");
    require_identifier(&table.name)?;
    let mut columns = vec!["id".to_owned()];
    let mut values = vec![pk_value(backend, &record.pk)?];
    for column in &table.columns {
        if column.name == "id" || column.primary_key {
            continue;
        }
        let Some(json) = record.fields.get(&column.name) else {
            continue;
        };
        require_identifier(&column.name)?;
        columns.push(column.name.clone());
        values.push(bind_value(backend, column, json)?);
    }
    let placeholders = (0..columns.len())
        .map(|index| placeholder(backend, index))
        .collect::<Vec<_>>()
        .join(", ");
    let names = columns
        .iter()
        .map(|name| quote(name))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = if upsert {
        let assignments = columns
            .iter()
            .filter(|name| name.as_str() != "id")
            .map(|name| format!("{} = excluded.{}", quote(name), quote(name)))
            .collect::<Vec<_>>()
            .join(", ");
        if assignments.is_empty() {
            format!(
                "INSERT INTO {} ({names}) VALUES ({placeholders}) ON CONFLICT (id) DO NOTHING",
                quote(&record.model)
            )
        } else {
            format!(
                "INSERT INTO {} ({names}) VALUES ({placeholders}) ON CONFLICT (id) DO UPDATE SET {assignments}",
                quote(&record.model)
            )
        }
    } else {
        format!(
            "INSERT INTO {} ({names}) VALUES ({placeholders})",
            quote(&record.model)
        )
    };
    execute(db, backend, &sql, values).await
}

fn bind_value(
    backend: DatabaseBackend,
    column: &crate::snapshot::ColumnSnapshot,
    value: &Json,
) -> Result<Value, MigrationError> {
    if value.is_null() {
        return Ok(match ColumnKind::from(column.logical_type) {
            ColumnKind::Decimal => Value::Bytes(None),
            ColumnKind::BigInt => Value::BigInt(None),
            ColumnKind::Boolean => Value::Bool(None),
            ColumnKind::Uuid if backend == DatabaseBackend::Postgres => Value::Uuid(None),
            ColumnKind::Date if backend == DatabaseBackend::Postgres => Value::ChronoDate(None),
            ColumnKind::Time if backend == DatabaseBackend::Postgres => Value::ChronoTime(None),
            ColumnKind::DateTime if backend == DatabaseBackend::Postgres => {
                Value::ChronoDateTimeUtc(None)
            }
            _ => Value::String(None),
        });
    }
    match ColumnKind::from(column.logical_type) {
        ColumnKind::Decimal => {
            let text = value.as_str().expect("validated");
            let (precision, scale) = decimal_meta(column)?;
            let decimal = parse_decimal(text, precision, scale).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
            let bytes = encode_decimal(decimal, precision, scale).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
            Ok(Value::Bytes(Some(Box::new(bytes.to_vec()))))
        }
        ColumnKind::BigInt => Ok(Value::BigInt(Some(value.as_i64().expect("validated")))),
        ColumnKind::Boolean => Ok(Value::Bool(Some(value.as_bool().expect("validated")))),
        ColumnKind::Uuid => pk_value(backend, value.as_str().expect("validated")),
        ColumnKind::Json => {
            let text = serde_json::to_string(value).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
            let canonical = fexu_core::canonical_json_text(&text)
                .map_err(|code| MigrationError::new(format!("Field '{}': {code}", column.name)))?;
            let parsed = serde_json::from_str(&canonical).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
            Ok(Value::Json(Some(Box::new(parsed))))
        }
        ColumnKind::Text | ColumnKind::Choice => Ok(Value::String(Some(Box::new(
            value.as_str().expect("validated").to_owned(),
        )))),
        ColumnKind::Date => {
            let date = parse_date(value.as_str().expect("validated")).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
            Ok(match backend {
                DatabaseBackend::Postgres => Value::ChronoDate(Some(Box::new(date))),
                _ => Value::String(Some(Box::new(fexu_core::temporal::format_date(date)))),
            })
        }
        ColumnKind::Time => {
            let time = parse_time(value.as_str().expect("validated")).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
            Ok(match backend {
                DatabaseBackend::Postgres => Value::ChronoTime(Some(Box::new(time))),
                _ => Value::String(Some(Box::new(fexu_core::temporal::format_time(time)))),
            })
        }
        ColumnKind::DateTime => {
            let instant =
                parse_datetime_utc(value.as_str().expect("validated")).map_err(|error| {
                    MigrationError::new(format!("Field '{}': {error}", column.name))
                })?;
            Ok(match backend {
                DatabaseBackend::Postgres => Value::ChronoDateTimeUtc(Some(Box::new(instant))),
                _ => Value::String(Some(Box::new(canonical_datetime(instant)))),
            })
        }
    }
}

fn canonical_datetime(value: chrono::DateTime<chrono::Utc>) -> String {
    fexu_core::temporal::format_datetime(value)
}

fn pk_value(backend: DatabaseBackend, pk: &str) -> Result<Value, MigrationError> {
    if let Ok(id) = uuid::Uuid::parse_str(pk) {
        return Ok(match backend {
            DatabaseBackend::Postgres => Value::Uuid(Some(Box::new(id))),
            _ => Value::String(Some(Box::new(id.to_string()))),
        });
    }
    if is_identifier(pk) {
        return Ok(Value::String(Some(Box::new(pk.to_owned()))));
    }
    Err(MigrationError::new(format!(
        "Primary key '{pk}' is not a UUID."
    )))
}

pub async fn dump_fixture(
    db: &DatabaseConnection,
    schema: &SchemaSnapshot,
    tables: Option<&BTreeSet<String>>,
) -> Result<FixtureFile, MigrationError> {
    let mut records = Vec::new();
    for table in &schema.tables {
        if let Some(tables) = tables {
            if !tables.contains(&table.name) {
                continue;
            }
        }
        require_identifier(&table.name)?;
        let backend = db.get_database_backend();
        let rows = db
            .query_all(Statement::from_string(
                backend,
                format!("SELECT * FROM {} ORDER BY id", quote(&table.name)),
            ))
            .await
            .map_err(map_db)?;
        for row in rows {
            let pk = read_pk(&row)?;
            let mut fields = Map::new();
            for column in &table.columns {
                if column.name == "id" || column.primary_key {
                    continue;
                }
                fields.insert(column.name.clone(), read_cell(&row, column)?);
            }
            records.push(FixtureRecord {
                model: table.name.clone(),
                pk,
                fields,
            });
        }
    }
    records = order_records(schema, records)?;
    Ok(FixtureFile {
        fexu: FIXTURE_VERSION,
        records,
    })
}

fn read_pk(row: &sea_orm::QueryResult) -> Result<String, MigrationError> {
    if let Ok(value) = row.try_get::<String>("", "id") {
        return Ok(value);
    }
    if let Ok(value) = row.try_get::<uuid::Uuid>("", "id") {
        return Ok(value.to_string());
    }
    Err(MigrationError::new(
        "A dumped row has no readable id.".to_owned(),
    ))
}

fn read_cell(
    row: &sea_orm::QueryResult,
    column: &crate::snapshot::ColumnSnapshot,
) -> Result<Json, MigrationError> {
    match ColumnKind::from(column.logical_type) {
        ColumnKind::Decimal => {
            let bytes = row
                .try_get::<Option<Vec<u8>>>("", &column.name)
                .map_err(map_db)?;
            match bytes {
                None => Ok(Json::Null),
                Some(bytes) => {
                    let scale = column.scale.unwrap_or(0);
                    let decimal = decode_decimal(&bytes, scale).map_err(|error| {
                        MigrationError::new(format!("Field '{}': {error}", column.name))
                    })?;
                    Ok(Json::String(format_decimal(decimal, scale).map_err(
                        |error| MigrationError::new(format!("Field '{}': {error}", column.name)),
                    )?))
                }
            }
        }
        ColumnKind::BigInt => {
            let value = row
                .try_get::<Option<i64>>("", &column.name)
                .or_else(|_| {
                    row.try_get::<Option<i32>>("", &column.name)
                        .map(|value| value.map(i64::from))
                })
                .map_err(map_db)?;
            Ok(match value {
                Some(value) => Json::from(value),
                None => Json::Null,
            })
        }
        ColumnKind::Boolean => {
            if let Ok(value) = row.try_get::<Option<bool>>("", &column.name) {
                return Ok(match value {
                    Some(value) => Json::Bool(value),
                    None => Json::Null,
                });
            }
            let value = row
                .try_get::<Option<i64>>("", &column.name)
                .map_err(map_db)?;
            Ok(match value {
                Some(value) => Json::Bool(value != 0),
                None => Json::Null,
            })
        }
        ColumnKind::Uuid => {
            if let Ok(value) = row.try_get::<Option<String>>("", &column.name) {
                return Ok(match value {
                    Some(value) => Json::String(value),
                    None => Json::Null,
                });
            }
            let value = row
                .try_get::<Option<uuid::Uuid>>("", &column.name)
                .map_err(map_db)?;
            Ok(match value {
                Some(value) => Json::String(value.to_string()),
                None => Json::Null,
            })
        }
        ColumnKind::Json => {
            if let Ok(value) = row.try_get::<Option<serde_json::Value>>("", &column.name) {
                return Ok(match value {
                    Some(value) => value,
                    None => Json::Null,
                });
            }
            let value = row
                .try_get::<Option<String>>("", &column.name)
                .map_err(map_db)?;
            Ok(match value {
                Some(text) => serde_json::from_str(&text).map_err(|error| {
                    MigrationError::new(format!("Field '{}': {error}", column.name))
                })?,
                None => Json::Null,
            })
        }
        ColumnKind::Text | ColumnKind::Choice => {
            let value = row
                .try_get::<Option<String>>("", &column.name)
                .map_err(map_db)?;
            Ok(match value {
                Some(value) => Json::String(value),
                None => Json::Null,
            })
        }
        ColumnKind::Date => read_temporal(
            row,
            column,
            |date: chrono::NaiveDate| fexu_core::temporal::format_date(date),
            parse_date,
            fexu_core::temporal::format_date,
        ),
        ColumnKind::Time => read_temporal(
            row,
            column,
            |time: chrono::NaiveTime| fexu_core::temporal::format_time(time),
            parse_time,
            fexu_core::temporal::format_time,
        ),
        ColumnKind::DateTime => read_temporal(
            row,
            column,
            canonical_datetime,
            parse_datetime_utc,
            canonical_datetime,
        ),
    }
}

fn read_temporal<T, U>(
    row: &sea_orm::QueryResult,
    column: &crate::snapshot::ColumnSnapshot,
    format_native: impl FnOnce(T) -> String,
    parse: fn(&str) -> Result<U, &'static str>,
    format_parsed: impl FnOnce(U) -> String,
) -> Result<Json, MigrationError>
where
    T: sea_orm::TryGetable,
{
    if let Ok(value) = row.try_get::<Option<T>>("", &column.name) {
        return Ok(match value {
            Some(value) => Json::String(format_native(value)),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<String>>("", &column.name)
        .map_err(map_db)?;
    match value {
        None => Ok(Json::Null),
        Some(text) => {
            let parsed = parse(&text).map_err(|error| {
                MigrationError::new(format!("Field '{}': {error}", column.name))
            })?;
            Ok(Json::String(format_parsed(parsed)))
        }
    }
}

fn find_table<'a>(schema: &'a SchemaSnapshot, name: &str) -> Option<&'a TableSnapshot> {
    schema
        .tables
        .iter()
        .find(|table| table.name == name)
        .or_else(|| {
            schema
                .external_tables
                .iter()
                .find(|table| table.table.name == name)
                .map(|table| &table.table)
        })
}

fn decimal_meta(column: &crate::snapshot::ColumnSnapshot) -> Result<(u8, u8), MigrationError> {
    match (column.precision, column.scale) {
        (Some(precision), Some(scale)) => Ok((precision, scale)),
        _ => Err(MigrationError::new(format!(
            "Decimal field '{}' has no precision and scale in the schema.",
            column.name
        ))),
    }
}

async fn execute(
    db: &impl ConnectionTrait,
    backend: DatabaseBackend,
    sql: &str,
    values: Vec<Value>,
) -> Result<(), MigrationError> {
    db.execute(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map(|_| ())
        .map_err(map_db)
}

fn placeholder(backend: DatabaseBackend, index: usize) -> String {
    match backend {
        DatabaseBackend::Postgres => format!("${}", index + 1),
        _ => "?".to_owned(),
    }
}

fn quote(identifier: &str) -> String {
    format!("\"{identifier}\"")
}

fn require_identifier(name: &str) -> Result<(), MigrationError> {
    if is_identifier(name) {
        Ok(())
    } else {
        Err(MigrationError::new(format!(
            "Refusing to query non-identifier '{name}'."
        )))
    }
}

fn map_db(error: DbErr) -> MigrationError {
    MigrationError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{dump_fixture, load_fixture, parse_fixture, prepare_records, LoadRequest};
    use crate::snapshot::{
        ColumnSnapshot, ExternalTableSnapshot, ForeignKeySnapshot, LogicalType, OnDelete,
        SchemaSnapshot, StoredChoice, TableSnapshot,
    };
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

    fn column(name: &str, logical_type: LogicalType, nullable: bool) -> ColumnSnapshot {
        ColumnSnapshot {
            name: name.to_owned(),
            logical_type,
            primary_key: name == "id",
            nullable,
            default: None,
            precision: None,
            scale: None,
            choices: Vec::new(),
        }
    }

    fn foreign_key(name: &str, column: &str, table: &str) -> ForeignKeySnapshot {
        ForeignKeySnapshot {
            name: name.to_owned(),
            columns: vec![column.to_owned()],
            referenced_table: table.to_owned(),
            referenced_columns: vec!["id".to_owned()],
            on_delete: OnDelete::Restrict,
            on_update: "no_action".to_owned(),
        }
    }

    fn schema() -> SchemaSnapshot {
        let mut price = column("price", LogicalType::Decimal, false);
        price.precision = Some(12);
        price.scale = Some(2);
        let mut status = column("status", LogicalType::Choice, false);
        status.choices = vec![StoredChoice {
            value: "draft".into(),
            label_de: "Entwurf".into(),
            label_en: "Draft".into(),
        }];
        SchemaSnapshot::from_tables(vec![
            TableSnapshot {
                name: "categories".into(),
                columns: vec![
                    column("id", LogicalType::Uuid, false),
                    column("name", LogicalType::String, false),
                ],
                unique_constraints: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
            },
            TableSnapshot {
                name: "products".into(),
                columns: vec![
                    column("id", LogicalType::Uuid, false),
                    column("name", LogicalType::String, false),
                    price,
                    status,
                    column("category_id", LogicalType::Uuid, false),
                ],
                unique_constraints: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: vec![foreign_key(
                    "fk_products_category",
                    "category_id",
                    "categories",
                )],
            },
        ])
    }

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.expect("sqlite");
        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys = ON",
        ))
        .await
        .expect("foreign keys");
        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "CREATE TABLE categories (id TEXT PRIMARY KEY, name TEXT NOT NULL)",
        ))
        .await
        .expect("categories");
        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "CREATE TABLE products (id TEXT PRIMARY KEY, name TEXT NOT NULL, price BLOB NOT NULL, status TEXT NOT NULL, category_id TEXT NOT NULL REFERENCES categories(id))",
        ))
        .await
        .expect("products");
        db
    }

    fn catalog() -> &'static str {
        r#"{
          "fexu": 1,
          "records": [
            {
              "model": "products",
              "pk": "11111111-1111-4111-8111-111111111111",
              "fields": {
                "name": "Cable",
                "price": "9.50",
                "status": "draft",
                "category_id": "22222222-2222-4222-8222-222222222222"
              }
            },
            {
              "model": "categories",
              "pk": "22222222-2222-4222-8222-222222222222",
              "fields": { "name": "Hardware" }
            }
          ]
        }"#
    }

    async fn count(db: &DatabaseConnection, table: &str) -> usize {
        db.query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            format!("SELECT id FROM {table}"),
        ))
        .await
        .expect("count")
        .len()
    }

    #[test]
    fn decimal_json_numbers_and_unknown_models_fail_before_a_database() {
        assert!(parse_fixture("{").is_err());
        let number = parse_fixture(
            r#"{
              "fexu": 1,
              "records": [{
                "model": "products",
                "pk": "11111111-1111-4111-8111-111111111111",
                "fields": {
                  "name": "Cable",
                  "price": 0.1,
                  "status": "draft",
                  "category_id": "22222222-2222-4222-8222-222222222222"
                }
              }]
            }"#,
        )
        .expect("json");
        let error = prepare_records(&schema(), &number, None).unwrap_err();
        assert!(error.0.contains("JSON string"));
    }

    #[tokio::test]
    async fn import_is_ordered_idempotent_and_rolls_back() {
        let db = database().await;
        let schema = schema();
        let file = parse_fixture(catalog()).expect("json");
        let report = load_fixture(&db, &schema, &file, &LoadRequest::default())
            .await
            .expect("load");
        assert!(report.written);
        assert_eq!(report.records, 2);
        assert_eq!(count(&db, "products").await, 1);

        let again = load_fixture(&db, &schema, &file, &LoadRequest::default()).await;
        assert!(again.unwrap_err().0.contains("does not overwrite"));
        assert_eq!(count(&db, "products").await, 1);

        let dry = LoadRequest {
            dry_run: true,
            ..LoadRequest::default()
        };
        let preview = load_fixture(&db, &schema, &file, &dry).await.unwrap_err();
        assert!(preview.0.contains("does not overwrite"));
        assert_eq!(count(&db, "categories").await, 1);

        let upsert = LoadRequest {
            upsert: true,
            ..LoadRequest::default()
        };
        load_fixture(&db, &schema, &file, &upsert)
            .await
            .expect("upsert");
        assert_eq!(count(&db, "products").await, 1);

        let dumped = dump_fixture(&db, &schema, None).await.expect("dump");
        assert_eq!(dumped.records[0].model, "categories");
        assert_eq!(
            dumped.records[1]
                .fields
                .get("price")
                .and_then(|value| value.as_str()),
            Some("9.50")
        );

        let partial = parse_fixture(
            r#"{
              "fexu": 1,
              "records": [
                {
                  "model": "categories",
                  "pk": "55555555-5555-4555-8555-555555555555",
                  "fields": { "name": "Rollback" }
                },
                {
                  "model": "products",
                  "pk": "11111111-1111-4111-8111-111111111111",
                  "fields": {
                    "name": "Nope",
                    "price": "1.00",
                    "status": "draft",
                    "category_id": "55555555-5555-4555-8555-555555555555"
                  }
                }
              ]
            }"#,
        )
        .expect("partial");
        let error = load_fixture(&db, &schema, &partial, &LoadRequest::default())
            .await
            .unwrap_err();
        assert!(error.0.contains("does not overwrite"));
        assert_eq!(count(&db, "categories").await, 1);

        let missing = parse_fixture(
            r#"{
              "fexu": 1,
              "records": [{
                "model": "products",
                "pk": "33333333-3333-4333-8333-333333333333",
                "fields": {
                  "name": "Broken",
                  "price": "1.00",
                  "status": "draft",
                  "category_id": "99999999-9999-4999-8999-999999999999"
                }
              }]
            }"#,
        )
        .expect("missing");
        let error = load_fixture(&db, &schema, &missing, &LoadRequest::default())
            .await
            .unwrap_err();
        assert!(error.0.contains("missing"));
        assert_eq!(count(&db, "products").await, 1);

        let replace = LoadRequest {
            truncate: true,
            ..LoadRequest::default()
        };
        let replaced = parse_fixture(
            r#"{
              "fexu": 1,
              "records": [
                {
                  "model": "products",
                  "pk": "77777777-7777-4777-8777-777777777777",
                  "fields": {
                    "name": "Replacement",
                    "price": "2.00",
                    "status": "draft",
                    "category_id": "66666666-6666-4666-8666-666666666666"
                  }
                },
                {
                  "model": "categories",
                  "pk": "66666666-6666-4666-8666-666666666666",
                  "fields": { "name": "Only" }
                }
              ]
            }"#,
        )
        .unwrap();
        load_fixture(&db, &schema, &replaced, &replace)
            .await
            .expect("truncate");
        assert_eq!(count(&db, "categories").await, 1);
        assert_eq!(count(&db, "products").await, 1);
    }

    #[tokio::test]
    async fn cycles_unknown_files_and_external_parents_are_explicit() {
        let db = database().await;
        let schema = schema();
        let unknown =
            parse_fixture(r#"{"fexu":1,"records":[{"model":"missing","pk":"alpha","fields":{}}]}"#)
                .expect("parsed");
        assert!(
            load_fixture(&db, &schema, &unknown, &LoadRequest::default())
                .await
                .unwrap_err()
                .0
                .contains("Unknown fixture model")
        );
        assert_eq!(count(&db, "categories").await, 0);

        let mut owner = column("owner_id", LogicalType::String, false);
        owner.nullable = false;
        let nodes = SchemaSnapshot::from_tables(vec![TableSnapshot {
            name: "nodes".into(),
            columns: vec![column("id", LogicalType::String, false), owner],
            unique_constraints: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: vec![foreign_key("fk_nodes_owner", "owner_id", "nodes")],
        }]);
        let cycle = parse_fixture(
            r#"{
              "fexu": 1,
              "records": [
                {"model":"nodes","pk":"alpha","fields":{"owner_id":"beta"}},
                {"model":"nodes","pk":"beta","fields":{"owner_id":"alpha"}}
              ]
            }"#,
        )
        .unwrap();
        let error = prepare_records(&nodes, &cycle, None).unwrap_err();
        assert!(error.0.contains("cycle"));

        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO categories (id, name) VALUES ('22222222-2222-4222-8222-222222222222', 'Hardware')",
        ))
        .await
        .unwrap();
        let mut child_schema = schema.clone();
        let categories = child_schema.tables.remove(0);
        child_schema.external_tables.push(ExternalTableSnapshot {
            app: "catalog".into(),
            table: categories,
        });
        let child = parse_fixture(
            r#"{
              "fexu": 1,
              "records": [{
                "model": "products",
                "pk": "11111111-1111-4111-8111-111111111111",
                "fields": {
                  "name": "Cable",
                  "price": "9.50",
                  "status": "draft",
                  "category_id": "22222222-2222-4222-8222-222222222222"
                }
              }]
            }"#,
        )
        .unwrap();
        load_fixture(&db, &child_schema, &child, &LoadRequest::default())
            .await
            .expect("external parent");
        assert_eq!(count(&db, "products").await, 1);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL and a local PostgreSQL server"]
    async fn postgres_fixture_roundtrip() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let db = Database::connect(&url).await.expect("postgres");
        let backend = sea_orm::DatabaseBackend::Postgres;
        for statement in [
            "DROP TABLE IF EXISTS fixture_products",
            "DROP TABLE IF EXISTS fixture_categories",
            "CREATE TABLE fixture_categories (id UUID PRIMARY KEY, name TEXT NOT NULL)",
            "CREATE TABLE fixture_products (id UUID PRIMARY KEY, name TEXT NOT NULL, price BYTEA NOT NULL, status TEXT NOT NULL, category_id UUID NOT NULL REFERENCES fixture_categories(id))",
        ] {
            db.execute(Statement::from_string(backend, statement))
                .await
                .expect(statement);
        }
        let mut loaded = schema();
        loaded.tables[0].name = "fixture_categories".into();
        loaded.tables[1].name = "fixture_products".into();
        loaded.tables[1].foreign_keys[0].referenced_table = "fixture_categories".into();
        let file = parse_fixture(
            &catalog()
                .replace("categories", "fixture_categories")
                .replace("products", "fixture_products"),
        )
        .unwrap();
        load_fixture(&db, &loaded, &file, &LoadRequest::default())
            .await
            .expect("postgres load");
        let dumped = dump_fixture(&db, &loaded, None)
            .await
            .expect("postgres dump");
        assert_eq!(
            dumped.records[1]
                .fields
                .get("price")
                .and_then(|v| v.as_str()),
            Some("9.50")
        );
        let error = load_fixture(&db, &loaded, &file, &LoadRequest::default())
            .await
            .unwrap_err();
        assert!(error.0.contains("does not overwrite"));
        db.execute(Statement::from_string(
            backend,
            "DROP TABLE IF EXISTS fixture_products",
        ))
        .await
        .ok();
        db.execute(Statement::from_string(
            backend,
            "DROP TABLE IF EXISTS fixture_categories",
        ))
        .await
        .ok();
    }

    fn temporal_schema() -> SchemaSnapshot {
        let mut amount = column("amount", LogicalType::Decimal, false);
        amount.precision = Some(10);
        amount.scale = Some(2);
        SchemaSnapshot::from_tables(vec![
            TableSnapshot {
                name: "fx_owners".into(),
                columns: vec![
                    column("id", LogicalType::Uuid, false),
                    column("name", LogicalType::String, false),
                ],
                unique_constraints: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
            },
            TableSnapshot {
                name: "fx_events".into(),
                columns: vec![
                    column("id", LogicalType::Uuid, false),
                    column("owner_id", LogicalType::Uuid, false),
                    column("event_date", LogicalType::Date, false),
                    column("start_time", LogicalType::Time, false),
                    column("created_at", LogicalType::DateTime, false),
                    column("closed_on", LogicalType::Date, true),
                    column("ends_at", LogicalType::Time, true),
                    column("updated_at", LogicalType::DateTime, true),
                    amount,
                ],
                unique_constraints: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: vec![foreign_key("fk_fx_events_owner", "owner_id", "fx_owners")],
            },
        ])
    }

    fn temporal_fixture() -> &'static str {
        r#"{
          "fexu": 1,
          "records": [
            {
              "model": "fx_owners",
              "pk": "22222222-2222-4222-8222-222222222222",
              "fields": { "name": "Ada" }
            },
            {
              "model": "fx_events",
              "pk": "11111111-1111-4111-8111-111111111111",
              "fields": {
                "owner_id": "22222222-2222-4222-8222-222222222222",
                "event_date": "2026-11-11",
                "start_time": "14:30",
                "created_at": "2026-11-11T13:30:00.123456Z",
                "closed_on": null,
                "ends_at": null,
                "updated_at": "2026-11-11T13:30:00+00:00",
                "amount": "12.50"
              }
            }
          ]
        }"#
    }

    fn assert_temporal(file: &super::FixtureFile) {
        let event = file
            .records
            .iter()
            .find(|record| record.model == "fx_events")
            .unwrap();
        assert_eq!(event.pk, "11111111-1111-4111-8111-111111111111");
        assert_eq!(
            event
                .fields
                .get("event_date")
                .and_then(|value| value.as_str()),
            Some("2026-11-11")
        );
        assert_eq!(
            event
                .fields
                .get("start_time")
                .and_then(|value| value.as_str()),
            Some("14:30:00")
        );
        assert_eq!(
            event
                .fields
                .get("created_at")
                .and_then(|value| value.as_str()),
            Some("2026-11-11T13:30:00.123456Z")
        );
        assert!(event.fields.get("closed_on").unwrap().is_null());
        assert!(event.fields.get("ends_at").unwrap().is_null());
        assert_eq!(
            event
                .fields
                .get("updated_at")
                .and_then(|value| value.as_str()),
            Some("2026-11-11T13:30:00Z")
        );
        assert_eq!(
            event.fields.get("amount").and_then(|value| value.as_str()),
            Some("12.50")
        );
        assert_eq!(
            event
                .fields
                .get("owner_id")
                .and_then(|value| value.as_str()),
            Some("22222222-2222-4222-8222-222222222222")
        );
    }

    async fn create_temporal_tables(db: &DatabaseConnection) {
        let statements: &[&str] = match db.get_database_backend() {
            sea_orm::DatabaseBackend::Sqlite => &[
                "CREATE TABLE fx_owners (id TEXT PRIMARY KEY, name TEXT NOT NULL)",
                "CREATE TABLE fx_events (
                    id TEXT PRIMARY KEY,
                    owner_id TEXT NOT NULL REFERENCES fx_owners(id),
                    event_date TEXT NOT NULL,
                    start_time TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    closed_on TEXT,
                    ends_at TEXT,
                    updated_at TEXT,
                    amount BLOB NOT NULL
                )",
            ],
            sea_orm::DatabaseBackend::Postgres => &[
                "CREATE TABLE fx_owners (id UUID PRIMARY KEY, name TEXT NOT NULL)",
                "CREATE TABLE fx_events (
                    id UUID PRIMARY KEY,
                    owner_id UUID NOT NULL REFERENCES fx_owners(id),
                    event_date DATE NOT NULL,
                    start_time TIME NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL,
                    closed_on DATE,
                    ends_at TIME,
                    updated_at TIMESTAMPTZ,
                    amount BYTEA NOT NULL
                )",
            ],
            sea_orm::DatabaseBackend::MySql => panic!("mysql is not a test backend"),
        };
        for statement in statements {
            db.execute(Statement::from_string(
                db.get_database_backend(),
                (*statement).to_owned(),
            ))
            .await
            .expect(statement);
        }
    }

    #[test]
    fn temporal_fixture_rejects_an_invalid_date_before_writing() {
        let file = parse_fixture(
            r#"{
              "fexu": 1,
              "records": [{
                "model": "fx_events",
                "pk": "11111111-1111-4111-8111-111111111111",
                "fields": {
                  "owner_id": "22222222-2222-4222-8222-222222222222",
                  "event_date": "2026-02-31",
                  "start_time": "14:30:00",
                  "created_at": "2026-11-11T13:30:00Z",
                  "amount": "12.50"
                }
              }]
            }"#,
        )
        .unwrap();
        let error = prepare_records(&temporal_schema(), &file, None).unwrap_err();
        assert!(error.0.contains("validation.date"));
    }

    #[tokio::test]
    async fn sqlite_temporal_fixture_roundtrip_keeps_canonical_values() {
        let db = database().await;
        create_temporal_tables(&db).await;
        let schema = temporal_schema();
        let file = parse_fixture(temporal_fixture()).unwrap();
        load_fixture(&db, &schema, &file, &LoadRequest::default())
            .await
            .unwrap();
        let dumped = dump_fixture(&db, &schema, None).await.unwrap();
        assert_temporal(&dumped);
        load_fixture(
            &db,
            &schema,
            &dumped,
            &LoadRequest {
                truncate: true,
                ..LoadRequest::default()
            },
        )
        .await
        .unwrap();
        let again = dump_fixture(&db, &schema, None).await.unwrap();
        assert_temporal(&again);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL, CREATEDB, and an approved PostgreSQL test database"]
    async fn postgres_temporal_fixture_roundtrip() {
        let sqlite = database().await;
        create_temporal_tables(&sqlite).await;
        let schema = temporal_schema();
        let file = parse_fixture(temporal_fixture()).unwrap();
        load_fixture(&sqlite, &schema, &file, &LoadRequest::default())
            .await
            .unwrap();
        let from_sqlite = dump_fixture(&sqlite, &schema, None).await.unwrap();
        assert_temporal(&from_sqlite);

        let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let disposable = fexu_db::DisposablePostgres::create(&admin)
            .await
            .expect("isolated postgres");
        let postgres = fexu_db::connect(disposable.url()).await.expect("postgres");
        create_temporal_tables(&postgres).await;
        load_fixture(&postgres, &schema, &from_sqlite, &LoadRequest::default())
            .await
            .expect("postgres load");
        let from_postgres = dump_fixture(&postgres, &schema, None)
            .await
            .expect("postgres dump");
        assert_temporal(&from_postgres);
        load_fixture(
            &postgres,
            &schema,
            &from_postgres,
            &LoadRequest {
                truncate: true,
                ..LoadRequest::default()
            },
        )
        .await
        .expect("postgres reload");
        let reloaded = dump_fixture(&postgres, &schema, None)
            .await
            .expect("postgres reload dump");
        assert_temporal(&reloaded);
        postgres.close().await.expect("close");
        disposable
            .drop_database()
            .await
            .expect("drop isolated database");
    }
}
