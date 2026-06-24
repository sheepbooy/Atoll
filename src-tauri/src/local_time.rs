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
pub fn local_day_key_from_iso_in_offset(
    timestamp: &str,
    offset: FixedOffset,
) -> Option<String> {
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
