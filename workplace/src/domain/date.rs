use time::macros::format_description;
use time::{Date, Month, OffsetDateTime, UtcOffset};

pub const ISO_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day]");

/// The shop operates on IST (Asia/Kolkata). Civil dates are therefore derived
/// from IST, not UTC, so a late-evening entry never lands on the wrong day.
pub const SHOP_UTC_OFFSET_SECONDS: i32 = 5 * 3600 + 30 * 60;

/// Longest range accepted by report/export endpoints, as a work bound.
pub const MAX_RANGE_DAYS: i64 = 366;

pub fn parse_iso(s: &str) -> Option<Date> {
    if s.len() != 10 {
        return None;
    }
    Date::parse(s, ISO_FORMAT).ok()
}

pub fn iso(d: Date) -> String {
    d.format(ISO_FORMAT).unwrap_or_default()
}

/// `dd-mm-yyyy`, the human format used in reports and the UI.
pub fn display(d: Date) -> String {
    format!("{:02}-{:02}-{:04}", d.day(), u8::from(d.month()), d.year())
}

pub fn today_ist() -> Date {
    let offset = UtcOffset::from_whole_seconds(SHOP_UTC_OFFSET_SECONDS).unwrap_or(UtcOffset::UTC);
    OffsetDateTime::now_utc().to_offset(offset).date()
}

/// First and last day of a calendar month, or `None` for an invalid month.
pub fn month_range(year: i32, month: u8) -> Option<(Date, Date)> {
    let month = Month::try_from(month).ok()?;
    let first = Date::from_calendar_date(year, month, 1).ok()?;
    let next_first = if month == Month::December {
        Date::from_calendar_date(year + 1, Month::January, 1).ok()?
    } else {
        Date::from_calendar_date(year, Month::try_from(u8::from(month) + 1).ok()?, 1).ok()?
    };
    let last = next_first.previous_day()?;
    Some((first, last))
}

pub fn days_inclusive(from: Date, to: Date) -> i64 {
    (to.to_julian_day() as i64) - (from.to_julian_day() as i64) + 1
}

/// Format a Unix-millisecond timestamp as an IST `dd-mm-yyyy HH:MM` string for
/// report headers.
pub fn format_datetime_ist(ms: i64) -> String {
    let offset = UtcOffset::from_whole_seconds(SHOP_UTC_OFFSET_SECONDS).unwrap_or(UtcOffset::UTC);
    let Ok(timestamp) = OffsetDateTime::from_unix_timestamp_nanos(ms as i128 * 1_000_000) else {
        return String::new();
    };
    let local = timestamp.to_offset(offset);
    format!(
        "{:02}-{:02}-{:04} {:02}:{:02}",
        local.day(),
        u8::from(local.month()),
        local.year(),
        local.hour(),
        local.minute()
    )
}

/// Reports are date ranges; validate ordering and bound the span so a single
/// request cannot scan the whole ledger.
pub fn validate_range(from: &str, to: &str) -> Result<(Date, Date), crate::error::FieldError> {
    let from_date = parse_iso(from)
        .ok_or_else(|| crate::error::FieldError::new("from", "expected YYYY-MM-DD"))?;
    let to_date =
        parse_iso(to).ok_or_else(|| crate::error::FieldError::new("to", "expected YYYY-MM-DD"))?;
    if from_date > to_date {
        return Err(crate::error::FieldError::new(
            "from",
            "must not be after the end date",
        ));
    }
    if days_inclusive(from_date, to_date) > MAX_RANGE_DAYS {
        return Err(crate::error::FieldError::new(
            "to",
            format!("range may not exceed {MAX_RANGE_DAYS} days"),
        ));
    }
    Ok((from_date, to_date))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_iso_dates() {
        let d = parse_iso("2026-11-23").expect("valid");
        assert_eq!(iso(d), "2026-11-23");
        assert_eq!(display(d), "23-11-2026");
        assert!(parse_iso("2026-13-01").is_none());
        assert!(parse_iso("23-11-2026").is_none());
        assert!(parse_iso("2026-11-23 ").is_none());
    }

    #[test]
    fn month_range_handles_leap_years_and_december() {
        assert_eq!(
            month_range(2028, 2).map(|(a, b)| (iso(a), iso(b))),
            Some(("2028-02-01".to_string(), "2028-02-29".to_string()))
        );
        assert_eq!(
            month_range(2026, 12).map(|(a, b)| (iso(a), iso(b))),
            Some(("2026-12-01".to_string(), "2026-12-31".to_string()))
        );
        assert!(month_range(2026, 13).is_none());
    }

    #[test]
    fn validates_ranges() {
        assert!(validate_range("2026-11-01", "2026-11-30").is_ok());
        assert!(validate_range("2026-11-30", "2026-11-01").is_err());
        assert!(validate_range("2025-01-01", "2026-12-31").is_err());
    }
}
