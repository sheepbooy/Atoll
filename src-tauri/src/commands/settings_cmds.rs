// User-facing settings commands: notice mode, notification language,
// global shortcuts, artwork backdrop, IME, retention windows.
use tauri::State;

use crate::*;

#[tauri::command]
pub(crate) fn set_ime_active(window: tauri::WebviewWindow, active: bool) {
    platform::set_ime_active(&window, active);
}

#[tauri::command]
pub(crate) fn get_approval_notice_mode(state: State<'_, AppState>) -> String {
    lock_state(&state.approval_notice_mode).clone()
}

#[tauri::command]
pub(crate) fn set_approval_notice_mode(state: State<'_, AppState>, mode: String) -> String {
    let mode = normalize_approval_notice_mode(&mode);
    *lock_state(&state.approval_notice_mode) = mode.to_string();
    persist_approval_notice_mode(mode);
    mode.to_string()
}

#[tauri::command]
pub(crate) fn set_notification_language(state: State<'_, AppState>, language: String) -> String {
    let language = normalize_notification_language(&language);
    *lock_state(&state.notification_language) = language.to_string();
    persist_notification_language(language);
    language.to_string()
}

#[tauri::command]
pub(crate) fn get_global_shortcut_config(
    state: State<'_, AppState>,
) -> shortcuts::GlobalShortcutView {
    let shortcuts = lock_state(&state.global_shortcuts);
    shortcuts::GlobalShortcutView {
        config: shortcuts.config.clone(),
        errors: shortcuts.errors.clone(),
    }
}

/// Persist + re-register the shortcut config. Always succeeds: accelerator
/// validation failures and registration failures (hotkey taken by another app)
/// are reported per action in `errors` so the Settings UI can render a clear
/// per-row error state instead of the change being silently swallowed.
#[tauri::command]
pub(crate) fn set_global_shortcut_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: shortcuts::GlobalShortcutConfig,
) -> shortcuts::GlobalShortcutView {
    let (config, errors) = shortcuts::canonicalize_config(config);
    // Validation failures skip re-registration so the last working bindings
    // stay live.
    let errors = if errors.has_errors() {
        errors
    } else {
        let registration = shortcuts::apply_config(&app, &config);
        shortcuts::persist_global_shortcut_config(&config);
        registration
    };
    {
        let mut shortcuts = lock_state(&state.global_shortcuts);
        shortcuts.config = config.clone();
        shortcuts.errors = errors.clone();
    }
    shortcuts::GlobalShortcutView { config, errors }
}

#[tauri::command]
pub(crate) fn get_artwork_backdrop_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.artwork_backdrop_enabled)
}

#[tauri::command]
pub(crate) fn set_artwork_backdrop_enabled(state: State<'_, AppState>, enabled: bool) -> bool {
    *lock_state(&state.artwork_backdrop_enabled) = enabled;
    persist_artwork_backdrop_enabled(enabled);
    enabled
}

#[tauri::command]
pub(crate) fn get_session_retention(state: State<'_, AppState>) -> u64 {
    *lock_state(&state.session_retention_secs)
}

#[tauri::command]
pub(crate) fn set_session_retention(state: State<'_, AppState>, minutes: u64) -> u64 {
    let clamped_minutes = minutes.clamp(1, 60);
    let secs = clamped_minutes * 60;
    let mut retention = lock_state(&state.session_retention_secs);
    *retention = secs;
    persist_retention_minutes(clamped_minutes);
    secs
}

#[tauri::command]
pub(crate) fn get_subagent_retention(state: State<'_, AppState>) -> u64 {
    *lock_state(&state.subagent_retention_secs)
}

#[tauri::command]
pub(crate) fn set_subagent_retention(state: State<'_, AppState>, minutes: u64) -> u64 {
    let clamped_minutes = minutes.clamp(1, 60);
    let secs = clamped_minutes * 60;
    let mut retention = lock_state(&state.subagent_retention_secs);
    *retention = secs;
    persist_settings(None, Some(clamped_minutes));
    secs
}
