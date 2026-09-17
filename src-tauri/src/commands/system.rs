// System-integration commands: notch metrics, file reveal, terminal,
// URL/agent-app opening, autostart, quit and deactivate.
use tauri::{AppHandle, Manager, State};

use crate::*;

#[tauri::command]
pub(crate) fn get_notch_metrics(state: State<'_, AppState>) -> NotchMetrics {
    *lock_state(&state.notch_metrics)
}

/// Reveal an exported file in the system file manager (Finder / Explorer).
#[tauri::command]
pub(crate) fn reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|error| format!("Failed to reveal path: {error}"))
}

#[tauri::command]
pub(crate) async fn open_in_terminal(cwd: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || platform::open_in_terminal(&cwd))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn focus_claude_app(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || platform::focus_claude_app(&app))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || platform::open_url(&app, &url))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn is_autostart_enabled() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(platform::autostart::is_enabled)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if enabled {
            platform::autostart::enable()
        } else {
            platform::autostart::disable()
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn quit_atoll(app: AppHandle) {
    exit_atoll(&app);
}

#[tauri::command]
pub(crate) async fn deactivate_atoll(
    app: AppHandle,
    agent: Option<String>,
    session: Option<String>,
    cwd: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        platform::restore_focus_after_approval(
            &app,
            &state,
            agent.as_deref(),
            session.as_deref(),
            cwd.as_deref(),
        );
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn open_agent_app(
    app: AppHandle,
    agent: String,
    cwd: String,
    session: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        platform::open_agent_app(&app, &state, &agent, &cwd, session.as_deref())
    })
    .await
    .map_err(|error| error.to_string())?
}
