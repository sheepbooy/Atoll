// Approval-history query/export/clear commands.
use crate::*;

#[tauri::command]
pub(crate) async fn get_approval_history(
    query: approval_history::ApprovalHistoryQuery,
) -> Result<approval_history::ApprovalHistoryPage, String> {
    tauri::async_runtime::spawn_blocking(move || approval_history::query_history(&query))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn export_approval_history(
    query: approval_history::ApprovalHistoryQuery,
    format: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || approval_history::export_history(&query, &format))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn clear_approval_history() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(approval_history::clear_history)
        .await
        .map_err(|error| error.to_string())?
}
