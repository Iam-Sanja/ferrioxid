//! Kalenderdatum, Tageszeit und UTC-Zeitpunkt. Keine stillen Zeitzonenverschiebungen.

use chrono::{DateTime, NaiveDate, NaiveTime, SecondsFormat, Timelike, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateBound {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

pub fn parse_date(value: &str) -> Result<NaiveDate, &'static str> {
    if value.is_empty() {
        return Err("validation.required");
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "validation.date")
}

pub fn parse_time(value: &str) -> Result<NaiveTime, &'static str> {
    if value.is_empty() {
        return Err("validation.required");
    }
    NaiveTime::parse_from_str(value, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(value, "%H:%M"))
        .map_err(|_| "validation.time")
}

/// Nur UTC. Andere Offsets werden abgelehnt, nicht umgerechnet.
pub fn parse_datetime_utc(value: &str) -> Result<DateTime<Utc>, &'static str> {
    if value.is_empty() {
        return Err("validation.required");
    }
    let parsed = DateTime::parse_from_rfc3339(value).map_err(|_| "validation.datetime")?;
    if parsed.offset().local_minus_utc() != 0 {
        return Err("validation.datetime_offset");
    }
    Ok(parsed.with_timezone(&Utc))
}

/// `datetime-local` ohne Zeitzone. Der Wert gilt als UTC, weil das Feld das ausdrücklich sagt.
pub fn parse_datetime_local(value: &str) -> Result<DateTime<Utc>, &'static str> {
    if value.is_empty() {
        return Err("validation.required");
    }
    if value.contains('Z') || value.contains('+') || value.matches('-').count() > 2 {
        return Err("validation.datetime_offset");
    }
    let naive = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))
        .map_err(|_| "validation.datetime")?;
    Ok(naive.and_utc())
}

pub fn format_date(value: NaiveDate) -> String {
    value.format("%Y-%m-%d").to_string()
}

pub fn format_time(value: NaiveTime) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        value.hour(),
        value.minute(),
        value.second()
    )
}

pub fn format_datetime(value: DateTime<Utc>) -> String {
    if value.timestamp_subsec_nanos() == 0 {
        value.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    } else {
        value.to_rfc3339_opts(SecondsFormat::AutoSi, true)
    }
}

pub fn datetime_local_value(value: &str) -> String {
    value.trim_end_matches('Z').to_owned()
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_date(&value).map_err(serde::de::Error::custom)
}

pub fn deserialize_time<'de, D>(deserializer: D) -> Result<NaiveTime, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_time(&value).map_err(serde::de::Error::custom)
}

pub fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_datetime_utc(&value).map_err(serde::de::Error::custom)
}

pub fn deserialize_optional_date<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    optional(deserializer, parse_date)
}

pub fn deserialize_optional_time<'de, D>(deserializer: D) -> Result<Option<NaiveTime>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    optional(deserializer, parse_time)
}

pub fn deserialize_optional_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    optional(deserializer, parse_datetime_utc)
}

fn optional<'de, D, T>(
    deserializer: D,
    parse: fn(&str) -> Result<T, &'static str>,
) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        None => Ok(None),
        Some(text) if text.is_empty() => Ok(None),
        Some(text) => parse(&text).map(Some).map_err(serde::de::Error::custom),
    }
}

pub fn hierarchy_bound(
    year: i32,
    month: Option<u32>,
    day: Option<u32>,
) -> Result<DateBound, &'static str> {
    let start = match (month, day) {
        (None, None) => NaiveDate::from_ymd_opt(year, 1, 1).ok_or("validation.date")?,
        (Some(month), None) => NaiveDate::from_ymd_opt(year, month, 1).ok_or("validation.date")?,
        (Some(month), Some(day)) => {
            NaiveDate::from_ymd_opt(year, month, day).ok_or("validation.date")?
        }
        (None, Some(_)) => return Err("validation.date"),
    };
    let end = if day.is_some() {
        start.succ_opt().ok_or("validation.date")?
    } else if let Some(month) = month {
        if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1).ok_or("validation.date")?
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1).ok_or("validation.date")?
        }
    } else {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).ok_or("validation.date")?
    };
    Ok(DateBound { start, end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_day_and_invalid_dates_stay_on_the_calendar() {
        assert!(parse_date("2024-02-29").is_ok());
        assert_eq!(parse_date("2023-02-29"), Err("validation.date"));
        assert_eq!(parse_date("2026-11-07").unwrap().to_string(), "2026-11-07");
    }

    #[test]
    fn time_and_utc_reject_offsets() {
        assert_eq!(format_time(parse_time("14:30").unwrap()), "14:30:00");
        assert_eq!(parse_time("24:00:00"), Err("validation.time"));
        let instant = parse_datetime_utc("2026-11-07T13:30:00Z").unwrap();
        assert_eq!(format_datetime(instant), "2026-11-07T13:30:00Z");
        let fractional = parse_datetime_utc("2026-11-07T13:30:00.123456Z").unwrap();
        assert_eq!(format_datetime(fractional), "2026-11-07T13:30:00.123456Z");
        assert_eq!(
            parse_datetime_utc("2026-11-07T13:30:00+02:00"),
            Err("validation.datetime_offset")
        );
        assert_eq!(
            format_datetime(parse_datetime_local("2026-11-07T13:30").unwrap()),
            "2026-11-07T13:30:00Z"
        );
        assert_eq!(
            format_datetime(parse_datetime_utc("2026-11-07T00:00:00Z").unwrap()),
            "2026-11-07T00:00:00Z"
        );
        assert_eq!(format_time(parse_time("00:00").unwrap()), "00:00:00");
        assert_eq!(format_time(parse_time("23:59:59").unwrap()), "23:59:59");
    }

    #[test]
    fn hierarchy_bounds_stop_before_the_next_day_month_and_year() {
        let day = hierarchy_bound(2026, Some(11), Some(7)).unwrap();
        assert_eq!(day.start.to_string(), "2026-11-07");
        assert_eq!(day.end.to_string(), "2026-11-08");
        let year = hierarchy_bound(2026, None, None).unwrap();
        assert_eq!(year.end.to_string(), "2027-01-01");
        let december = hierarchy_bound(2026, Some(12), None).unwrap();
        assert_eq!(december.end.to_string(), "2027-01-01");
        let leap = hierarchy_bound(2024, Some(2), Some(29)).unwrap();
        assert_eq!(leap.end.to_string(), "2024-03-01");
        assert_eq!(
            hierarchy_bound(2023, Some(2), Some(29)),
            Err("validation.date")
        );
    }
}
