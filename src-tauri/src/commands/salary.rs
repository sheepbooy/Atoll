// Salary-mode daily earnings history commands. The frontend reports the
// observed amount for a local date key (avoids Rust/frontend timezone skew).
use crate::salary_history;

#[tauri::command]
pub(crate) fn record_salary_day(date: String, amount: f64) -> Result<(), String> {
    salary_history::record_salary_day(&date, amount).map(|_| ())
}

#[tauri::command]
pub(crate) fn get_salary_history(
    days: u32,
) -> Result<salary_history::SalaryHistoryResponse, String> {
    salary_history::get_salary_history(days)
}
