//! Fünf-Feld-Cron in UTC, einer festen Verschiebung oder Europe/Berlin.
//!
//! Namen wie `MON` sind kein Ausdruck. Eine Lücke der Sommerzeit wird übersprungen.
//! Eine wiederholte Stunde löst einmal aus, und zwar zum früheren UTC-Zeitpunkt.
//! Verpasste Zeitpunkte werden nicht nachgeholt.

use crate::error::JobError;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Timelike, Utc, Weekday};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CronExpr {
    minute: [bool; 60],
    hour: [bool; 24],
    day: [bool; 32],
    month: [bool; 13],
    weekday: [bool; 7],
    day_any: bool,
    weekday_any: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleZone {
    Utc,
    Fixed { offset_secs: i32 },
    EuropeBerlin,
}

impl ScheduleZone {
    pub fn canonical(&self) -> String {
        match self {
            Self::Utc => "utc".to_owned(),
            Self::Fixed { offset_secs } => format!("fixed{}", format_offset(*offset_secs)),
            Self::EuropeBerlin => "europe/berlin".to_owned(),
        }
    }
}

pub fn parse_zone(value: &str) -> Result<ScheduleZone, JobError> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("utc") {
        return Ok(ScheduleZone::Utc);
    }
    if value.eq_ignore_ascii_case("europe/berlin") {
        return Ok(ScheduleZone::EuropeBerlin);
    }
    let Some(offset) = value
        .strip_prefix("fixed")
        .or_else(|| value.strip_prefix("FIXED"))
    else {
        return Err(schedule(
            "Die Zeitzone ist utc, europe/berlin oder fixed+HH:MM.",
        ));
    };
    Ok(ScheduleZone::Fixed {
        offset_secs: parse_offset(offset)?,
    })
}

pub fn parse_cron(expression: &str) -> Result<CronExpr, JobError> {
    let fields: Vec<_> = expression.split_whitespace().collect();
    if fields.len() != 5 || expression.len() > 64 {
        return Err(schedule(
            "Ein Cron-Ausdruck hat fünf Felder: Minute Stunde Tag Monat Wochentag.",
        ));
    }
    Ok(CronExpr {
        minute: field(fields[0], 0, 59)?,
        hour: field(fields[1], 0, 23)?,
        day: field_day(fields[2])?,
        month: field_month(fields[3])?,
        weekday: field_week(fields[4])?.0,
        day_any: fields[2] == "*",
        weekday_any: fields[4] == "*",
    })
}

pub fn next_after(expression: &str, zone: &str, after_unix: i64) -> Result<i64, JobError> {
    let expression = parse_cron(expression)?;
    let zone = parse_zone(zone)?;
    let start = to_local(&zone, after_unix)?;
    let mut cursor = start
        .checked_add_signed(Duration::minutes(1))
        .ok_or_else(|| schedule("Der nächste Zeitpunkt liegt außerhalb des Kalenders."))?;
    cursor = cursor.with_second(0).unwrap_or(cursor);
    for _ in 0..(366 * 24 * 60) {
        match local_to_utc(&zone, cursor) {
            LocalHit::Gap => {}
            LocalHit::At(unix) if unix > after_unix && expression.matches(cursor) => {
                return Ok(unix);
            }
            LocalHit::At(_) => {}
        }
        cursor = cursor
            .checked_add_signed(Duration::minutes(1))
            .ok_or_else(|| schedule("Der nächste Zeitpunkt liegt außerhalb des Kalenders."))?;
    }
    Err(schedule(
        "In den nächsten 366 Tagen gibt es keinen passenden Zeitpunkt.",
    ))
}

impl CronExpr {
    fn matches(&self, civil: NaiveDateTime) -> bool {
        if !self.minute[civil.minute() as usize]
            || !self.hour[civil.hour() as usize]
            || !self.month[civil.month() as usize]
        {
            return false;
        }
        let day_ok = self.day[civil.day() as usize];
        let week_ok = self.weekday[weekday_index(civil.weekday())];
        match (self.day_any, self.weekday_any) {
            (true, true) => true,
            (false, true) => day_ok,
            (true, false) => week_ok,
            (false, false) => day_ok || week_ok,
        }
    }
}

enum LocalHit {
    Gap,
    At(i64),
}

fn unix_naive(unix: i64) -> Result<NaiveDateTime, JobError> {
    DateTime::<Utc>::from_timestamp(unix, 0)
        .map(|value| value.naive_utc())
        .ok_or_else(|| schedule("Der Zeitpunkt liegt außerhalb des Kalenders."))
}

fn to_local(zone: &ScheduleZone, unix: i64) -> Result<NaiveDateTime, JobError> {
    let utc = unix_naive(unix)?;
    utc.checked_add_signed(Duration::seconds(i64::from(offset_at(zone, unix))))
        .ok_or_else(|| schedule("Der Zeitpunkt liegt außerhalb des Kalenders."))
}

fn local_to_utc(zone: &ScheduleZone, civil: NaiveDateTime) -> LocalHit {
    match zone {
        ScheduleZone::Utc => LocalHit::At(civil.and_utc().timestamp()),
        ScheduleZone::Fixed { offset_secs } => {
            LocalHit::At(civil.and_utc().timestamp() - i64::from(*offset_secs))
        }
        ScheduleZone::EuropeBerlin => {
            let mut hits = Vec::new();
            for offset in [3600, 7200] {
                let unix = civil.and_utc().timestamp() - i64::from(offset);
                if offset_at(zone, unix) == offset {
                    hits.push(unix);
                }
            }
            match hits.iter().min() {
                Some(unix) => LocalHit::At(*unix),
                None => LocalHit::Gap,
            }
        }
    }
}

fn offset_at(zone: &ScheduleZone, unix: i64) -> i32 {
    match zone {
        ScheduleZone::Utc => 0,
        ScheduleZone::Fixed { offset_secs } => *offset_secs,
        ScheduleZone::EuropeBerlin => {
            let year = to_year(unix);
            if unix >= dst_start(year) && unix < dst_end(year) {
                7200
            } else {
                3600
            }
        }
    }
}

fn to_year(unix: i64) -> i32 {
    unix_naive(unix).map(|value| value.year()).unwrap_or(1970)
}

fn dst_start(year: i32) -> i64 {
    last_sunday(year, 3)
        .and_hms_opt(1, 0, 0)
        .expect("dst start")
        .and_utc()
        .timestamp()
}

fn dst_end(year: i32) -> i64 {
    last_sunday(year, 10)
        .and_hms_opt(1, 0, 0)
        .expect("dst end")
        .and_utc()
        .timestamp()
}

fn last_sunday(year: i32, month: u32) -> NaiveDate {
    let mut day = NaiveDate::from_ymd_opt(year, month, 31).expect("month end");
    while day.weekday() != Weekday::Sun {
        day = day.pred_opt().expect("previous day");
    }
    day
}

fn weekday_index(day: Weekday) -> usize {
    day.num_days_from_sunday() as usize
}

fn field<const N: usize>(value: &str, min: u32, max: u32) -> Result<[bool; N], JobError> {
    let mut slots = [false; N];
    fill(&mut slots, value, min, max)?;
    Ok(slots)
}

fn field_day(value: &str) -> Result<[bool; 32], JobError> {
    let mut slots = [false; 32];
    fill(&mut slots[1..], value, 1, 31)?;
    Ok(slots)
}

fn field_month(value: &str) -> Result<[bool; 13], JobError> {
    let mut slots = [false; 13];
    fill(&mut slots[1..], value, 1, 12)?;
    Ok(slots)
}

fn field_week(value: &str) -> Result<([bool; 7], bool), JobError> {
    let mut slots = [false; 7];
    if value == "*" {
        slots.fill(true);
        return Ok((slots, true));
    }
    for part in value.split(',') {
        let (start, end, step) = range(part, 0, 7)?;
        let mut current = start;
        while current <= end {
            let index = if current == 7 { 0 } else { current };
            slots[index as usize] = true;
            current = current.saturating_add(step);
            if step == 0 {
                break;
            }
        }
    }
    Ok((slots, false))
}

fn fill(slots: &mut [bool], value: &str, min: u32, max: u32) -> Result<(), JobError> {
    if value == "*" {
        slots.fill(true);
        return Ok(());
    }
    for part in value.split(',') {
        let (start, end, step) = range(part, min, max)?;
        let mut current = start;
        while current <= end {
            let index =
                usize::try_from(current - min).map_err(|_| schedule("Feld ist ungültig."))?;
            if index >= slots.len() {
                return Err(schedule("Feld liegt außerhalb des erlaubten Bereichs."));
            }
            slots[index] = true;
            let next = current.saturating_add(step);
            if next == current {
                break;
            }
            current = next;
        }
    }
    if slots.iter().any(|slot| *slot) {
        Ok(())
    } else {
        Err(schedule("Feld enthält keinen Wert."))
    }
}

fn range(part: &str, min: u32, max: u32) -> Result<(u32, u32, u32), JobError> {
    if part.is_empty()
        || !part
            .chars()
            .all(|char| char.is_ascii_digit() || char == '-' || char == '/' || char == '*')
    {
        return Err(schedule(
            "Cron-Felder enthalten Zahlen, *, Komma, Bindestrich und Schrägstrich.",
        ));
    }
    let (body, step) = match part.split_once('/') {
        Some((body, step)) => (body, number(step, 1, 59)?),
        None => (part, 1),
    };
    let (start, end) = if body == "*" {
        (min, max)
    } else if let Some((start, end)) = body.split_once('-') {
        (number(start, min, max)?, number(end, min, max)?)
    } else {
        let value = number(body, min, max)?;
        (value, value)
    };
    if start > end {
        return Err(schedule("Ein Bereich darf nicht rückwärts laufen."));
    }
    Ok((start, end, step))
}

fn number(value: &str, min: u32, max: u32) -> Result<u32, JobError> {
    let parsed: u32 = value
        .parse()
        .map_err(|_| schedule("Ein Cron-Feld ist keine Zahl."))?;
    if (min..=max).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(schedule(
            "Ein Cron-Feld liegt außerhalb des erlaubten Bereichs.",
        ))
    }
}

fn parse_offset(value: &str) -> Result<i32, JobError> {
    let (sign, rest) = if let Some(rest) = value.strip_prefix('+') {
        (1, rest)
    } else if let Some(rest) = value.strip_prefix('-') {
        (-1, rest)
    } else {
        return Err(schedule("fixed braucht +HH:MM oder -HH:MM."));
    };
    let Some((hour, minute)) = rest.split_once(':') else {
        return Err(schedule("fixed braucht +HH:MM oder -HH:MM."));
    };
    let hour: i32 = hour
        .parse()
        .map_err(|_| schedule("Die Stundenverschiebung ist ungültig."))?;
    let minute: i32 = minute
        .parse()
        .map_err(|_| schedule("Die Minutenverschiebung ist ungültig."))?;
    if !(0..=14).contains(&hour) || !(0..=59).contains(&minute) || (hour == 14 && minute != 0) {
        return Err(schedule(
            "Die Verschiebung liegt zwischen -14:00 und +14:00.",
        ));
    }
    Ok(sign * (hour * 3600 + minute * 60))
}

fn format_offset(offset: i32) -> String {
    let sign = if offset < 0 { '-' } else { '+' };
    let absolute = offset.abs();
    format!("{sign}{:02}:{:02}", absolute / 3600, (absolute % 3600) / 60)
}

fn schedule(detail: &str) -> JobError {
    JobError::message("job.schedule", detail)
}

#[cfg(test)]
mod tests {
    use super::{next_after, parse_cron};
    use chrono::{NaiveDate, NaiveTime};

    fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
        NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_time(NaiveTime::from_hms_opt(hour, minute, 0).unwrap())
            .and_utc()
            .timestamp()
    }

    #[test]
    fn weekdays_and_names_follow_the_written_rules() {
        let expr = parse_cron("0 9 * * 1-5").unwrap();
        let monday = NaiveDate::from_ymd_opt(2026, 11, 16)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let sunday = NaiveDate::from_ymd_opt(2026, 11, 15)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        assert!(expr.matches(monday));
        assert!(!expr.matches(sunday));
        assert!(parse_cron("0 9 * * MON").is_err());
        assert!(parse_cron("*/0 * * * *").is_err());
        let either = parse_cron("0 0 1 * 1").unwrap();
        let first = NaiveDate::from_ymd_opt(2026, 11, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let monday_midnight = NaiveDate::from_ymd_opt(2026, 11, 16)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let other = NaiveDate::from_ymd_opt(2026, 11, 3)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        assert!(either.matches(first));
        assert!(either.matches(monday_midnight));
        assert!(!either.matches(other));
    }

    #[test]
    fn europe_berlin_skips_the_gap_and_fires_the_fold_once() {
        let before_gap = utc(2026, 3, 28, 22, 0);
        let next = next_after("30 2 * * *", "europe/berlin", before_gap).unwrap();
        let gap_day = utc(2026, 3, 29, 0, 30);
        assert_ne!(next, gap_day);
        let morning = next_after("0 3 * * *", "europe/berlin", before_gap).unwrap();
        assert_eq!(morning, utc(2026, 3, 29, 1, 0));
        let before_fold = utc(2026, 10, 24, 22, 0);
        let folded = next_after("30 2 * * *", "europe/berlin", before_fold).unwrap();
        assert_eq!(folded, utc(2026, 10, 25, 0, 30));
        let again = next_after("30 2 * * *", "europe/berlin", folded).unwrap();
        assert!(again > utc(2026, 10, 25, 2, 0));
    }

    #[test]
    fn fixed_offset_is_not_treated_as_utc() {
        let after = utc(2026, 1, 1, 7, 0);
        let next = next_after("0 9 * * *", "fixed+01:00", after).unwrap();
        assert_eq!(next, utc(2026, 1, 1, 8, 0));
        assert!(next_after("0 9 * * *", "America/New_York", after).is_err());
    }
}
