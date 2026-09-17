// Snapshot and per-session request-list commands.
use tauri::{AppHandle, Manager, State};

use crate::*;

#[tauri::command]
pub(crate) fn get_session_requests(
    state: State<'_, AppState>,
    session_id: String,
) -> Vec<PermissionRequest> {
    let requests = lock_state(&state.requests);
    requests
        .iter()
        .filter(|r| !r.archived && r.session == session_id)
        .cloned()
        .collect()
}

#[tauri::command]
pub(crate) async fn get_snapshot(app: AppHandle) -> Result<IslandSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        roll_over_token_usage_if_needed(&state);
        let snapshot = build_snapshot(&app, &state);
        if let Ok(mut last) = state.last_listening_online.lock() {
            *last = Some(snapshot.online);
        }
        remember_hook_health(&state, &snapshot.hook_health);
        snapshot
    })
    .await
    .map_err(|error| error.to_string())
}
