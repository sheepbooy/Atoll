use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone, Utc};

pub fn current_local_day_key() -> String {
    format_local_day_key(Local::now().date_naive())
}

pub fn format_local_day_key(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn local_day_key_from_iso(timestamp: &str) -> Option<String> {
    parse_iso_timestamp(timestamp)
        .map(|dt| format_local_day_key(dt.with_timezone(&Local).date_naive()))
}

pub fn is_local_today(timestamp: &str, local_today_key: &str) -> bool {
    local_day_key_from_iso(timestamp).as_deref() == Some(local_today_key)
}

#[allow(dead_code)]
pub fn local_day_key_from_iso_in_offset(timestamp: &str, offset: FixedOffset) -> Option<String> {
    parse_iso_timestamp(timestamp)
        .map(|dt| format_local_day_key(dt.with_timezone(&offset).date_naive()))
}

fn parse_iso_timestamp(timestamp: &str) -> Option<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(timestamp) {
        return Some(parsed.with_timezone(&Utc));
    }

    if let Ok(parsed) = timestamp.parse::<DateTime<Utc>>() {
        return Some(parsed);
    }

    const FORMATS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%.3fZ",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%dT%H:%M:%S",
    ];
    for format in FORMATS {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(timestamp, format) {
            return Some(Utc.from_utc_datetime(&naive));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn utc_late_evening_counts_as_next_local_day_in_utc_plus_8() {
        let offset = FixedOffset::east_opt(8 * 3600).expect("offset");
        // 2026-06-19 23:30 UTC -> 2026-06-20 07:30 in UTC+8
        let key =
            local_day_key_from_iso_in_offset("2026-06-19T23:30:00.000Z", offset).expect("key");
        assert_eq!(key, "2026-06-20");
    }

    #[test]
    fn is_local_today_matches_local_day_key() {
        let offset = FixedOffset::east_opt(8 * 3600).expect("offset");
        let today = "2026-06-20";
        let timestamp = "2026-06-19T23:30:00.000Z";
        let day_key = local_day_key_from_iso_in_offset(timestamp, offset).expect("key");
        assert_eq!(day_key, today);
    }
}

pub fn parse_iso_timestamp_secs(iso: &str) -> u64 {
    // Parse "YYYY-MM-DDTHH:MM:SSZ" to unix seconds (simplified)
    let parts: Vec<&str> = iso.split('T').collect();
    if parts.len() != 2 {
        return 0;
    }
    let date_parts: Vec<u64> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_str = parts[1].trim_end_matches('Z');
    let time_parts: Vec<u64> = time_str.split(':').filter_map(|s| s.parse().ok()).collect();

    if date_parts.len() != 3 || time_parts.len() < 3 {
        return 0;
    }

    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, min, sec) = (time_parts[0], time_parts[1], time_parts[2]);

    // Approximate days-from-epoch calculation
    let mut days: u64 = 0;
    for y in 1970..year {
        days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
    }
    let month_days = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    if month >= 1 && month <= 12 {
        days += month_days[(month - 1) as usize];
        if month > 2 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            days += 1;
        }
    }
    days += day.saturating_sub(1);

    days * 86400 + hour * 3600 + min * 60 + sec
}

pub fn iso_timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    format_unix_timestamp(duration.as_secs())
}

pub fn format_unix_timestamp(timestamp: u64) -> String {
    // Compact UTC formatter to avoid pulling in a full time crate for the MVP.
    const SECONDS_PER_DAY: u64 = 86_400;
    let days = timestamp / SECONDS_PER_DAY;
    let seconds_of_day = timestamp % SECONDS_PER_DAY;
    let (year, day_of_year) = civil_year_and_day(days);
    let (month, day) = month_and_day(year, day_of_year);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

pub fn civil_year_and_day(days_since_epoch: u64) -> (i32, u64) {
    let mut year = 1970;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            return (year, remaining_days);
        }

        remaining_days -= days_in_year;
        year += 1;
    }
}

pub fn month_and_day(year: i32, day_of_year: u64) -> (u64, u64) {
    let month_lengths = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut remaining = day_of_year;

    for (index, days) in month_lengths.iter().enumerate() {
        if remaining < *days {
            return (index as u64 + 1, remaining + 1);
        }
        remaining -= days;
    }

    (12, 31)
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
