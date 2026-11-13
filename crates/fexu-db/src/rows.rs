//! Row values shared by the PostgreSQL and SQLite adapters.

use crate::error::DbErr;
use crate::sqlite::{CellValue, SqliteRecord};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::Value as Json;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) enum Cell {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Text(String),
    Bytes(Vec<u8>),
    Json(Json),
    Uuid(Uuid),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime(NaiveDateTime),
    TimestampTz(DateTime<Utc>),
    Numeric(Decimal),
}

#[derive(Clone, Debug)]
pub struct QueryResult {
    columns: Vec<(String, Cell)>,
}

impl QueryResult {
    pub(crate) fn from_cells(columns: Vec<(String, Cell)>) -> Self {
        Self { columns }
    }

    pub(crate) fn from_sqlite(record: SqliteRecord) -> Self {
        Self {
            columns: record
                .columns
                .into_iter()
                .map(|(name, value)| (name, cell_from_sqlite(value)))
                .collect(),
        }
    }

    pub fn try_get_by_index<T: TryGetable>(&self, index: usize) -> Result<T, DbErr> {
        match self.columns.get(index) {
            Some((name, _)) => T::from_row(self, name),
            None => Err(DbErr::query(format!("column index {index} is missing"))),
        }
    }

    pub fn try_get<T: TryGetable>(&self, prefix: &str, column: &str) -> Result<T, DbErr> {
        let name = format!("{prefix}{column}");
        T::from_row(self, &name)
    }

    fn find(&self, column: &str) -> Option<&Cell> {
        self.columns
            .iter()
            .find(|(name, _)| name == column)
            .or_else(|| {
                self.columns
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(column))
            })
            .map(|(_, value)| value)
    }
}

pub trait TryGetable: Sized {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr>;
}

impl<T: TryGetable> TryGetable for Option<T> {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match row.find(column) {
            None | Some(Cell::Null) => Ok(None),
            Some(_) => T::from_row(row, column).map(Some),
        }
    }
}

impl TryGetable for i64 {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Int(value) => Ok(*value),
            _ => Err(mismatch(column, "i64")),
        }
    }
}

impl TryGetable for i32 {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        let value = i64::from_row(row, column)?;
        i32::try_from(value).map_err(|_| mismatch(column, "i32"))
    }
}

impl TryGetable for bool {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Bool(value) => Ok(*value),
            Cell::Int(value) => Ok(*value != 0),
            _ => Err(mismatch(column, "bool")),
        }
    }
}

impl TryGetable for f64 {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Real(value) => Ok(*value),
            Cell::Int(value) => Ok(*value as f64),
            _ => Err(mismatch(column, "f64")),
        }
    }
}

impl TryGetable for String {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Text(value) => Ok(value.clone()),
            _ => Err(mismatch(column, "text")),
        }
    }
}

impl TryGetable for Vec<u8> {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Bytes(value) => Ok(value.clone()),
            _ => Err(mismatch(column, "blob")),
        }
    }
}

impl TryGetable for Uuid {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Uuid(value) => Ok(*value),
            Cell::Bytes(value) => Uuid::from_slice(value).map_err(|_| mismatch(column, "uuid")),
            Cell::Text(value) => Uuid::parse_str(value).map_err(|_| mismatch(column, "uuid")),
            _ => Err(mismatch(column, "uuid")),
        }
    }
}

impl TryGetable for Json {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Json(value) => Ok(value.clone()),
            Cell::Text(value) => serde_json::from_str(value).map_err(|_| mismatch(column, "json")),
            _ => Err(mismatch(column, "json")),
        }
    }
}

impl TryGetable for NaiveDate {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Date(value) => Ok(*value),
            Cell::Text(value) => {
                NaiveDate::parse_from_str(value, "%F").map_err(|_| mismatch(column, "date"))
            }
            _ => Err(mismatch(column, "date")),
        }
    }
}

impl TryGetable for NaiveTime {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Time(value) => Ok(*value),
            Cell::Text(value) => NaiveTime::parse_from_str(value, "%T%.f")
                .or_else(|_| NaiveTime::parse_from_str(value, "%T"))
                .map_err(|_| mismatch(column, "time")),
            _ => Err(mismatch(column, "time")),
        }
    }
}

impl TryGetable for NaiveDateTime {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::DateTime(value) => Ok(*value),
            Cell::TimestampTz(value) => Ok(value.naive_utc()),
            Cell::Text(value) => decode_datetime(value)
                .map(|value| value.naive_utc())
                .ok_or_else(|| mismatch(column, "datetime")),
            _ => Err(mismatch(column, "datetime")),
        }
    }
}

impl TryGetable for DateTime<Utc> {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::TimestampTz(value) => Ok(*value),
            Cell::DateTime(value) => Ok(Utc.from_utc_datetime(value)),
            Cell::Text(value) => decode_datetime(value)
                .map(|value| Utc.from_utc_datetime(&value.naive_utc()))
                .ok_or_else(|| mismatch(column, "datetime")),
            Cell::Int(seconds) => Utc
                .timestamp_opt(*seconds, 0)
                .single()
                .ok_or_else(|| mismatch(column, "datetime")),
            Cell::Real(value) => decode_julian(*value).ok_or_else(|| mismatch(column, "datetime")),
            _ => Err(mismatch(column, "datetime")),
        }
    }
}

impl TryGetable for Decimal {
    fn from_row(row: &QueryResult, column: &str) -> Result<Self, DbErr> {
        match required(row, column)? {
            Cell::Numeric(value) => Ok(*value),
            Cell::Text(value) => value.parse().map_err(|_| mismatch(column, "numeric")),
            _ => Err(mismatch(column, "numeric")),
        }
    }
}

fn required<'a>(row: &'a QueryResult, column: &str) -> Result<&'a Cell, DbErr> {
    match row.find(column) {
        None => Err(DbErr::query(format!("no column {column}"))),
        Some(Cell::Null) => Err(DbErr::query(format!("null value in {column}"))),
        Some(value) => Ok(value),
    }
}

fn mismatch(column: &str, expected: &str) -> DbErr {
    DbErr::query(format!("column {column} is not a {expected}"))
}

fn cell_from_sqlite(value: CellValue) -> Cell {
    match value {
        CellValue::Null => Cell::Null,
        CellValue::Integer(value) => Cell::Int(value),
        CellValue::Real(value) => Cell::Real(value),
        CellValue::Text(value) => Cell::Text(value),
        CellValue::Blob(value) => Cell::Bytes(value),
    }
}

pub(crate) fn cell_null() -> Cell {
    Cell::Null
}

pub(crate) fn cell_bool(value: bool) -> Cell {
    Cell::Bool(value)
}

pub(crate) fn cell_int(value: i64) -> Cell {
    Cell::Int(value)
}

pub(crate) fn cell_real(value: f64) -> Cell {
    Cell::Real(value)
}

pub(crate) fn cell_text(value: String) -> Cell {
    Cell::Text(value)
}

pub(crate) fn cell_bytes(value: Vec<u8>) -> Cell {
    Cell::Bytes(value)
}

pub(crate) fn cell_json(value: Json) -> Cell {
    Cell::Json(value)
}

pub(crate) fn cell_uuid(value: Uuid) -> Cell {
    Cell::Uuid(value)
}

pub(crate) fn cell_date(value: NaiveDate) -> Cell {
    Cell::Date(value)
}

pub(crate) fn cell_time(value: NaiveTime) -> Cell {
    Cell::Time(value)
}

pub(crate) fn cell_datetime(value: NaiveDateTime) -> Cell {
    Cell::DateTime(value)
}

pub(crate) fn cell_timestamptz(value: DateTime<Utc>) -> Cell {
    Cell::TimestampTz(value)
}

pub(crate) fn cell_numeric(value: Decimal) -> Cell {
    Cell::Numeric(value)
}

fn decode_datetime(value: &str) -> Option<DateTime<FixedOffset>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Some(parsed);
    }
    let formats = ["%F %T%.f", "%F %R", "%F %T%.fZ", "%FT%T%.f", "%FT%T%.fZ"];
    for format in formats {
        if let Ok(parsed) = DateTime::parse_from_str(value, format) {
            return Some(parsed);
        }
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return Some(parsed.and_utc().fixed_offset());
        }
    }
    None
}

fn decode_julian(value: f64) -> Option<DateTime<Utc>> {
    let timestamp = (value - 2_440_587.5) * 86_400.0;
    if !timestamp.is_finite() {
        return None;
    }
    let seconds = timestamp.trunc() as i64;
    let nanos = (timestamp.fract() * 1_000_000_000.0).abs() as u32;
    Utc.timestamp_opt(seconds, nanos).single()
}
