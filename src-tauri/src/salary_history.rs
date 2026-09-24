// Daily earnings history for the "salary" display mode. The frontend derives
// today's earnings from the wall clock and its salary settings; this module
// only persists the last observed per-day amount so the heatmap can look back.
use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::local_time::{current_local_day_key, format_local_day_key};

const HISTORY_VERSION: u32 = 1;
const RETENTION_DAYS: i64 = 365;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct SalaryHistoryFile {
    version: u32,
    timezone: String,
    days: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalaryHistoryDay {
    pub date: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalaryHistoryResponse {
    pub timezone: String,
    pub days: Vec<SalaryHistoryDay>,
}

pub fn salary_history_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_SALARY_HISTORY_PATH") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }

    dirs::home_dir().map(|home| home.join(".atoll").join("salary_history.json"))
}

fn system_timezone_name() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

fn fresh_history_file() -> SalaryHistoryFile {
    SalaryHistoryFile {
        version: HISTORY_VERSION,
        timezone: system_timezone_name(),
        days: HashMap::new(),
    }
}

fn load_history_file() -> SalaryHistoryFile {
    let Some(path) = salary_history_path() else {
        return fresh_history_file();
    };

    load_history_file_at(&path).unwrap_or_else(|| {
        let backup = path.with_extension("json.bak");
        load_history_file_at(&backup).unwrap_or_else(|| {
            eprintln!(
                "Atoll: failed to load salary history from {} (and backup); starting fresh",
                path.display()
            );
            fresh_history_file()
        })
    })
}

fn load_history_file_at(path: &std::path::Path) -> Option<SalaryHistoryFile> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_history_file(file: &SalaryHistoryFile) -> Result<(), String> {
    let Some(path) = salary_history_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let formatted = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    let temp_path = path.with_extension("json.tmp");
    std::fs::write(&temp_path, &formatted).map_err(|error| error.to_string())?;
    // Keep a backup so a crash mid-rename does not leave us with no recoverable file.
    let backup_path = path.with_extension("json.bak");
    if path.exists() {
        let _ = std::fs::copy(&path, &backup_path);
    }
    std::fs::rename(&temp_path, &path).map_err(|error| error.to_string())
}

fn prune_old_days(days: &mut HashMap<String, f64>) {
    let Ok(today) = NaiveDate::parse_from_str(&current_local_day_key(), "%Y-%m-%d") else {
        return;
    };
    let cutoff = today - Duration::days(RETENTION_DAYS);
    days.retain(|day_key, _| {
        NaiveDate::parse_from_str(day_key, "%Y-%m-%d")
            .map(|date| date >= cutoff)
            .unwrap_or(false)
    });
}

fn sanitize_amount(amount: f64) -> f64 {
    if !amount.is_finite() || amount < 0.0 {
        return 0.0;
    }
    // Keep the file stable: earnings only ever display two decimals.
    (amount * 100.0).round() / 100.0
}

/// Record the observed earnings for one day. Merged with max so a restart or a
/// mid-day wage drop never rewinds what was already earned and persisted.
pub fn record_salary_day(day_key: &str, amount: f64) -> Result<f64, String> {
    NaiveDate::parse_from_str(day_key, "%Y-%m-%d").map_err(|error| error.to_string())?;
    let amount = sanitize_amount(amount);
    let mut file = load_history_file();
    let merged = file.days.get(day_key).copied().unwrap_or(0.0).max(amount);
    file.version = HISTORY_VERSION;
    file.timezone = system_timezone_name();
    file.days.insert(day_key.to_string(), merged);
    prune_old_days(&mut file.days);
    save_history_file(&file)?;
    Ok(merged)
}

pub fn get_salary_history(days: u32) -> Result<SalaryHistoryResponse, String> {
    let file = load_history_file();
    let today_key = current_local_day_key();
    let today =
        NaiveDate::parse_from_str(&today_key, "%Y-%m-%d").map_err(|error| error.to_string())?;
    let span = days.max(1).min(365) as i64;

    let mut result = Vec::new();
    for offset in (0..span).rev() {
        let date = today - Duration::days(offset);
        let date_key = format_local_day_key(date);
        let amount = file.days.get(&date_key).copied().unwrap_or(0.0);
        result.push(SalaryHistoryDay {
            date: date_key,
            amount,
        });
    }

    Ok(SalaryHistoryResponse {
        timezone: file.timezone,
        days: result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    fn salary_path_test_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::SALARY_HISTORY_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn temp_salary_paths(test_name: &str) -> PathBuf {
        let pid = std::process::id();
        std::env::temp_dir().join(format!("atoll-salary-history-{pid}-{test_name}.json"))
    }

    fn setup_history_path(test_name: &str) -> PathBuf {
        let path = temp_salary_paths(test_name);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
        let _ = std::fs::remove_file(path.with_extension("json.tmp"));
        std::env::set_var("ATOLL_SALARY_HISTORY_PATH", path.to_string_lossy().as_ref());
        path
    }

    fn cleanup_history_path(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
        let _ = std::fs::remove_file(path.with_extension("json.tmp"));
        std::env::remove_var("ATOLL_SALARY_HISTORY_PATH");
    }

    #[test]
    fn record_then_query_returns_amount() {
        let _guard = salary_path_test_lock();
        let path = setup_history_path("record-query");
        let day_key = current_local_day_key();

        let merged = record_salary_day(&day_key, 123.456).expect("record");
        assert_eq!(merged, 123.46);

        let history = get_salary_history(3).expect("query");
        assert_eq!(history.days.len(), 3);
        let today = history.days.last().expect("today entry");
        assert_eq!(today.date, day_key);
        assert_eq!(today.amount, 123.46);

        cleanup_history_path(&path);
    }

    #[test]
    fn record_merges_with_max_on_regression() {
        let _guard = salary_path_test_lock();
        let path = setup_history_path("merge-max");
        let day_key = current_local_day_key();

        record_salary_day(&day_key, 200.0).expect("first record");
        let merged = record_salary_day(&day_key, 150.0).expect("second record");
        assert_eq!(
            merged, 200.0,
            "a lower observed value must not rewind the day"
        );

        record_salary_day(&day_key, 260.0).expect("third record");
        let history = get_salary_history(1).expect("query");
        assert_eq!(history.days[0].amount, 260.0);

        cleanup_history_path(&path);
    }

    #[test]
    fn record_rejects_invalid_date_and_sanitizes_amount() {
        let _guard = salary_path_test_lock();
        let path = setup_history_path("invalid-input");

        assert!(record_salary_day("not-a-date", 10.0).is_err());

        let day_key = current_local_day_key();
        let merged = record_salary_day(&day_key, f64::NAN).expect("nan record");
        assert_eq!(merged, 0.0);
        let merged = record_salary_day(&day_key, -5.0).expect("negative record");
        assert_eq!(merged, 0.0);

        cleanup_history_path(&path);
    }

    #[test]
    fn separate_days_do_not_interfere() {
        let _guard = salary_path_test_lock();
        let path = setup_history_path("separate-days");
        let today = Local::now().date_naive();
        let yesterday = format_local_day_key(today - Duration::days(1));

        record_salary_day(&current_local_day_key(), 50.0).expect("today record");
        record_salary_day(&yesterday, 80.0).expect("yesterday record");

        let history = get_salary_history(2).expect("query");
        assert_eq!(history.days[0].date, yesterday);
        assert_eq!(history.days[0].amount, 80.0);
        assert_eq!(history.days[1].amount, 50.0);

        cleanup_history_path(&path);
    }

    #[test]
    fn load_recovers_from_backup_when_main_is_corrupt() {
        let _guard = salary_path_test_lock();
        let path = setup_history_path("backup-recover");
        let day_key = current_local_day_key();

        record_salary_day(&day_key, 100.0).expect("initial record");
        record_salary_day(&day_key, 300.0).expect("updated record");

        // Simulate crash mid-write: main file is invalid JSON but backup remains.
        std::fs::write(&path, "{ not valid json").expect("corrupt main");

        let history = get_salary_history(1).expect("query after recovery");
        assert_eq!(
            history.days[0].amount, 100.0,
            "should load the pre-update snapshot from .bak"
        );

        cleanup_history_path(&path);
    }

    #[test]
    fn prune_drops_days_older_than_retention() {
        let _guard = salary_path_test_lock();
        let path = setup_history_path("prune-retention");
        let today = Local::now().date_naive();
        let old = format_local_day_key(today - Duration::days(RETENTION_DAYS + 1));
        let boundary = format_local_day_key(today - Duration::days(RETENTION_DAYS));

        record_salary_day(&old, 500.0).expect("old record");
        record_salary_day(&boundary, 400.0).expect("boundary record");
        record_salary_day(&current_local_day_key(), 10.0).expect("today record");

        let file = load_history_file();
        assert!(
            !file.days.contains_key(&old),
            "day beyond retention should be pruned"
        );
        assert!(
            file.days.contains_key(&boundary),
            "day at the retention cutoff is kept"
        );
        assert!(file.days.contains_key(&current_local_day_key()));

        cleanup_history_path(&path);
    }
}
