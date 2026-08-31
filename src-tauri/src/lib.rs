use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::utils::config::Color;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalSize, State};

mod approval_history;
mod capture;
mod clipboard_history;
mod debug_agent;
mod hook_bridge;
mod hook_trust;
mod local_time;
mod lyrics;
#[cfg(target_os = "macos")]
mod media;
// Compiled on every platform so the pure-logic unit tests run on any host;
// the WinRT calls are cfg(windows) inside, hence the dead-code allowance on
// non-Windows targets where only the tests reference them.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod media_windows;
mod platform;
mod pricing;
mod shortcuts;
mod token_history;
mod transcript;

mod state;

pub(crate) use state::*;

mod settings;

pub(crate) use settings::*;

mod transcript_cache;

pub(crate) use transcript_cache::*;

mod tray;

pub(crate) use tray::*;

mod token_usage;

pub(crate) use token_usage::*;

mod monitors;

pub(crate) use monitors::*;

mod window;

pub(crate) use window::*;

mod session;

pub(crate) use session::*;

mod hooks;

pub(crate) use hooks::*;

fn build_hook_status(
    installed: bool,
    script_found: bool,
    settings_path: String,
    script_path: String,
    config: Option<&Value>,
    marker: &str,
    agent_key: &str,
) -> HookStatus {
    let node_path = config
        .and_then(|cfg| configured_atoll_hook_node_path(cfg, marker))
        .unwrap_or_default();
    let node_found = node_executable_ready(&node_path);
    // Only meaningful once installed — an agent that was never hooked up has
    // nothing to have drifted away from.
    let configured_script = config.and_then(|cfg| configured_atoll_hook_script_path(cfg, marker));
    let needs_retrust = installed
        && hook_trust::needs_retrust(agent_key, &script_path, configured_script.as_deref());
    HookStatus {
        installed,
        script_found,
        settings_path,
        script_path,
        node_path,
        node_found,
        needs_retrust,
        competing_hooks: Vec::new(),
    }
}

#[tauri::command]
fn resolve_permission_request(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    decision: Decision,
    note: String,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    let status = match decision {
        Decision::Approved => PermissionStatus::Approved,
        Decision::Denied => PermissionStatus::Denied,
    };

    let Some(request) = requests.iter_mut().find(|request| request.id == id) else {
        return Err(format!("Permission request not found: {id}"));
    };

    request.status = status;
    if !note.trim().is_empty() {
        request.detail = format!("{} Note: {}", request.detail, note.trim());
    }

    let session_id = request.session.clone();
    let resolved_request = request.clone();

    let waiter = state
        .hook_waiters
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&id);
    if let Some(waiter) = waiter {
        let _ = waiter.send(DecisionWithNote {
            decision,
            note: note.clone(),
            updated_input: None,
        });
    }

    touch_session_activity(&state, &session_id);
    drop(requests);
    approval_history::record_outcome(
        &state,
        &resolved_request,
        match decision {
            Decision::Approved => approval_history::HistoryStatus::Approved,
            Decision::Denied => approval_history::HistoryStatus::Denied,
        },
    );
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn resolve_permission_with_input(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    decision: Decision,
    note: String,
    updated_input: Option<Value>,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    let status = match decision {
        Decision::Approved => PermissionStatus::Approved,
        Decision::Denied => PermissionStatus::Denied,
    };

    let Some(request) = requests.iter_mut().find(|request| request.id == id) else {
        return Err(format!("Permission request not found: {id}"));
    };

    request.status = status;
    if !note.trim().is_empty() {
        request.detail = format!("{} Note: {}", request.detail, note.trim());
    }

    let session_id = request.session.clone();
    let resolved_request = request.clone();

    let waiter = state
        .hook_waiters
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&id);
    if let Some(waiter) = waiter {
        let _ = waiter.send(DecisionWithNote {
            decision,
            note: note.clone(),
            updated_input,
        });
    }

    touch_session_activity(&state, &session_id);
    drop(requests);
    approval_history::record_outcome(
        &state,
        &resolved_request,
        match decision {
            Decision::Approved => approval_history::HistoryStatus::Approved,
            Decision::Denied => approval_history::HistoryStatus::Denied,
        },
    );
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn get_notch_metrics(state: State<'_, AppState>) -> NotchMetrics {
    *lock_state(&state.notch_metrics)
}

#[tauri::command]
fn set_ime_active(window: tauri::WebviewWindow, active: bool) {
    platform::set_ime_active(&window, active);
}

#[tauri::command]
fn set_session_auto_approve(
    state: State<'_, AppState>,
    session: String,
    enabled: bool,
) -> Result<(), String> {
    let mut sessions = state
        .auto_approve_sessions
        .lock()
        .map_err(|error| error.to_string())?;
    if enabled {
        sessions.insert(session);
    } else {
        sessions.remove(&session);
    }
    Ok(())
}

#[tauri::command]
fn archive_request(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    if let Some(request) = requests.iter_mut().find(|r| r.id == id) {
        request.archived = true;
    }
    drop(requests);
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn get_session_requests(state: State<'_, AppState>, session_id: String) -> Vec<PermissionRequest> {
    let requests = lock_state(&state.requests);
    requests
        .iter()
        .filter(|r| !r.archived && r.session == session_id)
        .cloned()
        .collect()
}

#[tauri::command]
fn get_token_history(days: u32) -> Result<token_history::TokenHistoryResponse, String> {
    token_history::get_token_history(days)
}

#[tauri::command]
fn get_pricing() -> Result<pricing::PricingResponse, String> {
    pricing::get_pricing()
}

#[tauri::command]
fn set_model_rate(
    request: pricing::SetModelRateRequest,
) -> Result<pricing::PricingResponse, String> {
    pricing::set_model_rate(request)
}

#[tauri::command]
fn reset_model_rate(model_id: String) -> Result<pricing::PricingResponse, String> {
    pricing::reset_model_rate(model_id)
}

#[tauri::command]
fn hide_model(model_id: String) -> Result<pricing::PricingResponse, String> {
    pricing::hide_model(model_id)
}

#[tauri::command]
fn unhide_model(model_id: String) -> Result<pricing::PricingResponse, String> {
    pricing::unhide_model(model_id)
}

#[tauri::command]
async fn refresh_pricing() -> Result<pricing::PricingResponse, String> {
    tauri::async_runtime::spawn_blocking(|| pricing::refresh_pricing_catalog(true))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn archive_all_resolved(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    let pinned = state
        .pinned_sessions
        .lock()
        .map_err(|error| error.to_string())?;
    // Archive-all: keep pending requests and requests belonging to pinned sessions.
    requests.retain(|request| {
        request.status == PermissionStatus::Pending || pinned.contains(&request.session)
    });
    // Also remove non-pinned known sessions.
    {
        let mut known = state
            .known_sessions
            .lock()
            .map_err(|error| error.to_string())?;
        known.retain(|session_id, _| pinned.contains(session_id));
    }
    drop(requests);
    drop(pinned);
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn archive_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<IslandSnapshot, String> {
    let removed_pending_ids: Vec<String> = {
        let requests = state.requests.lock().map_err(|error| error.to_string())?;
        requests
            .iter()
            .filter(|request| {
                request.session == session_id && request.status == PermissionStatus::Pending
            })
            .map(|request| request.id.clone())
            .collect()
    };

    for request_id in removed_pending_ids {
        if let Ok(mut waiters) = state.hook_waiters.lock() {
            if let Some(waiter) = waiters.remove(&request_id) {
                let _ = waiter.send(DecisionWithNote {
                    decision: Decision::Denied,
                    note: "Session archived in Atoll.".into(),
                    updated_input: None,
                });
            }
        }
    }

    {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        // Remove session data outright so retention replay does not keep it visible.
        requests.retain(|request| request.session != session_id);
    }
    {
        let mut known = state
            .known_sessions
            .lock()
            .map_err(|error| error.to_string())?;
        known.remove(&session_id);
    }
    {
        let mut pinned = state
            .pinned_sessions
            .lock()
            .map_err(|error| error.to_string())?;
        pinned.remove(&session_id);
    }
    if let Ok(mut last_seen) = state.session_last_seen.lock() {
        last_seen.remove(&session_id);
    }
    if let Ok(mut totals) = state.session_request_totals.lock() {
        totals.remove(&session_id);
    }
    // Keep session_token_usage so archived sessions still count toward daily totals.
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn pin_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    pinned: bool,
) -> Result<IslandSnapshot, String> {
    {
        let mut pinned_set = state
            .pinned_sessions
            .lock()
            .map_err(|error| error.to_string())?;
        if pinned {
            pinned_set.insert(session_id);
        } else {
            pinned_set.remove(&session_id);
        }
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

/// Fetch the current Now Playing track from the platform media source —
/// the macOS MediaRemote adapter or the Windows SMTC session manager.
#[cfg(target_os = "macos")]
fn platform_now_playing() -> Option<NowPlayingTrack> {
    media::fetch_now_playing()
}

#[cfg(target_os = "windows")]
fn platform_now_playing() -> Option<NowPlayingTrack> {
    media_windows::fetch_now_playing()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_now_playing() -> Option<NowPlayingTrack> {
    None
}

#[tauri::command]
fn get_now_playing() -> Option<NowPlayingTrack> {
    platform_now_playing()
}

#[tauri::command]
fn send_media_command(command: String) -> bool {
    #[cfg(target_os = "macos")]
    {
        let cmd = match command.as_str() {
            "play" => media::MR_COMMAND_PLAY,
            "pause" => media::MR_COMMAND_PAUSE,
            "toggle" => media::MR_COMMAND_TOGGLE,
            "next" => media::MR_COMMAND_NEXT,
            "prev" => media::MR_COMMAND_PREV,
            _ => return false,
        };
        media::send_media_command_raw(cmd)
    }
    #[cfg(target_os = "windows")]
    {
        media_windows::send_media_command(&command)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = command;
        false
    }
}

#[tauri::command]
fn get_media_card_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.media_card_enabled)
}

#[tauri::command]
fn set_media_card_enabled(state: State<'_, AppState>, enabled: bool) -> bool {
    *lock_state(&state.media_card_enabled) = enabled;
    persist_media_card_enabled(enabled);
    enabled
}

#[tauri::command]
fn get_approval_notice_mode(state: State<'_, AppState>) -> String {
    lock_state(&state.approval_notice_mode).clone()
}

#[tauri::command]
fn set_approval_notice_mode(state: State<'_, AppState>, mode: String) -> String {
    let mode = normalize_approval_notice_mode(&mode);
    *lock_state(&state.approval_notice_mode) = mode.to_string();
    persist_approval_notice_mode(mode);
    mode.to_string()
}

#[tauri::command]
fn set_notification_language(state: State<'_, AppState>, language: String) -> String {
    let language = normalize_notification_language(&language);
    *lock_state(&state.notification_language) = language.to_string();
    persist_notification_language(language);
    language.to_string()
}

#[tauri::command]
fn get_global_shortcut_config(state: State<'_, AppState>) -> shortcuts::GlobalShortcutView {
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
fn set_global_shortcut_config(
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
fn get_artwork_backdrop_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.artwork_backdrop_enabled)
}

#[tauri::command]
fn set_artwork_backdrop_enabled(state: State<'_, AppState>, enabled: bool) -> bool {
    *lock_state(&state.artwork_backdrop_enabled) = enabled;
    persist_artwork_backdrop_enabled(enabled);
    enabled
}

#[tauri::command]
fn get_lyrics_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.lyrics_enabled)
}

#[tauri::command]
fn set_lyrics_enabled(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> bool {
    *lock_state(&state.lyrics_enabled) = enabled;
    persist_lyrics_enabled(enabled);
    if !enabled {
        *lock_state(&state.lyrics) = None;
        *lock_state(&state.lyrics_track_key) = String::new();
        let _ = app.emit("lyrics-changed", Option::<lyrics::LyricPayload>::None);
    }
    enabled
}

#[tauri::command]
fn get_current_lyrics(state: State<'_, AppState>) -> Option<lyrics::LyricPayload> {
    lock_state(&state.lyrics).clone()
}

#[tauri::command]
fn get_session_retention(state: State<'_, AppState>) -> u64 {
    *lock_state(&state.session_retention_secs)
}

#[tauri::command]
fn set_session_retention(state: State<'_, AppState>, minutes: u64) -> u64 {
    let clamped_minutes = minutes.clamp(1, 60);
    let secs = clamped_minutes * 60;
    let mut retention = lock_state(&state.session_retention_secs);
    *retention = secs;
    persist_retention_minutes(clamped_minutes);
    secs
}

#[tauri::command]
fn get_subagent_retention(state: State<'_, AppState>) -> u64 {
    *lock_state(&state.subagent_retention_secs)
}

#[tauri::command]
fn set_subagent_retention(state: State<'_, AppState>, minutes: u64) -> u64 {
    let clamped_minutes = minutes.clamp(1, 60);
    let secs = clamped_minutes * 60;
    let mut retention = lock_state(&state.subagent_retention_secs);
    *retention = secs;
    persist_settings(None, Some(clamped_minutes));
    secs
}

#[tauri::command]
fn get_clipboard_history(state: State<'_, AppState>) -> Vec<clipboard_history::ClipboardEntry> {
    let limit = *lock_state(&state.clipboard_history_limit);
    let mut entries = lock_state(&state.clipboard_history);
    let before = entries.len();
    clipboard_history::prune_expired(&mut entries, limit);
    let result = entries.clone();
    if result.len() != before {
        clipboard_history::save_history(&entries);
    }
    result
}

#[tauri::command]
async fn get_approval_history(
    query: approval_history::ApprovalHistoryQuery,
) -> Result<approval_history::ApprovalHistoryPage, String> {
    tauri::async_runtime::spawn_blocking(move || approval_history::query_history(&query))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn export_approval_history(
    query: approval_history::ApprovalHistoryQuery,
    format: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || approval_history::export_history(&query, &format))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn clear_approval_history() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(approval_history::clear_history)
        .await
        .map_err(|error| error.to_string())?
}

/// Reveal an exported file in the system file manager (Finder / Explorer).
#[tauri::command]
fn reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|error| format!("Failed to reveal path: {error}"))
}

#[tauri::command]
fn copy_clipboard_entry(app: AppHandle, state: State<'_, AppState>, id: String) -> bool {
    let entry = lock_state(&state.clipboard_history)
        .iter()
        .find(|e| e.id == id)
        .cloned();
    let Some(entry) = entry else {
        return false;
    };
    let payload = match entry.kind {
        clipboard_history::EntryKind::Text => Some(clipboard_history::ClipboardPayload::Text(
            entry.content.clone(),
        )),
        clipboard_history::EntryKind::Image => clipboard_history::read_image_blob(&entry.id)
            .map(|png| clipboard_history::ClipboardPayload::Image { png }),
        clipboard_history::EntryKind::Files => Some(clipboard_history::ClipboardPayload::Files(
            entry.content.lines().map(str::to_string).collect(),
        )),
    };
    let Some(payload) = payload else {
        return false;
    };
    write_clipboard_payload(&app, &payload)
}

#[tauri::command]
fn clear_clipboard_history(app: AppHandle, state: State<'_, AppState>) {
    let mut entries = lock_state(&state.clipboard_history);
    clipboard_history::clear_unfavorited(&mut entries);
    clipboard_history::save_history(&entries);
    let snapshot = entries.clone();
    drop(entries);
    let _ = app.emit("clipboard-history-changed", &snapshot);
}

#[tauri::command]
fn toggle_clipboard_favorite(app: AppHandle, state: State<'_, AppState>, id: String) -> bool {
    let limit = *lock_state(&state.clipboard_history_limit);
    let mut entries = lock_state(&state.clipboard_history);
    let changed = clipboard_history::toggle_favorite(&mut entries, &id, limit);
    if changed {
        clipboard_history::save_history(&entries);
        let snapshot = entries.clone();
        drop(entries);
        let _ = app.emit("clipboard-history-changed", &snapshot);
    }
    changed
}

#[tauri::command]
fn get_clipboard_history_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.clipboard_history_enabled)
}

#[tauri::command]
fn set_clipboard_history_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> bool {
    *lock_state(&state.clipboard_history_enabled) = enabled;
    persist_clipboard_history_enabled(enabled);
    if enabled {
        // Snapshot the current clipboard first (main-thread read), then
        // reload persisted history under the state lock so enabling feels
        // like it captured what was just copied. Never touch the main
        // thread while holding the history lock: sync Tauri commands run
        // on the main thread and take the same lock.
        let snapshot = read_clipboard_snapshot(&app);
        let limit = *lock_state(&state.clipboard_history_limit);
        let mut entries = lock_state(&state.clipboard_history);
        *entries = clipboard_history::load_history(limit);
        if let Some(payload) = snapshot {
            if clipboard_history::add_entry(&mut entries, payload, limit) {
                clipboard_history::save_history(&entries);
            }
        }
    }
    enabled
}

#[tauri::command]
fn get_clipboard_history_limit(state: State<'_, AppState>) -> usize {
    *lock_state(&state.clipboard_history_limit)
}

#[tauri::command]
fn set_clipboard_history_limit(state: State<'_, AppState>, limit: usize) -> usize {
    let clamped = limit.clamp(
        clipboard_history::MIN_HISTORY_LIMIT,
        clipboard_history::MAX_HISTORY_LIMIT,
    );
    *lock_state(&state.clipboard_history_limit) = clamped;
    persist_clipboard_history_limit(clamped);
    // Shrinking the limit prunes immediately (and drops trimmed blobs).
    let mut entries = lock_state(&state.clipboard_history);
    let before = entries.len();
    clipboard_history::prune_expired(&mut entries, clamped);
    if entries.len() != before {
        clipboard_history::save_history(&entries);
    }
    clamped
}

#[tauri::command]
fn get_clipboard_entry_thumbnail(state: State<'_, AppState>, id: String) -> Option<String> {
    let is_image = lock_state(&state.clipboard_history)
        .iter()
        .any(|e| e.id == id && e.kind == clipboard_history::EntryKind::Image);
    if !is_image {
        return None;
    }
    clipboard_history::read_thumbnail_data_url(&id)
}

/// ZCode writes one model-I/O rollout JSONL per session, named after the full
/// session id (`sess_...`). Session ids come from hook payloads and subagent
/// metadata files, so restrict them to the shape ZCode actually emits before
/// splicing one into a path.
fn is_safe_zcode_session_id(session_id: &str) -> bool {
    session_id.starts_with("sess_")
        && session_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn zcode_rollout_path(session_id: &str) -> Option<std::path::PathBuf> {
    if !is_safe_zcode_session_id(session_id) {
        return None;
    }
    dirs::home_dir().map(|home| {
        home.join(".zcode")
            .join("cli")
            .join("rollout")
            .join(format!("model-io-{session_id}.jsonl"))
    })
}

/// Directory ZCode uses to persist per-subagent metadata
/// (`~/.zcode/cli/agents/<parent_session_id>/agent_*/metadata.json`).
fn zcode_session_agents_dir(parent_session_id: &str) -> Option<std::path::PathBuf> {
    if !is_safe_zcode_session_id(parent_session_id) {
        return None;
    }
    dirs::home_dir().map(|home| {
        home.join(".zcode")
            .join("cli")
            .join("agents")
            .join(parent_session_id)
    })
}

#[tauri::command]
async fn open_in_terminal(cwd: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || platform::open_in_terminal(&cwd))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn focus_claude_app(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || platform::focus_claude_app(&app))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || platform::open_url(&app, &url))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn is_autostart_enabled() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(platform::autostart::is_enabled)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
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
fn quit_atoll(app: AppHandle) {
    exit_atoll(&app);
}

#[tauri::command]
async fn deactivate_atoll(
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
async fn open_agent_app(
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

#[tauri::command]
async fn get_snapshot(app: AppHandle) -> Result<IslandSnapshot, String> {
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            requests: Mutex::new(Vec::new()),
            session_request_totals: Mutex::new(HashMap::new()),
            hook_waiters: Mutex::new(HashMap::new()),
            auto_approve_sessions: Mutex::new(HashSet::new()),
            compact_width: Mutex::new(COMPACT_WINDOW_WIDTH),
            compact_left_width: Mutex::new(0.0),
            presentation_generation: Arc::new(AtomicU64::new(0)),
            home_bounds: Mutex::new(None),
            notch_metrics: Mutex::new(NotchMetrics::default()),
            session_last_seen: Mutex::new(HashMap::new()),
            session_retention_secs: Mutex::new(DEFAULT_SESSION_RETENTION_SECS),
            subagent_retention_secs: Mutex::new(DEFAULT_SUBAGENT_RETENTION_SECS),
            session_token_usage: Mutex::new(HashMap::new()),
            session_token_usage_by_model: Mutex::new(HashMap::new()),
            session_agent_map: Mutex::new(HashMap::new()),
            token_usage_file_offsets: Mutex::new(HashMap::new()),
            token_usage_day: Mutex::new(current_local_day_key()),
            startup_daily_floor: Mutex::new(token_history::load_today_baseline()),
            startup_daily_floor_by_model: Mutex::new(token_history::load_today_by_model_baseline()),
            absolute_token_sessions: Mutex::new(HashSet::new()),
            daily_tokens_baseline: Mutex::new(token_history::load_today_baseline()),
            known_sessions: Mutex::new(HashMap::new()),
            pinned_sessions: Mutex::new(HashSet::new()),
            previous_app_pid: Mutex::new(None),
            last_listening_online: Mutex::new(None),
            last_hook_health: Mutex::new(None),
            bridge_port: AtomicU16::new(0),
            bridge_auth_token: Mutex::new(uuid::Uuid::new_v4().to_string()),
            last_bridge_reachable: Mutex::new(None),
            active_subagents: Mutex::new(Vec::new()),
            cursor_subagent_conversations: Mutex::new(HashMap::new()),
            cursor_lifecycle_token_sessions: Mutex::new(HashSet::new()),
            last_subagent_snapshot_emit: Mutex::new(Instant::now() - Duration::from_secs(10)),
            snapshot_debounce_generation: AtomicU64::new(0),
            snapshot_debounce_worker_running: AtomicBool::new(false),
            last_subagent_reconcile: Mutex::new(Instant::now() - Duration::from_secs(10)),
            last_hook_activity: Mutex::new(Instant::now()),
            token_history_dirty: AtomicBool::new(false),
            transcript_cache: Mutex::new(TranscriptCache::default()),
            media_card_enabled: Mutex::new(load_media_card_enabled()),
            artwork_backdrop_enabled: Mutex::new(load_artwork_backdrop_enabled()),
            clipboard_history_limit: Mutex::new(load_clipboard_history_limit()),
            clipboard_history: Mutex::new(clipboard_history::load_history(
                load_clipboard_history_limit(),
            )),
            clipboard_history_enabled: Mutex::new(load_clipboard_history_enabled()),
            lyrics_enabled: Mutex::new(load_lyrics_enabled()),
            lyrics: Mutex::new(None),
            lyrics_track_key: Mutex::new(String::new()),
            approval_notice_mode: Mutex::new(load_approval_notice_mode()),
            notification_language: Mutex::new(load_notification_language()),
            global_shortcuts: Mutex::new(shortcuts::GlobalShortcutsState::default()),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_session_requests,
            get_session_transcript,
            get_session_chat,
            resolve_permission_request,
            resolve_permission_with_input,
            set_session_auto_approve,
            archive_request,
            archive_all_resolved,
            archive_session,
            pin_session,
            set_island_presentation,
            get_notch_metrics,
            set_ime_active,
            uses_micro_island,
            get_claude_hook_status,
            install_claude_hooks,
            uninstall_claude_hooks,
            remove_competing_claude_hooks,
            get_codex_hook_status,
            install_codex_hooks,
            uninstall_codex_hooks,
            get_zcode_hook_status,
            install_zcode_hooks,
            uninstall_zcode_hooks,
            get_gemini_hook_status,
            install_gemini_hooks,
            uninstall_gemini_hooks,
            get_cursor_hook_status,
            install_cursor_hooks,
            uninstall_cursor_hooks,
            get_session_retention,
            set_session_retention,
            get_subagent_retention,
            set_subagent_retention,
            get_now_playing,
            send_media_command,
            get_media_card_enabled,
            set_media_card_enabled,
            get_approval_notice_mode,
            set_approval_notice_mode,
            set_notification_language,
            get_global_shortcut_config,
            set_global_shortcut_config,
            get_artwork_backdrop_enabled,
            set_artwork_backdrop_enabled,
            get_lyrics_enabled,
            set_lyrics_enabled,
            get_current_lyrics,
            get_clipboard_history,
            copy_clipboard_entry,
            clear_clipboard_history,
            get_approval_history,
            export_approval_history,
            clear_approval_history,
            reveal_path,
            get_clipboard_history_enabled,
            set_clipboard_history_enabled,
            get_clipboard_history_limit,
            set_clipboard_history_limit,
            toggle_clipboard_favorite,
            get_clipboard_entry_thumbnail,
            archive_subagent,
            archive_completed_subagents,
            get_token_history,
            get_pricing,
            set_model_rate,
            reset_model_rate,
            hide_model,
            unhide_model,
            refresh_pricing,
            open_in_terminal,
            open_agent_app,
            focus_claude_app,
            open_url,
            is_autostart_enabled,
            set_autostart_enabled,
            quit_atoll,
            deactivate_atoll,
            capture::capture_provide_screenshot
        ])
        .setup(|app| {
            #[cfg(desktop)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
                app.handle().plugin(tauri_plugin_notification::init())?;
                app.handle()
                    .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;
            }

            if !platform::setup_app(app) {
                std::process::exit(0);
            }

            build_tray(app.handle())?;
            hook_bridge::start_server(app.handle().clone());
            #[cfg(desktop)]
            shortcuts::startup(app.handle());
            start_island_hover_monitor(app.handle().clone());
            platform::start_activation_observer(app.handle().clone());
            if let Some(window) = app.get_webview_window("main") {
                let reveal_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(true) = event {
                        handle_island_reveal_request(&reveal_handle);
                    }
                });
            }
            {
                let state = app.state::<AppState>();
                let retention = load_persisted_retention_secs();
                *lock_state(&state.session_retention_secs) = retention;
                let sub_retention = load_persisted_subagent_retention_secs();
                *lock_state(&state.subagent_retention_secs) = sub_retention;
            }
            start_auto_archive_timer(app.handle().clone());
            start_token_refresh_timer(app.handle().clone());
            start_token_history_writer(app.handle().clone());
            start_media_monitor(app.handle().clone());
            start_clipboard_monitor(app.handle().clone());
            start_lyrics_monitor(app.handle().clone());
            start_initial_maintenance(app.handle().clone());
            std::thread::spawn(|| {
                pricing::maybe_refresh_pricing_catalog_on_startup();
            });

            if capture::enabled() {
                let state = app.state::<AppState>();
                capture::seed_approval_demo(app.handle(), &state);
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_shadow(false);
                let _ = window.set_skip_taskbar(true);
                let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = window.show();
                    // Apply island style AFTER show() so the window number is
                    // assigned and the NSPanel promotion takes effect on macOS.
                }
                platform::apply_island_window_style(&window);
                eprintln!("[Atoll] step: island style applied, now applying mode...");
                let initial_mode = if cfg!(target_os = "windows") {
                    IslandWindowMode::Micro
                } else {
                    IslandWindowMode::Compact
                };
                if let Ok(Some(home)) = apply_island_window_mode(
                    &window,
                    initial_mode,
                    COMPACT_WINDOW_WIDTH,
                    0.0,
                    false,
                    false,
                    false,
                ) {
                    eprintln!("[Atoll] step: island window mode applied");
                    let state = app.state::<AppState>();
                    if let Ok(mut home_bounds) = state.home_bounds.lock() {
                        *home_bounds = Some(home);
                    };
                    if let Ok(mut notch_metrics) = state.notch_metrics.lock() {
                        *notch_metrics = home.notch;
                    };
                }
                #[cfg(target_os = "windows")]
                platform::show_island_on_top(&window);
                eprintln!("[Atoll] step: setup window complete");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Atoll");
}

fn parse_iso_timestamp_secs(iso: &str) -> u64 {
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

/// Why the island window was opened. A global-hotkey summon toggles in the
/// frontend (press again to collapse, no idle auto-collapse); every other
/// opener keeps the expand-then-idle-collapse behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IslandOpenSource {
    Summon,
    Focus,
}

/// Compose the system-notification copy for a new approval request.
fn approval_notification_copy(
    agent_label: &str,
    command: &str,
    cwd: &str,
    language: &str,
) -> (String, String) {
    let first_line = command.lines().next().unwrap_or("").trim();
    let mut summary: String = first_line.chars().take(140).collect();
    if first_line.chars().count() > 140 {
        summary.push('…');
    }
    let project = cwd
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(cwd);
    if language == "zh-CN" {
        (
            format!("{agent_label} 请求批准"),
            format!("{summary}\n{project} · 点击通知打开 Atoll"),
        )
    } else {
        (
            format!("{agent_label} requests approval"),
            format!("{summary}\n{project} · Click to open Atoll"),
        )
    }
}

fn cursor_client_point(window: &tauri::WebviewWindow) -> Option<(f64, f64)> {
    let scale = window.scale_factor().ok()?;
    let cursor = window.cursor_position().ok()?.to_logical::<f64>(scale);
    let origin = window.outer_position().ok()?.to_logical::<f64>(scale);
    Some((cursor.x - origin.x, cursor.y - origin.y))
}

fn is_cursor_over_window(window: &tauri::WebviewWindow) -> tauri::Result<bool> {
    if !window.is_visible()? {
        return Ok(false);
    }

    let cursor = window.cursor_position()?;
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let padding = 8.0;

    let left = position.x as f64 - padding;
    let top = position.y as f64 - padding;
    let right = position.x as f64 + size.width as f64 + padding;
    let bottom = position.y as f64 + size.height as f64 + padding;

    Ok(cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom)
}

fn default_compact_left_pane_width(compact_width: f64, notch: NotchMetrics) -> f64 {
    if notch.has_notch {
        ((compact_width - notch.width).max(0.0) / 2.0).max(28.0)
    } else {
        (compact_width / 2.0).max(28.0)
    }
}

fn compact_window_origin_x(
    monitor_center_x: f64,
    window_width: f64,
    notch: NotchMetrics,
    left_pane_width: f64,
    mode: IslandWindowMode,
) -> f64 {
    if notch.has_notch && matches!(mode, IslandWindowMode::Compact) {
        monitor_center_x - notch.width / 2.0 - left_pane_width.max(0.0)
    } else {
        monitor_center_x - window_width / 2.0
    }
}

fn is_collapsed_pass_through_mode(mode: IslandWindowMode) -> bool {
    matches!(
        mode,
        IslandWindowMode::Micro | IslandWindowMode::Compact | IslandWindowMode::Dormant
    )
}

fn ease_out_cubic(progress: f64) -> f64 {
    1.0 - (1.0 - progress).powi(3)
}

/// Under-damped spring step response, normalized so it settles exactly at 1.0.
/// Launches with an initial velocity (fast start like the old ease-out-back),
/// decelerates, overshoots ~2%, peaks around 70–80% of the duration, then
/// settles without dipping — the Dynamic-Island expand feel.
/// ζ = 0.72, ω = 5.5, v₀ = 2.2.
fn ease_out_spring(progress: f64) -> f64 {
    let zeta: f64 = 0.72;
    let omega: f64 = 5.5;
    let v0: f64 = 2.2;
    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
    let c = (zeta * omega - v0) / omega_d;
    let value = |t: f64| {
        let decay = (-zeta * omega * t).exp();
        1.0 - decay * ((omega_d * t).cos() + c * (omega_d * t).sin())
    };
    let end = value(1.0);
    if end.abs() < 1e-9 {
        return progress.clamp(0.0, 1.0);
    }
    value(progress.clamp(0.0, 1.0)) / end
}

/// A display has a camera housing ("notch") when the two menu-bar halves
/// (auxiliary top areas) don't span the full screen width — the gap between
/// them is the notch.
#[cfg(test)]
fn has_camera_housing(frame_width: f64, aux_left_width: f64, aux_right_width: f64) -> bool {
    aux_left_width > 0.0
        && aux_right_width > 0.0
        && aux_left_width + aux_right_width < frame_width - 1.0
}

/// Notch width in logical points, derived from the gap between the auxiliary
/// menu-bar areas (matches ping-island's detection). Falls back when the
/// auxiliary areas are unavailable.
#[cfg(test)]
fn notch_logical_width(
    frame_width: f64,
    aux_left_width: f64,
    aux_right_width: f64,
    fallback: f64,
) -> f64 {
    if aux_left_width > 0.0 && aux_right_width > 0.0 {
        let detected = (frame_width - aux_left_width - aux_right_width + 4.0).ceil();
        detected.max(fallback)
    } else {
        fallback
    }
}

fn snapshot_from(
    requests: &[PermissionRequest],
    session_last_seen: &HashMap<String, u64>,
    retention_secs: u64,
    session_token_usage: &HashMap<String, TokenUsage>,
    known_sessions: &HashMap<String, KnownSession>,
    pinned_sessions: &HashSet<String>,
    online: bool,
    excluded_session_ids: &HashSet<String>,
) -> IslandSnapshot {
    let visible: Vec<&PermissionRequest> = requests
        .iter()
        .filter(|request| !request.archived)
        .collect();
    let pending_count = visible
        .iter()
        .filter(|request| request.status == PermissionStatus::Pending)
        .count();
    let active_request = visible
        .iter()
        .find(|request| request.status == PermissionStatus::Pending)
        .cloned()
        .cloned();
    let archived_count = requests.iter().filter(|r| r.archived).count();
    let mut sessions = build_session_summaries(&visible);
    // Cursor subagent conversations are nested under their parent; never list them
    // as independent top-level sessions.
    sessions.retain(|session| !excluded_session_ids.contains(&session.session_id));

    for session in sessions.iter_mut() {
        if let Some(info) = known_sessions.get(&session.session_id) {
            if session.transcript_path.is_none() && info.transcript_path.is_some() {
                session.transcript_path = info.transcript_path.clone();
            }
            if session.cwd.is_empty() || session.cwd == "." {
                if !info.cwd.is_empty() && info.cwd != "." {
                    session.cwd = info.cwd.clone();
                }
            }
        }
        session.session_host = session_host_for_summary(
            known_sessions,
            &session.session_id,
            &session.cwd,
            &session.agent,
        );
        if matches!(session.agent, AgentKind::Codex | AgentKind::Cursor)
            && session.transcript_path.is_none()
        {
            if let Some(path) = resolve_session_transcript_path_from_snapshot(
                known_sessions,
                requests,
                &session.session_id,
                &session.agent,
            ) {
                session.transcript_path = Some(path);
            }
        }
    }

    // Mark active sessions as pinned.
    for session in sessions.iter_mut() {
        session.pinned = pinned_sessions.contains(&session.session_id);
    }

    if retention_secs > 0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let active_session_ids: HashSet<&str> =
            sessions.iter().map(|s| s.session_id.as_str()).collect();

        let mut retained_map: HashMap<&str, (String, String, Option<String>, AgentKind)> =
            HashMap::new();
        for request in requests.iter().filter(|r| r.archived) {
            if active_session_ids.contains(request.session.as_str()) {
                continue;
            }
            if excluded_session_ids.contains(&request.session) {
                continue;
            }
            if matches!(request.agent, AgentKind::Codex) && is_codex_internal_cwd(&request.cwd) {
                continue;
            }
            // Pinned sessions are always retained regardless of time.
            let is_pinned = pinned_sessions.contains(&request.session);
            if !is_pinned {
                let last_seen_ts = session_last_seen
                    .get(&request.session)
                    .copied()
                    .unwrap_or_else(|| parse_iso_timestamp_secs(&request.requested_at));
                if now.saturating_sub(last_seen_ts) >= retention_secs {
                    continue;
                }
            }
            let entry = retained_map.entry(&request.session).or_insert_with(|| {
                (
                    request.cwd.clone(),
                    request.requested_at.clone(),
                    request.transcript_path.clone(),
                    request.agent.clone(),
                )
            });
            if request.requested_at > entry.1 {
                entry.0 = request.cwd.clone();
                entry.1 = request.requested_at.clone();
                entry.3 = request.agent.clone();
            }
            if entry.2.is_none() && request.transcript_path.is_some() {
                entry.2 = request.transcript_path.clone();
            }
        }

        for (session_id, (cwd, last_activity, transcript_path, agent)) in retained_map {
            let session_host = session_host_for_summary(known_sessions, session_id, &cwd, &agent);
            sessions.push(SessionSummary {
                session_id: session_id.to_string(),
                agent,
                cwd,
                pending_count: 0,
                total_count: 0,
                last_activity,
                transcript_path,
                pinned: pinned_sessions.contains(session_id),
                session_host,
                active_subagents: Vec::new(),
            });
        }

        sessions.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.pending_count.cmp(&a.pending_count))
                .then(b.last_activity.cmp(&a.last_activity))
        });
    }

    // Include known sessions (from Stop/PostToolUse events) that have no
    // permission requests – these are sessions with only text output.
    {
        let existing_ids: HashSet<String> = sessions.iter().map(|s| s.session_id.clone()).collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for (session_id, info) in known_sessions {
            if existing_ids.contains(session_id.as_str()) {
                continue;
            }
            if excluded_session_ids.contains(session_id) {
                continue;
            }
            if matches!(info.agent, AgentKind::Codex) && is_codex_internal_cwd(&info.cwd) {
                continue;
            }
            // Cursor observer hooks often omit workspace_roots; skip ghost rows until
            // we can resolve cwd or a transcript from ~/.cursor/projects.
            if matches!(info.agent, AgentKind::Cursor)
                && is_unresolved_cursor_cwd(&info.cwd)
                && info.transcript_path.is_none()
                && !pinned_sessions.contains(session_id)
            {
                continue;
            }
            // Pinned sessions always included; non-pinned filtered by retention.
            let is_pinned = pinned_sessions.contains(session_id);
            if !is_pinned && retention_secs > 0 {
                let last_seen_ts = session_last_seen
                    .get(session_id)
                    .copied()
                    .unwrap_or_else(|| parse_iso_timestamp_secs(&info.last_activity));
                if now.saturating_sub(last_seen_ts) >= retention_secs {
                    continue;
                }
            }
            sessions.push(SessionSummary {
                session_id: session_id.clone(),
                agent: info.agent.clone(),
                cwd: info.cwd.clone(),
                pending_count: 0,
                total_count: 0,
                last_activity: info.last_activity.clone(),
                transcript_path: info.transcript_path.clone(),
                pinned: is_pinned,
                session_host: if info.host != platform::SessionHost::Unknown {
                    info.host
                } else {
                    session_host_for_summary(known_sessions, session_id, &info.cwd, &info.agent)
                },
                active_subagents: Vec::new(),
            });
        }

        sessions.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.pending_count.cmp(&a.pending_count))
                .then(b.last_activity.cmp(&a.last_activity))
        });
    }

    // ZCode chat history lives in its sqlite store, so every ZCode session row
    // must expose the virtual transcript path the chat reader understands.
    // Applied after all session sources are merged (live requests, archived
    // retention, known sessions) because the hook payload paths they carry are
    // ephemeral temp files deleted as soon as the hook returns.
    for session in sessions.iter_mut() {
        if matches!(session.agent, AgentKind::Zcode) {
            session.transcript_path = zcode_db_session_path(&session.session_id);
        }
    }

    let mut daily_tokens = TokenUsage::default();
    for usage in session_token_usage.values() {
        daily_tokens.add_assign(*usage);
    }

    let active_ids: HashSet<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
    let mut active_session_tokens = TokenUsage::default();
    for (session_id, usage) in session_token_usage.iter() {
        if active_ids.contains(session_id.as_str()) {
            active_session_tokens.add_assign(*usage);
        }
    }

    IslandSnapshot {
        online,
        pending_count,
        archived_count,
        active_request,
        recent: visible
            .into_iter()
            .take(12)
            .map(|r| {
                let mut stripped = (*r).clone();
                if stripped.status != PermissionStatus::Pending {
                    stripped.tool_input = None;
                }
                stripped
            })
            .collect(),
        sessions,
        daily_tokens,
        active_session_tokens,
        daily_tokens_by_model: HashMap::new(),
        active_session_tokens_by_model: HashMap::new(),
        hook_health: HookHealthSnapshot::default(),
    }
}

fn build_session_summaries(visible: &[&PermissionRequest]) -> Vec<SessionSummary> {
    let mut session_map: HashMap<&str, (String, usize, usize, String, Option<String>, AgentKind)> =
        HashMap::new();

    for request in visible {
        if matches!(request.agent, AgentKind::Codex) && is_codex_internal_cwd(&request.cwd) {
            continue;
        }
        let entry = session_map.entry(&request.session).or_insert_with(|| {
            (
                request.cwd.clone(),
                0,
                0,
                request.requested_at.clone(),
                request.transcript_path.clone(),
                request.agent.clone(),
            )
        });
        entry.2 += 1;
        if request.status == PermissionStatus::Pending {
            entry.1 += 1;
        }
        if request.requested_at > entry.3 {
            entry.0 = request.cwd.clone();
            entry.3 = request.requested_at.clone();
            entry.5 = request.agent.clone();
        }
        if entry.4.is_none() && request.transcript_path.is_some() {
            entry.4 = request.transcript_path.clone();
        }
    }

    let mut summaries: Vec<SessionSummary> = session_map
        .into_iter()
        .map(
            |(
                session_id,
                (cwd, pending_count, total_count, last_activity, transcript_path, agent),
            )| SessionSummary {
                session_id: session_id.to_string(),
                agent,
                cwd,
                pending_count,
                total_count,
                last_activity,
                transcript_path,
                pinned: false,
                session_host: platform::SessionHost::Unknown,
                active_subagents: Vec::new(),
            },
        )
        .collect();

    summaries.sort_by(|a, b| {
        b.pending_count
            .cmp(&a.pending_count)
            .then(b.last_activity.cmp(&a.last_activity))
    });
    summaries
}

fn iso_timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    format_unix_timestamp(duration.as_secs())
}

fn format_unix_timestamp(timestamp: u64) -> String {
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

fn civil_year_and_day(days_since_epoch: u64) -> (i32, u64) {
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

fn month_and_day(year: i32, day_of_year: u64) -> (u64, u64) {
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

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod core_tests {
    use super::*;

    #[test]
    fn approval_notice_mode_normalization_falls_back_to_interrupt() {
        assert_eq!(normalize_approval_notice_mode("notify"), "notify");
        assert_eq!(normalize_approval_notice_mode("interrupt"), "interrupt");
        assert_eq!(normalize_approval_notice_mode(""), "interrupt");
        assert_eq!(normalize_approval_notice_mode("yolo"), "interrupt");
    }

    #[test]
    fn notification_language_normalization_falls_back_to_english() {
        assert_eq!(normalize_notification_language("zh-CN"), "zh-CN");
        assert_eq!(normalize_notification_language("en"), "en");
        assert_eq!(normalize_notification_language("fr"), "en");
    }

    #[test]
    fn approval_notification_copy_summarizes_command_and_project() {
        let (title_en, body_en) = approval_notification_copy(
            "Claude",
            "git push --force origin main\nsecond line",
            "/Users/dev/Atoll",
            "en",
        );
        assert_eq!(title_en, "Claude requests approval");
        assert!(body_en.starts_with("git push --force origin main"));
        assert!(body_en.contains("Atoll"));

        let (title_zh, body_zh) =
            approval_notification_copy("Claude", "rm -rf /tmp/x", "/home/dev/Atoll", "zh-CN");
        assert_eq!(title_zh, "Claude 请求批准");
        assert!(body_zh.contains("rm -rf /tmp/x"));
    }

    #[test]
    fn approval_notification_copy_truncates_long_commands() {
        let long_command = "echo ".repeat(80);
        let (_, body) = approval_notification_copy("Claude", &long_command, "/tmp/x", "en");
        assert!(body.chars().count() < 200);
        assert!(body.ends_with('…') || body.contains('…'));
    }

    #[test]
    fn archived_requests_still_appear_in_session_list_until_retention_expires() {
        let requested_at = iso_timestamp_now();
        let requests = vec![PermissionRequest {
            id: "req-1".into(),
            tool_use_id: None,
            agent: AgentKind::Claude,
            session: "session-a".into(),
            command: "Bash: ls".into(),
            detail: "List files".into(),
            cwd: "/tmp/project".into(),
            requested_at,
            status: PermissionStatus::Approved,
            archived: true,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        }];

        let snapshot = snapshot_from(
            &requests,
            &HashMap::new(),
            900,
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
            true,
            &HashSet::new(),
        );

        assert_eq!(snapshot.sessions.len(), 1);
    }

    #[test]
    fn removed_session_requests_do_not_reappear_in_session_list() {
        let snapshot = snapshot_from(
            &[],
            &HashMap::new(),
            900,
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
            true,
            &HashSet::new(),
        );

        assert!(snapshot.sessions.is_empty());
    }

    #[test]
    fn codex_memories_background_session_is_ignored() {
        let memories_cwd = dirs::home_dir()
            .expect("home dir")
            .join(".codex")
            .join("memories")
            .to_string_lossy()
            .into_owned();
        let known_sessions = HashMap::from([
            (
                "memories-thread".into(),
                KnownSession {
                    agent: AgentKind::Codex,
                    cwd: memories_cwd.clone(),
                    transcript_path: None,
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::Unknown,
                    conversation_id: None,
                },
            ),
            (
                "real-session".into(),
                KnownSession {
                    agent: AgentKind::Codex,
                    cwd: "/Users/test/project".into(),
                    transcript_path: None,
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::Unknown,
                    conversation_id: None,
                },
            ),
        ]);

        let snapshot = snapshot_from(
            &[],
            &HashMap::new(),
            900,
            &HashMap::new(),
            &known_sessions,
            &HashSet::new(),
            true,
            &HashSet::new(),
        );

        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].session_id, "real-session");
        assert!(is_codex_internal_session(
            &AgentKind::Codex,
            &memories_cwd,
            None,
        ));
        assert!(is_codex_internal_session(&AgentKind::Codex, ".", None));
        assert!(is_codex_internal_session(&AgentKind::Codex, "", None));
        assert!(!is_codex_internal_session(
            &AgentKind::Codex,
            "/Users/test/project",
            None,
        ));
        assert!(!is_codex_internal_session(
            &AgentKind::Codex,
            "/Users/test/code/Atoll/.codex",
            None,
        ));
        assert!(!is_codex_internal_session(
            &AgentKind::Codex,
            "/Users/test/.codex/sessions/2026/06/23/rollout.jsonl",
            None,
        ));
    }

    #[test]
    fn session_host_for_summary_trusts_stored_host() {
        let known_sessions = HashMap::from([
            (
                "cli-session".into(),
                KnownSession {
                    agent: AgentKind::Claude,
                    cwd: "/tmp/project".into(),
                    transcript_path: None,
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::ClaudeCli,
                    conversation_id: None,
                },
            ),
            (
                "desktop-session".into(),
                KnownSession {
                    agent: AgentKind::Claude,
                    cwd: "/tmp/desktop".into(),
                    transcript_path: None,
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::ClaudeDesktop,
                    conversation_id: None,
                },
            ),
        ]);

        assert_eq!(
            session_host_for_summary(
                &known_sessions,
                "cli-session",
                "/tmp/project",
                &AgentKind::Claude,
            ),
            platform::SessionHost::ClaudeCli,
        );
        assert_eq!(
            session_host_for_summary(
                &known_sessions,
                "desktop-session",
                "/tmp/desktop",
                &AgentKind::Claude,
            ),
            platform::SessionHost::ClaudeDesktop,
        );
    }

    #[test]
    fn session_host_from_transcript_path_when_stored_unknown() {
        let known_sessions = HashMap::from([
            (
                "cli-unknown".into(),
                KnownSession {
                    agent: AgentKind::Claude,
                    cwd: "/tmp/project".into(),
                    transcript_path: Some("/Users/test/.claude/projects/-tmp-project/abc.jsonl".into()),
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::Unknown,
                    conversation_id: None,
                },
            ),
            (
                "desktop-unknown".into(),
                KnownSession {
                    agent: AgentKind::Claude,
                    cwd: "/tmp/project".into(),
                    transcript_path: Some("/Users/test/Library/Application Support/Claude-3p/local-agent-mode-sessions/xyz.jsonl".into()),
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::Unknown,
                    conversation_id: None,
                },
            ),
        ]);

        assert_eq!(
            session_host_for_summary(
                &known_sessions,
                "cli-unknown",
                "/tmp/project",
                &AgentKind::Claude,
            ),
            platform::SessionHost::ClaudeCli,
        );
        assert_eq!(
            session_host_for_summary(
                &known_sessions,
                "desktop-unknown",
                "/tmp/project",
                &AgentKind::Claude,
            ),
            platform::SessionHost::ClaudeDesktop,
        );
    }

    #[test]
    fn host_from_claude_transcript_path_patterns() {
        assert_eq!(
            host_from_claude_transcript_path("/Users/me/.claude/projects/-tmp-project/abc.jsonl"),
            Some(platform::SessionHost::ClaudeCli),
        );
        assert_eq!(
            host_from_claude_transcript_path("/Users/me/Library/Application Support/Claude-3p/local-agent-mode-sessions/xyz.jsonl"),
            Some(platform::SessionHost::ClaudeDesktop),
        );
        assert_eq!(
            host_from_claude_transcript_path("/Users/me/Library/Application Support/com.anthropic.claudefordesktop/agent-sessions/xyz.jsonl"),
            Some(platform::SessionHost::ClaudeDesktop),
        );
        assert_eq!(
            host_from_claude_transcript_path(
                "/Users/me/Library/Application Support/Claude/projects/xyz.jsonl"
            ),
            Some(platform::SessionHost::ClaudeDesktop),
        );
        assert_eq!(
            host_from_claude_transcript_path("/some/random/path/transcript.jsonl"),
            None,
        );
    }

    #[test]
    fn host_from_codex_transcript_path_patterns() {
        assert_eq!(
            host_from_codex_transcript_path("/Users/me/.codex/sessions/2026/06/23/rollout.jsonl"),
            Some(platform::SessionHost::CodexCli),
        );
        assert_eq!(
            host_from_codex_transcript_path(
                "/Users/me/Library/Application Support/com.openai.codex/sessions/abc.jsonl"
            ),
            Some(platform::SessionHost::CodexDesktop),
        );
        assert_eq!(
            host_from_codex_transcript_path("/some/random/path/transcript.jsonl"),
            None,
        );
    }

    #[test]
    fn session_host_for_summary_codex_trusts_stored_host() {
        let known_sessions = HashMap::from([
            (
                "codex-cli-session".into(),
                KnownSession {
                    agent: AgentKind::Codex,
                    cwd: "/tmp/codex-cli".into(),
                    transcript_path: None,
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::CodexCli,
                    conversation_id: None,
                },
            ),
            (
                "codex-desktop-session".into(),
                KnownSession {
                    agent: AgentKind::Codex,
                    cwd: "/tmp/codex-desktop".into(),
                    transcript_path: None,
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::CodexDesktop,
                    conversation_id: None,
                },
            ),
        ]);

        assert_eq!(
            session_host_for_summary(
                &known_sessions,
                "codex-cli-session",
                "/tmp/codex-cli",
                &AgentKind::Codex,
            ),
            platform::SessionHost::CodexCli,
        );
        assert_eq!(
            session_host_for_summary(
                &known_sessions,
                "codex-desktop-session",
                "/tmp/codex-desktop",
                &AgentKind::Codex,
            ),
            platform::SessionHost::CodexDesktop,
        );
    }

    #[test]
    fn codex_missing_cwd_is_resolved_from_transcript() {
        let dir = std::env::temp_dir().join(format!("atoll-codex-session-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let transcript_path = dir.join("rollout-test.jsonl");
        std::fs::write(
            &transcript_path,
            r#"{"type":"session_meta","payload":{"id":"session-app","cwd":"C:/Users/test/project"}}"#,
        )
        .expect("write transcript");
        let transcript = transcript_path.to_string_lossy().into_owned();

        assert!(!is_codex_internal_session(
            &AgentKind::Codex,
            ".",
            Some(&transcript),
        ));
        assert_eq!(
            resolve_codex_session_cwd(".", Some(&transcript)),
            "C:/Users/test/project"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn codex_transcript_reader_keeps_last_messages_without_full_history() {
        let dir = std::env::temp_dir().join(format!(
            "atoll-codex-transcript-window-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let transcript_path = dir.join("session.jsonl");
        let mut content = String::from(
            r#"{"type":"session_meta","payload":{"id":"session-app","cwd":"/tmp/project"}}"#,
        );
        content.push('\n');
        for i in 0..75 {
            content.push_str(&format!(
                r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"message {i}"}}]}}}}"#
            ));
            content.push('\n');
        }
        std::fs::write(&transcript_path, content).expect("write transcript");

        let state = test_app_state();
        let messages =
            read_transcript_messages_cached(&state, &transcript_path).expect("read messages");

        assert_eq!(messages.len(), TRANSCRIPT_MAX_MESSAGES);
        assert_eq!(messages[0].content, "message 25");
        assert_eq!(messages[TRANSCRIPT_MAX_MESSAGES - 1].content, "message 74");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn transcript_cache_reads_appends_and_recovers_from_truncation() {
        use std::io::Write;

        let dir =
            std::env::temp_dir().join(format!("atoll-transcript-cache-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("session.jsonl");
        std::fs::write(
            &path,
            "{\"type\":\"assistant\",\"message\":{\"content\":\"first\"}}\n",
        )
        .expect("initial transcript");
        let state = test_app_state();
        let first = read_transcript_messages_cached(&state, &path).expect("first read");
        assert_eq!(first.len(), 1);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("append transcript");
        writeln!(
            file,
            "{{\"type\":\"assistant\",\"message\":{{\"content\":\"second\"}}}}"
        )
        .expect("append line");
        let appended = read_transcript_messages_cached(&state, &path).expect("append read");
        assert_eq!(appended.len(), 2);
        assert_eq!(appended[1].content, "second");

        std::fs::write(
            &path,
            "{\"type\":\"assistant\",\"message\":{\"content\":\"reset\"}}\n",
        )
        .expect("truncate transcript");
        let reset = read_transcript_messages_cached(&state, &path).expect("reset read");
        assert_eq!(reset.len(), 1);
        assert_eq!(reset[0].content, "reset");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn transcript_cache_bounds_initial_read_for_large_file() {
        use std::io::{Seek, SeekFrom, Write};

        let dir = std::env::temp_dir().join(format!(
            "atoll-large-transcript-cache-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("large.jsonl");
        let mut file = std::fs::File::create(&path).expect("large transcript");
        file.set_len(100 * 1024 * 1024).expect("sparse transcript");
        file.seek(SeekFrom::End(0)).expect("seek end");
        writeln!(
            file,
            "\n{{\"type\":\"assistant\",\"message\":{{\"content\":\"tail\"}}}}"
        )
        .expect("tail message");
        drop(file);

        let state = test_app_state();
        let messages = read_transcript_messages_cached(&state, &path).expect("bounded read");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "tail");
        let cached = state.transcript_cache.lock().expect("cache");
        let entry = cached.entries.get(&path).expect("cache entry");
        assert_eq!(entry.read_offset, std::fs::metadata(&path).unwrap().len());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rollover_completes_while_requests_mutex_is_held() {
        let _env = TOKEN_HISTORY_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path =
            std::env::temp_dir().join(format!("atoll-rollover-lock-{}.json", uuid::Uuid::new_v4()));
        std::env::set_var("ATOLL_TOKEN_HISTORY_PATH", &path);

        let state = Arc::new(test_app_state());
        *state.token_usage_day.lock().expect("day") = "2000-01-01".into();
        state.session_token_usage.lock().expect("usage").insert(
            "session-a".into(),
            TokenUsage {
                input_tokens: 1,
                ..TokenUsage::default()
            },
        );
        state
            .session_agent_map
            .lock()
            .expect("agent map")
            .insert("session-a".into(), "codex".into());
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let worker_state = Arc::clone(&state);
        std::thread::spawn(move || {
            let _requests = worker_state.requests.lock().expect("requests");
            roll_over_token_usage_if_needed(&worker_state);
            let _ = tx.send(());
        });
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());

        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
        let _ = std::fs::remove_file(path.with_extension("json.tmp"));
    }

    #[test]
    fn codex_session_summary_exposes_known_or_request_transcript_path() {
        let requested_at = iso_timestamp_now();
        let request_path = "/tmp/atoll-codex-request.jsonl".to_string();
        let known_path = "/tmp/atoll-codex-known.jsonl".to_string();
        let requests = vec![PermissionRequest {
            id: "req-codex".into(),
            tool_use_id: None,
            agent: AgentKind::Codex,
            session: "codex-request-session".into(),
            command: "Bash: ls".into(),
            detail: "List files".into(),
            cwd: "/tmp/request-project".into(),
            requested_at: requested_at.clone(),
            status: PermissionStatus::Approved,
            archived: false,
            supports_always: false,
            transcript_path: Some(request_path.clone()),
            tool_input: None,
        }];
        let known_sessions = HashMap::from([(
            "codex-known-session".into(),
            KnownSession {
                agent: AgentKind::Codex,
                cwd: "/tmp/known-project".into(),
                transcript_path: Some(known_path.clone()),
                last_activity: requested_at,
                host: platform::SessionHost::Unknown,
                conversation_id: None,
            },
        )]);

        let snapshot = snapshot_from(
            &requests,
            &HashMap::new(),
            900,
            &HashMap::new(),
            &known_sessions,
            &HashSet::new(),
            true,
            &HashSet::new(),
        );

        let request_session = snapshot
            .sessions
            .iter()
            .find(|session| session.session_id == "codex-request-session")
            .expect("request session");
        assert_eq!(
            request_session.transcript_path.as_deref(),
            Some(request_path.as_str())
        );

        let known_session = snapshot
            .sessions
            .iter()
            .find(|session| session.session_id == "codex-known-session")
            .expect("known session");
        assert_eq!(
            known_session.transcript_path.as_deref(),
            Some(known_path.as_str())
        );
    }

    #[test]
    fn transcript_path_validation_only_allows_known_transcripts() {
        let state = test_app_state();
        let dir = std::env::temp_dir().join(format!(
            "atoll-transcript-validation-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let transcript_path = dir.join("session.jsonl");
        let unknown_path = dir.join("unknown.jsonl");
        let note_path = dir.join("note.txt");
        std::fs::write(&transcript_path, "{}\n").expect("write transcript");
        std::fs::write(&unknown_path, "{}\n").expect("write unknown");
        std::fs::write(&note_path, "not a transcript").expect("write txt");

        {
            let mut known = state.known_sessions.lock().expect("lock");
            known.insert(
                "session-1".into(),
                KnownSession {
                    agent: AgentKind::Claude,
                    cwd: "/tmp/project".into(),
                    transcript_path: Some(transcript_path.to_string_lossy().into_owned()),
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::Unknown,
                    conversation_id: None,
                },
            );
        }

        assert_eq!(
            validate_trusted_transcript_path(&state, &transcript_path.to_string_lossy(),)
                .expect("valid known transcript"),
            dunce::canonicalize(&transcript_path).expect("canonical transcript"),
        );
        assert!(
            validate_trusted_transcript_path(&state, &unknown_path.to_string_lossy(),).is_err()
        );
        assert!(validate_trusted_transcript_path(&state, &note_path.to_string_lossy(),).is_err());
        assert!(validate_trusted_transcript_path(
            &state,
            &dir.join("nested")
                .join("..")
                .join("session.jsonl")
                .to_string_lossy(),
        )
        .is_err());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deployed_hook_assets_current_checks_script_and_bridge_module() {
        let dir = std::env::temp_dir().join(format!(
            "atoll-hook-assets-current-{}",
            uuid::Uuid::new_v4()
        ));
        let source_dir = dir.join("source");
        let deployed_dir = dir.join("deployed");
        std::fs::create_dir_all(&source_dir).expect("source dir");
        std::fs::create_dir_all(&deployed_dir).expect("deployed dir");
        let source_script = source_dir.join("atoll-codex-hook.mjs");
        let deployed_script = deployed_dir.join("atoll-codex-hook.mjs");
        let source_bridge = source_dir.join("atoll-hook-bridge.mjs");
        let deployed_bridge = deployed_dir.join("atoll-hook-bridge.mjs");

        std::fs::write(&source_script, "new script").expect("source script");
        std::fs::write(&deployed_script, "old script").expect("deployed script");
        std::fs::write(&source_bridge, "new bridge").expect("source bridge");
        std::fs::write(&deployed_bridge, "old bridge").expect("deployed bridge");

        assert!(!deployed_hook_assets_current(
            &source_script,
            &deployed_script
        ));

        std::fs::write(&deployed_script, "new script").expect("deployed script update");
        assert!(!deployed_hook_assets_current(
            &source_script,
            &deployed_script
        ));

        std::fs::write(&deployed_bridge, "new bridge").expect("deployed bridge update");
        assert!(deployed_hook_assets_current(
            &source_script,
            &deployed_script
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_hook_scripts_are_not_usable() {
        let dir = std::env::temp_dir().join(format!("atoll-empty-hook-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let empty = dir.join("atoll-claude-hook.mjs");
        std::fs::write(&empty, []).expect("empty script");
        assert!(!hook_script_is_usable(&empty));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn repo_hook_scripts_are_usable_install_sources() {
        for name in [
            "atoll-claude-hook.mjs",
            "atoll-codex-hook.mjs",
            "atoll-cursor-hook.mjs",
            "atoll-hook-bridge.mjs",
        ] {
            let path = repo_hook_script_path(name);
            assert!(
                hook_script_is_usable(&path),
                "missing usable repo hook script {}",
                path.display()
            );
        }
    }

    #[test]
    fn install_source_skips_empty_files_and_finds_repo_script() {
        let dir = std::env::temp_dir().join(format!("atoll-hook-source-{}", uuid::Uuid::new_v4()));
        let scripts = dir.join("scripts");
        std::fs::create_dir_all(&scripts).expect("scripts dir");
        let empty = scripts.join("atoll-claude-hook.mjs");
        std::fs::write(&empty, []).expect("empty local copy");

        let found = first_usable_hook_script(bundled_hook_script_candidates(
            Some(dir.as_path()),
            None,
            "atoll-claude-hook.mjs",
        ));
        assert!(found.is_some());
        let found = found.expect("repo script");
        assert!(
            found.ends_with("scripts/atoll-claude-hook.mjs")
                || found.ends_with("scripts\\atoll-claude-hook.mjs")
        );
        assert_ne!(
            std::fs::metadata(&found).expect("meta").len(),
            0,
            "install source must not be the empty local copy"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn copy_deployed_hook_file_does_not_truncate_when_source_is_destination() {
        let dir =
            std::env::temp_dir().join(format!("atoll-hook-self-copy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let script = dir.join("atoll-claude-hook.mjs");
        std::fs::write(&script, "keep me").expect("script");
        copy_deployed_hook_file(&script, &script, "hook script").expect("self copy");
        assert_eq!(std::fs::read_to_string(&script).expect("read"), "keep me");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn active_session_tokens_only_sum_visible_sessions() {
        let requests = vec![PermissionRequest {
            id: "req-active".into(),
            tool_use_id: None,
            agent: AgentKind::Claude,
            session: "session-active".into(),
            command: "Bash: ls".into(),
            detail: String::new(),
            cwd: "/tmp/active".into(),
            requested_at: iso_timestamp_now(),
            status: PermissionStatus::Approved,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        }];
        let token_usage = HashMap::from([
            (
                "session-active".into(),
                TokenUsage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_read_tokens: 0,
                    cache_creation_tokens: 0,
                },
            ),
            (
                "session-expired".into(),
                TokenUsage {
                    input_tokens: 200,
                    output_tokens: 80,
                    cache_read_tokens: 0,
                    cache_creation_tokens: 0,
                },
            ),
        ]);

        let snapshot = snapshot_from(
            &requests,
            &HashMap::new(),
            900,
            &token_usage,
            &HashMap::new(),
            &HashSet::new(),
            true,
            &HashSet::new(),
        );

        assert_eq!(snapshot.daily_tokens.input_tokens, 300);
        assert_eq!(snapshot.daily_tokens.output_tokens, 130);
        assert_eq!(snapshot.active_session_tokens.input_tokens, 100);
        assert_eq!(snapshot.active_session_tokens.output_tokens, 50);
    }

    #[test]
    fn cursor_composer_modes_all_register_in_snapshot() {
        for (mode, session_id, cwd) in [
            ("ask", "conv-mode-ask", "/tmp/ask"),
            ("agent", "conv-mode-agent", "/tmp/agent"),
            ("edit", "conv-mode-edit", "/tmp/edit"),
            ("debug", "conv-mode-debug", "/tmp/debug"),
        ] {
            let state = test_app_state();
            register_known_session(&state, session_id, AgentKind::Cursor, cwd, None);
            touch_session_activity(&state, session_id);

            let known = state.known_sessions.lock().expect("lock");
            let last_seen = state.session_last_seen.lock().expect("lock");
            let token_usage = state.session_token_usage.lock().expect("lock");
            let pinned = state.pinned_sessions.lock().expect("lock");
            let snapshot = snapshot_from(
                &[],
                &last_seen,
                DEFAULT_SESSION_RETENTION_SECS,
                &token_usage,
                &known,
                &pinned,
                true,
                &HashSet::new(),
            );

            assert_eq!(
                snapshot.sessions.len(),
                1,
                "composer_mode={mode} should produce one session"
            );
            assert_eq!(snapshot.sessions[0].session_id, session_id);
            assert_eq!(snapshot.sessions[0].cwd, cwd);
        }
    }

    #[test]
    fn cursor_before_submit_prompt_refreshes_session_activity() {
        let state = test_app_state();
        let session_id = "conv-submit-prompt";
        register_known_session(&state, session_id, AgentKind::Cursor, "/tmp/project", None);
        touch_session_activity(&state, session_id);

        let activity_after = {
            let known = state.known_sessions.lock().expect("lock");
            known
                .get(session_id)
                .map(|entry| entry.last_activity.clone())
                .expect("session")
        };
        assert!(!activity_after.is_empty());
    }

    #[test]
    fn cursor_ask_session_start_appears_in_snapshot() {
        let state = test_app_state();
        register_known_session(
            &state,
            "conv-ask-1",
            AgentKind::Cursor,
            "/tmp/ask-project",
            None,
        );
        touch_session_activity(&state, "conv-ask-1");

        let known = state.known_sessions.lock().expect("lock");
        let last_seen = state.session_last_seen.lock().expect("lock");
        let token_usage = state.session_token_usage.lock().expect("lock");
        let pinned = state.pinned_sessions.lock().expect("lock");
        let snapshot = snapshot_from(
            &[],
            &last_seen,
            DEFAULT_SESSION_RETENTION_SECS,
            &token_usage,
            &known,
            &pinned,
            true,
            &HashSet::new(),
        );

        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].session_id, "conv-ask-1");
        assert!(matches!(snapshot.sessions[0].agent, AgentKind::Cursor));
        assert_eq!(snapshot.sessions[0].cwd, "/tmp/ask-project");
    }

    #[test]
    fn decode_cursor_project_slug_recovers_workspace_path() {
        let home = dirs::home_dir().expect("home");
        let home_str = home.to_string_lossy();
        #[cfg(not(windows))]
        {
            let suffix = home_str
                .strip_prefix("/Users/")
                .unwrap_or(home_str.as_ref());
            let slug = format!("Users-{}", suffix.replace('/', "-"));
            let decoded = decode_cursor_project_slug(&slug).expect("decoded");
            assert_eq!(decoded, *home_str);
        }
        #[cfg(windows)]
        {
            let drive = home_str.chars().next().unwrap_or('C');
            let rest = &home_str[3..]; // skip "C:\"
            let slug = format!("{}-{}", drive, rest.replace('\\', "-"));
            let decoded = decode_cursor_project_slug(&slug).expect("decoded");
            assert_eq!(decoded, *home_str);
        }
    }

    #[test]
    fn discover_cursor_agent_transcript_finds_workspace_and_path() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let projects = home.join(".cursor").join("projects");
        let Ok(entries) = std::fs::read_dir(&projects) else {
            return;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let transcripts = entry.path().join("agent-transcripts");
            let Ok(conv_entries) = std::fs::read_dir(&transcripts) else {
                continue;
            };
            for conv in conv_entries.flatten() {
                if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let conv_id = conv.file_name().to_string_lossy().into_owned();
                let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
                if !jsonl.is_file() {
                    continue;
                }
                let (path, workspace) =
                    discover_cursor_agent_transcript(&conv_id).expect("discovered");
                assert_eq!(path, jsonl.to_string_lossy());
                if let Some(expected) =
                    decode_cursor_project_slug(&entry.file_name().to_string_lossy())
                {
                    assert_eq!(workspace, expected);
                }
                return;
            }
        }
    }

    #[test]
    fn discover_cursor_agent_transcript_matches_short_session_id_prefix() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let projects = home.join(".cursor").join("projects");
        let Ok(entries) = std::fs::read_dir(&projects) else {
            return;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let transcripts = entry.path().join("agent-transcripts");
            let Ok(conv_entries) = std::fs::read_dir(&transcripts) else {
                continue;
            };
            for conv in conv_entries.flatten() {
                if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let conv_id = conv.file_name().to_string_lossy().into_owned();
                let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
                if !jsonl.is_file() || conv_id.len() <= CURSOR_TRANSCRIPT_PREFIX_MIN_LEN {
                    continue;
                }
                let short_prefix = &conv_id[..CURSOR_TRANSCRIPT_PREFIX_MIN_LEN];
                let (path, _workspace) =
                    discover_cursor_agent_transcript(short_prefix).expect("prefix discover");
                assert_eq!(path, jsonl.to_string_lossy());
                return;
            }
        }
    }

    #[test]
    fn ghost_cursor_sessions_with_dot_cwd_are_hidden_from_snapshot() {
        let state = test_app_state();
        register_known_session(&state, "ghost-conv", AgentKind::Cursor, ".", None);
        touch_session_activity(&state, "ghost-conv");

        let known = state.known_sessions.lock().expect("lock");
        let last_seen = state.session_last_seen.lock().expect("lock");
        let token_usage = state.session_token_usage.lock().expect("lock");
        let pinned = state.pinned_sessions.lock().expect("lock");
        let snapshot = snapshot_from(
            &[],
            &last_seen,
            DEFAULT_SESSION_RETENTION_SECS,
            &token_usage,
            &known,
            &pinned,
            true,
            &HashSet::new(),
        );

        assert!(snapshot.sessions.is_empty());
    }

    #[test]
    fn backfill_cursor_session_metadata_links_on_disk_transcript() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let projects = home.join(".cursor").join("projects");
        let Ok(entries) = std::fs::read_dir(&projects) else {
            return;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let transcripts = entry.path().join("agent-transcripts");
            let Ok(conv_entries) = std::fs::read_dir(&transcripts) else {
                continue;
            };
            for conv in conv_entries.flatten() {
                if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let conv_id = conv.file_name().to_string_lossy().into_owned();
                let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
                if !jsonl.is_file() {
                    continue;
                }
                let workspace = decode_cursor_project_slug(&entry.file_name().to_string_lossy())
                    .unwrap_or_else(|| "/tmp/unknown".to_string());

                let state = test_app_state();
                register_known_session(&state, &conv_id, AgentKind::Cursor, &workspace, None);
                backfill_cursor_session_metadata(&state);

                let known = state.known_sessions.lock().expect("lock");
                let session = known.get(&conv_id).expect("session");
                assert_eq!(
                    session.transcript_path.as_deref(),
                    Some(jsonl.to_string_lossy().as_ref())
                );
                return;
            }
        }
    }

    #[test]
    fn resolve_session_transcript_path_recovers_from_stale_path() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let projects = home.join(".cursor").join("projects");
        let Ok(entries) = std::fs::read_dir(&projects) else {
            return;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let transcripts = entry.path().join("agent-transcripts");
            let Ok(conv_entries) = std::fs::read_dir(&transcripts) else {
                continue;
            };
            for conv in conv_entries.flatten() {
                if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let conv_id = conv.file_name().to_string_lossy().into_owned();
                let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
                if !jsonl.is_file() {
                    continue;
                }

                // Simulate a session whose stored transcript_path is broken, e.g.
                // a Windows path Cursor reported with a URI prefix or GBK mojibake.
                let state = test_app_state();
                register_known_session(
                    &state,
                    &conv_id,
                    AgentKind::Cursor,
                    ".",
                    Some("/atoll-nonexistent/broken-transcript.jsonl"),
                );

                // A stale on-disk path must not short-circuit resolution: the
                // resolver should fall back to disk discovery via the full UUID.
                let resolved = resolve_session_transcript_path(&state, &conv_id, &[]);
                assert_eq!(resolved.as_deref(), Some(jsonl.to_string_lossy().as_ref()));
                return;
            }
        }
    }

    #[test]
    fn cursor_after_agent_response_accumulates_tokens() {
        let _env_guard = TOKEN_HISTORY_ENV_LOCK
            .lock()
            .expect("token history env lock");
        let history_path = std::env::temp_dir().join(format!(
            "atoll-token-history-{}-{}.json",
            std::process::id(),
            "cursor-after-agent-response"
        ));
        let _ = std::fs::remove_file(&history_path);
        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        let state = test_app_state();
        let session_id = "conv-ask-tokens";
        let payload = json!({
            "conversation_id": session_id,
            "input_tokens": 1200,
            "output_tokens": 300
        });

        ingest_cursor_token_usage_from_payload(&state, session_id, &payload, "afterAgentResponse")
            .expect("token ingest");

        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(session_id)
            .copied()
            .expect("usage");
        assert_eq!(usage.input_tokens, 1200);
        assert_eq!(usage.output_tokens, 300);

        let follow_up = json!({
            "conversation_id": session_id,
            "token_usage": {
                "input_tokens": 400,
                "output_tokens": 100
            }
        });
        ingest_cursor_token_usage_from_payload(
            &state,
            session_id,
            &follow_up,
            "afterAgentResponse",
        )
        .expect("follow-up ingest");

        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(session_id)
            .copied()
            .expect("usage");
        assert_eq!(usage.input_tokens, 1600);
        assert_eq!(usage.output_tokens, 400);

        let _ = std::fs::remove_file(&history_path);
        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
    }

    #[test]
    fn cursor_token_ingest_accepts_usage_aliases() {
        let _env_guard = TOKEN_HISTORY_ENV_LOCK
            .lock()
            .expect("token history env lock");
        let history_path = std::env::temp_dir().join(format!(
            "atoll-token-history-{}-{}.json",
            std::process::id(),
            "cursor-usage-aliases"
        ));
        let _ = std::fs::remove_file(&history_path);
        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        let state = test_app_state();
        let session_id = "conv-usage-aliases";
        let payload = json!({
            "conversation_id": session_id,
            "usage": {
                "prompt_tokens": "1200",
                "completion_tokens": 300.0,
                "cache_read_input_tokens": 40,
                "cache_creation_input_tokens": 12
            }
        });

        ingest_cursor_token_usage_from_payload(&state, session_id, &payload, "afterAgentResponse")
            .expect("token ingest");

        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(session_id)
            .copied()
            .expect("usage");
        assert_eq!(usage.input_tokens, 1200);
        assert_eq!(usage.output_tokens, 300);
        assert_eq!(usage.cache_read_tokens, 40);
        assert_eq!(usage.cache_creation_tokens, 12);

        let _ = std::fs::remove_file(&history_path);
        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
    }

    #[test]
    fn cursor_stop_token_fallback_uses_runtime_lifecycle_signal() {
        let state = test_app_state();
        let session_id = "conv-runtime-token-signal";
        let token_payload = json!({
            "conversation_id": session_id,
            "usage": {
                "prompt_tokens": 120,
                "completion_tokens": 30
            }
        });
        let empty_payload = json!({
            "conversation_id": session_id
        });

        assert!(cursor_payload_has_token_usage(&token_payload));
        assert!(!cursor_payload_has_token_usage(&empty_payload));
        assert!(crate::hook_bridge::cursor_stop_should_ingest_tokens(
            &state,
            &token_payload
        ));

        remember_cursor_lifecycle_token_session(&state, session_id);

        assert!(cursor_lifecycle_token_seen(&state, session_id));
        assert!(!crate::hook_bridge::cursor_stop_should_ingest_tokens(
            &state,
            &token_payload
        ));
    }

    #[test]
    fn cursor_token_ingest_skips_empty_payload() {
        let state = test_app_state();
        ingest_cursor_token_usage_from_payload(
            &state,
            "conv-empty",
            &json!({ "conversation_id": "conv-empty" }),
            "afterAgentResponse",
        )
        .expect("empty ingest");

        let usage = state.session_token_usage.lock().expect("lock");
        assert!(!usage.contains_key("conv-empty"));
    }

    #[test]
    fn archived_session_tokens_still_count_toward_daily_total() {
        let token_usage = HashMap::from([(
            "session-archived".into(),
            TokenUsage {
                input_tokens: 400,
                output_tokens: 100,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        )]);

        let snapshot = snapshot_from(
            &[],
            &HashMap::new(),
            900,
            &token_usage,
            &HashMap::new(),
            &HashSet::new(),
            true,
            &HashSet::new(),
        );

        assert!(snapshot.sessions.is_empty());
        assert_eq!(snapshot.daily_tokens.input_tokens, 400);
        assert_eq!(snapshot.daily_tokens.output_tokens, 100);
        assert_eq!(snapshot.active_session_tokens.input_tokens, 0);
        assert_eq!(snapshot.active_session_tokens.output_tokens, 0);
    }

    pub(crate) fn test_app_state() -> AppState {
        AppState {
            requests: Mutex::new(Vec::new()),
            session_request_totals: Mutex::new(HashMap::new()),
            hook_waiters: Mutex::new(HashMap::new()),
            auto_approve_sessions: Mutex::new(HashSet::new()),
            compact_width: Mutex::new(COMPACT_WINDOW_WIDTH),
            compact_left_width: Mutex::new(0.0),
            presentation_generation: Arc::new(AtomicU64::new(0)),
            home_bounds: Mutex::new(None),
            notch_metrics: Mutex::new(NotchMetrics::default()),
            session_last_seen: Mutex::new(HashMap::new()),
            session_retention_secs: Mutex::new(DEFAULT_SESSION_RETENTION_SECS),
            subagent_retention_secs: Mutex::new(DEFAULT_SUBAGENT_RETENTION_SECS),
            session_token_usage: Mutex::new(HashMap::new()),
            session_token_usage_by_model: Mutex::new(HashMap::new()),
            session_agent_map: Mutex::new(HashMap::new()),
            token_usage_file_offsets: Mutex::new(HashMap::new()),
            token_usage_day: Mutex::new(current_local_day_key()),
            startup_daily_floor: Mutex::new(TokenUsage::default()),
            startup_daily_floor_by_model: Mutex::new(HashMap::new()),
            absolute_token_sessions: Mutex::new(HashSet::new()),
            daily_tokens_baseline: Mutex::new(TokenUsage::default()),
            known_sessions: Mutex::new(HashMap::new()),
            pinned_sessions: Mutex::new(HashSet::new()),
            previous_app_pid: Mutex::new(None),
            last_listening_online: Mutex::new(None),
            last_hook_health: Mutex::new(None),
            bridge_port: AtomicU16::new(0),
            bridge_auth_token: Mutex::new(uuid::Uuid::new_v4().to_string()),
            last_bridge_reachable: Mutex::new(None),
            active_subagents: Mutex::new(Vec::new()),
            cursor_subagent_conversations: Mutex::new(HashMap::new()),
            cursor_lifecycle_token_sessions: Mutex::new(HashSet::new()),
            last_subagent_snapshot_emit: Mutex::new(Instant::now() - Duration::from_secs(10)),
            snapshot_debounce_generation: AtomicU64::new(0),
            snapshot_debounce_worker_running: AtomicBool::new(false),
            last_subagent_reconcile: Mutex::new(Instant::now() - Duration::from_secs(10)),
            last_hook_activity: Mutex::new(Instant::now()),
            token_history_dirty: AtomicBool::new(false),
            transcript_cache: Mutex::new(TranscriptCache::default()),
            media_card_enabled: Mutex::new(true),
            artwork_backdrop_enabled: Mutex::new(false),
            clipboard_history: Mutex::new(Vec::new()),
            clipboard_history_limit: Mutex::new(clipboard_history::DEFAULT_MAX_ENTRIES),
            clipboard_history_enabled: Mutex::new(false),
            lyrics_enabled: Mutex::new(false),
            lyrics: Mutex::new(None),
            lyrics_track_key: Mutex::new(String::new()),
            approval_notice_mode: Mutex::new(APPROVAL_NOTICE_INTERRUPT.to_string()),
            notification_language: Mutex::new("en".to_string()),
            global_shortcuts: Mutex::new(shortcuts::GlobalShortcutsState::default()),
        }
    }

    #[test]
    fn effective_daily_tokens_avoids_restart_transcript_double_count() {
        let startup_floor = TokenUsage {
            input_tokens: 3_000_000,
            output_tokens: 1_200_000,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        let session_usage = HashMap::from([(
            "session-rescan".into(),
            TokenUsage {
                input_tokens: 2_000_000,
                output_tokens: 800_000,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        )]);
        let absolute_sessions = HashSet::from(["session-rescan".into()]);

        let daily = effective_daily_tokens(&session_usage, startup_floor, &absolute_sessions);
        assert_eq!(daily.input_tokens, 3_000_000);
        assert_eq!(daily.output_tokens, 1_200_000);

        let hook_only = HashMap::from([(
            "session-new".into(),
            TokenUsage {
                input_tokens: 500,
                output_tokens: 100,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        )]);
        let daily = effective_daily_tokens(&hook_only, startup_floor, &HashSet::new());
        assert_eq!(daily.input_tokens, 3_000_500);
        assert_eq!(daily.output_tokens, 1_200_100);
    }

    #[test]
    fn effective_daily_tokens_by_model_uses_startup_floor() {
        let usage = |input: u64| TokenUsage {
            input_tokens: input,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        let floor = HashMap::from([("gpt-4o".into(), usage(1_000_000))]);
        let live = HashMap::from([(
            "session-new".into(),
            HashMap::from([("gpt-4o".into(), usage(200_000))]),
        )]);
        let merged = effective_daily_tokens_by_model(&live, &floor, &HashSet::new());
        assert_eq!(merged.get("gpt-4o").unwrap().input_tokens, 1_200_000);

        let absolute = HashSet::from(["session-new".into()]);
        let merged_abs = effective_daily_tokens_by_model(&live, &floor, &absolute);
        assert_eq!(merged_abs.get("gpt-4o").unwrap().input_tokens, 1_000_000);
    }

    #[test]
    fn cursor_session_end_uses_max_for_cumulative_totals() {
        let _env_guard = TOKEN_HISTORY_ENV_LOCK
            .lock()
            .expect("token history env lock");
        let history_path = std::env::temp_dir().join(format!(
            "atoll-token-history-{}-{}.json",
            std::process::id(),
            "cursor-session-end"
        ));
        let _ = std::fs::remove_file(&history_path);
        let _ = std::fs::remove_file(history_path.with_extension("json.bak"));
        let _ = std::fs::remove_file(history_path.with_extension("json.tmp"));
        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        let state = test_app_state();
        let session_id = "conv-session-end";
        ingest_cursor_token_usage_from_payload(
            &state,
            session_id,
            &json!({ "input_tokens": 1200, "output_tokens": 300 }),
            "afterAgentResponse",
        )
        .expect("turn ingest");
        ingest_cursor_token_usage_from_payload(
            &state,
            session_id,
            &json!({ "input_tokens": 1500, "output_tokens": 400 }),
            "sessionEnd",
        )
        .expect("session end ingest");

        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(session_id)
            .copied()
            .expect("usage");
        assert_eq!(usage.input_tokens, 1500);
        assert_eq!(usage.output_tokens, 400);

        let _ = std::fs::remove_file(&history_path);
        let _ = std::fs::remove_file(history_path.with_extension("json.bak"));
        let _ = std::fs::remove_file(history_path.with_extension("json.tmp"));
        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
    }

    #[test]
    fn rollover_flushes_previous_local_day_before_clearing_usage() {
        use chrono::{Duration, Local};

        let _env_guard = TOKEN_HISTORY_ENV_LOCK
            .lock()
            .expect("token history env lock");
        let history_path = std::env::temp_dir().join(format!(
            "atoll-token-history-{}-{}.json",
            std::process::id(),
            "rollover-test"
        ));
        let _ = std::fs::remove_file(&history_path);
        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        let state = test_app_state();
        let flushed_day = (Local::now().date_naive() - Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        {
            let mut usage_day = state.token_usage_day.lock().expect("lock");
            *usage_day = flushed_day.clone();
        }

        {
            let mut usage = state.session_token_usage.lock().expect("lock");
            usage.insert(
                "session-rollover".into(),
                TokenUsage {
                    input_tokens: 250,
                    output_tokens: 75,
                    cache_read_tokens: 0,
                    cache_creation_tokens: 0,
                },
            );
        }

        roll_over_token_usage_if_needed(&state);

        let usage_after = state.session_token_usage.lock().expect("lock");
        assert!(usage_after.is_empty());

        let history = token_history::get_token_history(365).expect("history");
        let flushed = history
            .days
            .iter()
            .find(|day| day.date == flushed_day)
            .expect("previous day should be persisted");
        assert_eq!(flushed.usage.input_tokens, 250);
        assert_eq!(flushed.usage.output_tokens, 75);

        let _ = std::fs::remove_file(&history_path);
        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
    }

    #[test]
    fn restart_preserves_historical_days_when_sessions_are_empty() {
        use chrono::{Duration, Local};

        let _env_guard = TOKEN_HISTORY_ENV_LOCK
            .lock()
            .expect("token history env lock");
        let history_path = std::env::temp_dir().join(format!(
            "atoll-token-history-{}-{}.json",
            std::process::id(),
            "restart-test"
        ));
        let _ = std::fs::remove_file(&history_path);

        let today = Local::now().date_naive();
        let yesterday = today - Duration::days(1);
        let two_days_ago = today - Duration::days(2);
        let today_key = today.format("%Y-%m-%d").to_string();
        let yesterday_key = yesterday.format("%Y-%m-%d").to_string();
        let two_days_ago_key = two_days_ago.format("%Y-%m-%d").to_string();

        let seed = serde_json::json!({
            "version": 1,
            "timezone": "Asia/Shanghai",
            "days": {
                &two_days_ago_key: {
                    "inputTokens": 1000,
                    "outputTokens": 500,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "byAgent": { "claude": { "inputTokens": 1000, "outputTokens": 500, "cacheReadTokens": 0, "cacheCreationTokens": 0 } }
                },
                &yesterday_key: {
                    "inputTokens": 2000,
                    "outputTokens": 800,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "byAgent": { "codex": { "inputTokens": 2000, "outputTokens": 800, "cacheReadTokens": 0, "cacheCreationTokens": 0 } }
                },
                &today_key: {
                    "inputTokens": 3000,
                    "outputTokens": 1200,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "byAgent": { "claude": { "inputTokens": 3000, "outputTokens": 1200, "cacheReadTokens": 0, "cacheCreationTokens": 0 } }
                }
            }
        });
        std::fs::write(
            &history_path,
            serde_json::to_string_pretty(&seed).expect("serialize"),
        )
        .expect("write seed history");

        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        // Simulate app restart: baseline loaded from persisted file, sessions empty.
        let baseline = token_history::load_today_baseline();
        assert_eq!(baseline.input_tokens, 3000);
        assert_eq!(baseline.output_tokens, 1200);

        let state = test_app_state();
        *state.daily_tokens_baseline.lock().expect("lock") = baseline;
        *state.startup_daily_floor.lock().expect("lock") = baseline;

        // First snapshot sync with no active sessions (upgrade/restart edge case).
        token_history::sync_today_to_history(&state).expect("sync");

        let history = token_history::get_token_history(365).expect("history");
        let past_two = history
            .days
            .iter()
            .find(|day| day.date == two_days_ago_key)
            .expect("two days ago");
        let past_one = history
            .days
            .iter()
            .find(|day| day.date == yesterday_key)
            .expect("yesterday");
        assert_eq!(past_two.usage.input_tokens, 1000);
        assert_eq!(past_two.usage.output_tokens, 500);
        assert_eq!(past_one.usage.input_tokens, 2000);
        assert_eq!(past_one.usage.output_tokens, 800);

        // Today's file value must also be preserved (not overwritten with zeros).
        let today_record = history
            .days
            .iter()
            .find(|day| day.date == today_key)
            .expect("today");
        assert_eq!(today_record.usage.input_tokens, 3000);
        assert_eq!(today_record.usage.output_tokens, 1200);

        // UI floor: daily total must not drop below persisted baseline.
        let live_daily = effective_daily_tokens(&HashMap::new(), baseline, &HashSet::new());
        assert_eq!(live_daily.input_tokens, 3000);
        assert_eq!(live_daily.output_tokens, 1200);

        // Post-restart hook increments must add on top of the startup floor.
        let post_restart = HashMap::from([(
            "session-new".into(),
            TokenUsage {
                input_tokens: 500,
                output_tokens: 100,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        )]);
        let combined = effective_daily_tokens(&post_restart, baseline, &HashSet::new());
        assert_eq!(combined.input_tokens, 3500);
        assert_eq!(combined.output_tokens, 1300);

        let _ = std::fs::remove_file(&history_path);
        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
    }

    #[test]
    fn full_scan_does_not_regress_session_token_usage() {
        let _env_guard = TOKEN_HISTORY_ENV_LOCK
            .lock()
            .expect("token history env lock");
        let history_path = std::env::temp_dir().join(format!(
            "atoll-token-history-{}-{}.json",
            std::process::id(),
            "full-scan-regression"
        ));
        let _ = std::fs::remove_file(&history_path);
        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        let state = test_app_state();
        let session_id = "session-rescan";
        let dir = std::env::temp_dir().join(format!("atoll-token-rescan-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let transcript_path = dir.join("transcript.jsonl");
        // Empty transcript simulates rotation/truncation after hooks already counted usage.
        std::fs::write(&transcript_path, "").expect("write transcript");
        let transcript = transcript_path.to_string_lossy().into_owned();

        {
            let mut usage = state.session_token_usage.lock().expect("lock");
            usage.insert(
                session_id.into(),
                TokenUsage {
                    input_tokens: 8000,
                    output_tokens: 2000,
                    cache_read_tokens: 0,
                    cache_creation_tokens: 0,
                },
            );
        }

        refresh_session_token_usage(
            &state,
            session_id,
            Some(transcript.as_str()),
            Some(&AgentKind::Claude),
        )
        .expect("refresh");

        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(session_id)
            .copied()
            .expect("usage");
        assert_eq!(usage.input_tokens, 8000);
        assert_eq!(usage.output_tokens, 2000);

        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_file(&history_path);
        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
    }

    #[test]
    fn auto_archive_retention_purge_preserves_session_token_usage() {
        let state = test_app_state();
        let session_id = "session-auto-archived".to_string();
        let token_usage = TokenUsage {
            input_tokens: 500,
            output_tokens: 120,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };

        {
            let mut usage = state.session_token_usage.lock().expect("lock");
            usage.insert(session_id.clone(), token_usage);
        }
        {
            let mut known = state.known_sessions.lock().expect("lock");
            known.insert(
                session_id.clone(),
                KnownSession {
                    agent: AgentKind::Claude,
                    cwd: "/tmp/project".into(),
                    transcript_path: Some("/tmp/project/transcript.jsonl".into()),
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::Unknown,
                    conversation_id: None,
                },
            );
        }
        {
            let mut sticky = state.session_agent_map.lock().expect("lock");
            sticky.insert(session_id.clone(), "claude".to_string());
        }

        // Simulate auto-archive timer purging a retention-expired known session.
        purge_tracked_session(&state, &session_id, Some("/tmp/project/transcript.jsonl"));

        let usage_after = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(&session_id)
            .copied()
            .expect("token usage should survive retention purge");
        assert_eq!(usage_after.input_tokens, 500);
        assert_eq!(usage_after.output_tokens, 120);

        let known_after = state.known_sessions.lock().expect("lock");
        assert!(!known_after.contains_key(&session_id));

        let token_usage_map = state.session_token_usage.lock().expect("lock");
        let snapshot = snapshot_from(
            &[],
            &HashMap::new(),
            900,
            &token_usage_map,
            &HashMap::new(),
            &HashSet::new(),
            true,
            &HashSet::new(),
        );
        assert_eq!(snapshot.daily_tokens.input_tokens, 500);
        assert_eq!(snapshot.daily_tokens.output_tokens, 120);
        assert_eq!(snapshot.active_session_tokens.input_tokens, 0);
    }

    #[test]
    fn expanded_window_is_560_by_320() {
        let size = island_window_logical_size(
            IslandWindowMode::Expanded,
            COMPACT_WINDOW_WIDTH,
            NotchMetrics::default(),
            false,
            false,
            false,
        );

        assert_eq!(size, LogicalSize::new(560.0, 320.0));
    }

    #[test]
    fn expanded_idle_window_is_shorter() {
        let size = island_window_logical_size(
            IslandWindowMode::Expanded,
            COMPACT_WINDOW_WIDTH,
            NotchMetrics::default(),
            true,
            false,
            false,
        );

        assert_eq!(size, LogicalSize::new(560.0, EXPANDED_IDLE_WINDOW_HEIGHT));
    }

    #[test]
    fn expanded_plan_window_is_taller() {
        let size = island_window_logical_size(
            IslandWindowMode::Expanded,
            COMPACT_WINDOW_WIDTH,
            NotchMetrics::default(),
            false,
            true,
            false,
        );

        assert_eq!(
            size,
            LogicalSize::new(EXPANDED_PLAN_WINDOW_WIDTH, EXPANDED_PLAN_WINDOW_HEIGHT)
        );
    }

    #[test]
    fn expanded_settings_window_is_larger() {
        let size = island_window_logical_size(
            IslandWindowMode::Expanded,
            COMPACT_WINDOW_WIDTH,
            NotchMetrics::default(),
            false,
            false,
            true,
        );

        assert_eq!(
            size,
            LogicalSize::new(
                EXPANDED_SETTINGS_WINDOW_WIDTH,
                EXPANDED_SETTINGS_WINDOW_HEIGHT,
            )
        );
    }

    #[test]
    fn tray_contains_only_show_and_quit() {
        assert_eq!(
            tray_menu_entries(),
            [("show", "Show Atoll"), ("quit", "Quit")]
        );
    }

    #[test]
    fn paused_position_freezes_creep_but_allows_seeks() {
        // Mirrors QQ Music behavior measured live: elapsedTime keeps
        // advancing while paused and snaps back on resume.
        let mut held: Option<f64> = None;
        let mut prev_raw: Option<f64> = None;
        let step = |raw, playing, prev, held: &mut Option<f64>| {
            sanitize_paused_position(Some(raw), playing, prev, held)
        };

        // Playing positions pass through.
        assert_eq!(step(71.2, true, prev_raw, &mut held), Some(71.2));
        prev_raw = Some(71.2);

        // First paused sample lands within the last playing position's
        // creep window — held at the last playing value.
        assert_eq!(step(71.8, false, prev_raw, &mut held), Some(71.2));
        prev_raw = Some(71.8);

        // Subsequent paused creep stays frozen.
        assert_eq!(step(72.9, false, prev_raw, &mut held), Some(71.2));
        prev_raw = Some(72.9);
        assert_eq!(step(73.99, false, prev_raw, &mut held), Some(71.2));
        prev_raw = Some(73.99);

        // A forward seek while paused (≥2s in one poll) is adopted.
        assert_eq!(step(100.0, false, prev_raw, &mut held), Some(100.0));
        prev_raw = Some(100.0);

        // A backward seek while paused is adopted.
        assert_eq!(step(50.0, false, prev_raw, &mut held), Some(50.0));
        prev_raw = Some(50.0);

        // Resume passes the snapped-back true position through.
        assert_eq!(step(71.0, true, prev_raw, &mut held), Some(71.0));
    }

    #[test]
    fn paused_position_none_and_cold_start_pass_through() {
        let mut held: Option<f64> = None;
        // No history: adopt whatever arrives.
        assert_eq!(
            sanitize_paused_position(Some(30.0), false, None, &mut held),
            Some(30.0)
        );
        // None (player omitted elapsedTime) passes through as None.
        assert_eq!(
            sanitize_paused_position(None, false, Some(30.0), &mut held),
            None
        );
        assert_eq!(held, None);
    }

    #[test]
    fn window_animation_interpolates_to_exact_endpoints() {
        assert_eq!(interpolate_f64(132.0, 560.0, ease_out_cubic(0.0)), 132.0);
        assert_eq!(interpolate_f64(132.0, 560.0, ease_out_cubic(1.0)), 560.0);
        assert_eq!(interpolate_f64(100.0, -20.0, ease_out_cubic(0.0)), 100.0);
        assert_eq!(interpolate_f64(100.0, -20.0, ease_out_cubic(1.0)), -20.0);
        assert!((ease_out_spring(0.0) - 0.0).abs() < 1e-9);
        assert!((ease_out_spring(1.0) - 1.0).abs() < 1e-9);
        // Mild overshoot: mid-late progress exceeds 1.0 briefly.
        assert!(ease_out_spring(0.75) > 1.0);
        assert!(ease_out_spring(0.75) < 1.08);
        // Monotonic approach after the overshoot peak, no re-dip below target
        // at the very end (single clean settle).
        assert!(ease_out_spring(0.95) >= 0.999);
        assert!(ease_out_spring(0.95) <= ease_out_spring(0.75) + 1e-6);
        // Fast launch: reaches half the distance in under a quarter of the
        // animation window, keeping the snappy start of the old back-ease.
        assert!(ease_out_spring(0.2) > 0.5);
    }

    #[test]
    fn camera_housing_is_detected_from_auxiliary_top_areas() {
        // Notch present: the menu-bar halves leave a gap (the housing).
        assert!(has_camera_housing(1512.0, 700.0, 700.0));
        // No notch: the halves span the full width.
        assert!(!has_camera_housing(1512.0, 756.0, 756.0));
        // Missing auxiliary areas are treated as "no notch".
        assert!(!has_camera_housing(1512.0, 0.0, 0.0));
    }

    #[test]
    fn notch_width_never_drops_below_the_fallback_floor() {
        // 1512 - 700 - 700 + 4 = 116, clamped up to the fallback floor.
        assert_eq!(
            notch_logical_width(1512.0, 700.0, 700.0, FALLBACK_NOTCH_WIDTH),
            FALLBACK_NOTCH_WIDTH
        );
        // A wider gap is reported verbatim once it exceeds the floor.
        assert_eq!(notch_logical_width(1512.0, 600.0, 600.0, 200.0), 316.0);
        // Without auxiliary areas we fall back.
        assert_eq!(notch_logical_width(1512.0, 0.0, 0.0, 200.0), 200.0);
    }

    #[test]
    fn notched_display_widens_to_notch_width() {
        let notch = NotchMetrics {
            has_notch: true,
            width: 200.0,
            height: 38.0,
            ..NotchMetrics::default()
        };
        let compact = island_window_logical_size(
            IslandWindowMode::Compact,
            132.0,
            notch,
            false,
            false,
            false,
        );
        // Compact sits in the menu-bar band (like dormant) — no extra_top.
        assert_eq!(compact.height, COMPACT_WINDOW_HEIGHT);
        // Width is clamped up to the notch width so the capsule visually
        // fuses with the camera housing (Dynamic-Island style).
        assert_eq!(compact.width, 200.0);

        // Content wider than the notch keeps its own width.
        let wide = island_window_logical_size(
            IslandWindowMode::Compact,
            300.0,
            notch,
            false,
            false,
            false,
        );
        assert_eq!(wide.width, 300.0);

        // Dormant is slightly wider than the notch (padding on each side).
        let dormant = island_window_logical_size(
            IslandWindowMode::Dormant,
            132.0,
            notch,
            false,
            false,
            false,
        );
        assert_eq!(dormant.width, 200.0 + 2.0 * DORMANT_NOTCH_PADDING);
        assert_eq!(dormant.height, DORMANT_WINDOW_HEIGHT);
    }

    #[test]
    fn dormant_window_is_centered_on_notched_displays() {
        let notch = NotchMetrics {
            has_notch: true,
            width: 200.0,
            height: 38.0,
            ..NotchMetrics::default()
        };
        let center_x = 756.0;
        let dormant_width = 200.0 + 2.0 * DORMANT_NOTCH_PADDING;
        let origin = compact_window_origin_x(
            center_x,
            dormant_width,
            notch,
            0.0,
            IslandWindowMode::Dormant,
        );
        assert_eq!(origin, center_x - dormant_width / 2.0);
    }

    #[test]
    fn compact_window_anchors_left_column_before_the_notch() {
        let notch = NotchMetrics {
            has_notch: true,
            width: 200.0,
            height: 38.0,
            ..NotchMetrics::default()
        };
        let center_x = 756.0;
        let left_pane = 58.0;
        let origin =
            compact_window_origin_x(center_x, 460.0, notch, left_pane, IslandWindowMode::Compact);
        assert_eq!(origin, center_x - notch.width / 2.0 - left_pane);
    }

    #[test]
    fn non_notched_display_uses_minimum_comfortable_width() {
        let no_notch = NotchMetrics::default();

        // Compact: content width is kept as-is on non-notched displays.
        let compact = island_window_logical_size(
            IslandWindowMode::Compact,
            132.0,
            no_notch,
            false,
            false,
            false,
        );
        assert_eq!(compact.width, 132.0);
        assert_eq!(compact.height, COMPACT_WINDOW_HEIGHT);

        // A compact_width that already exceeds the floor is kept as-is.
        let wide = island_window_logical_size(
            IslandWindowMode::Compact,
            250.0,
            no_notch,
            false,
            false,
            false,
        );
        assert_eq!(wide.width, 250.0);

        // Dormant: uses the same FALLBACK_NOTCH_WIDTH reference + padding.
        let dormant = island_window_logical_size(
            IslandWindowMode::Dormant,
            132.0,
            no_notch,
            false,
            false,
            false,
        );
        assert_eq!(
            dormant.width,
            FALLBACK_NOTCH_WIDTH + 2.0 * DORMANT_NOTCH_PADDING
        );
        assert_eq!(dormant.height, DORMANT_WINDOW_HEIGHT);
    }

    #[test]
    fn micro_window_is_a_thin_top_strip() {
        let wide = island_window_logical_size(
            IslandWindowMode::Micro,
            104.0,
            NotchMetrics::default(),
            false,
            false,
            false,
        );
        assert_eq!(wide.width, 104.0);
        assert_eq!(wide.height, MICRO_WINDOW_HEIGHT);
        let narrow = island_window_logical_size(
            IslandWindowMode::Micro,
            48.0,
            NotchMetrics::default(),
            false,
            false,
            false,
        );
        assert_eq!(narrow.width, MICRO_WINDOW_WIDTH);
    }

    #[test]
    fn micro_presentation_width_does_not_clamp_to_saved_compact_width() {
        assert_eq!(
            resolve_presentation_width(IslandWindowMode::Micro, Some(104.0), 220.0),
            104.0
        );
        assert_eq!(
            resolve_presentation_width(IslandWindowMode::Micro, None, 220.0),
            MICRO_WINDOW_WIDTH
        );
        assert_eq!(
            resolve_presentation_width(IslandWindowMode::Compact, None, 220.0),
            220.0
        );
        assert_eq!(
            resolve_presentation_width(IslandWindowMode::Compact, Some(180.0), 220.0),
            180.0
        );
    }

    #[test]
    fn micro_mode_skips_compact_width_persistence() {
        assert!(!should_persist_compact_width(IslandWindowMode::Micro));
        assert!(should_persist_compact_width(IslandWindowMode::Compact));
    }

    #[test]
    fn collapsed_pass_through_includes_micro() {
        assert!(is_collapsed_pass_through_mode(IslandWindowMode::Micro));
        assert!(is_collapsed_pass_through_mode(IslandWindowMode::Compact));
        assert!(!is_collapsed_pass_through_mode(IslandWindowMode::Expanded));
    }

    #[test]
    fn appkit_frame_places_the_window_at_the_screen_top() {
        fn appkit_window_origin_y(
            screen_origin_y: f64,
            screen_height: f64,
            window_height: f64,
            desired_top_y: f64,
            monitor_top_y: f64,
        ) -> f64 {
            screen_origin_y + screen_height - (desired_top_y - monitor_top_y) - window_height
        }

        assert_eq!(appkit_window_origin_y(0.0, 1260.0, 28.0, 0.0, 0.0), 1232.0);
        assert_eq!(appkit_window_origin_y(0.0, 1260.0, 320.0, 0.0, 0.0), 940.0);
    }
}

#[cfg(test)]
mod cursor_subagent_tests {
    use super::*;
    use serde_json::json;

    fn test_app_state() -> AppState {
        AppState {
            requests: Mutex::new(Vec::new()),
            session_request_totals: Mutex::new(HashMap::new()),
            hook_waiters: Mutex::new(HashMap::new()),
            auto_approve_sessions: Mutex::new(HashSet::new()),
            compact_width: Mutex::new(COMPACT_WINDOW_WIDTH),
            compact_left_width: Mutex::new(0.0),
            presentation_generation: Arc::new(AtomicU64::new(0)),
            home_bounds: Mutex::new(None),
            notch_metrics: Mutex::new(NotchMetrics::default()),
            session_last_seen: Mutex::new(HashMap::new()),
            session_retention_secs: Mutex::new(DEFAULT_SESSION_RETENTION_SECS),
            subagent_retention_secs: Mutex::new(DEFAULT_SUBAGENT_RETENTION_SECS),
            session_token_usage: Mutex::new(HashMap::new()),
            session_token_usage_by_model: Mutex::new(HashMap::new()),
            session_agent_map: Mutex::new(HashMap::new()),
            token_usage_file_offsets: Mutex::new(HashMap::new()),
            token_usage_day: Mutex::new(current_local_day_key()),
            startup_daily_floor: Mutex::new(TokenUsage::default()),
            startup_daily_floor_by_model: Mutex::new(HashMap::new()),
            absolute_token_sessions: Mutex::new(HashSet::new()),
            daily_tokens_baseline: Mutex::new(TokenUsage::default()),
            known_sessions: Mutex::new(HashMap::new()),
            pinned_sessions: Mutex::new(HashSet::new()),
            previous_app_pid: Mutex::new(None),
            last_listening_online: Mutex::new(None),
            last_hook_health: Mutex::new(None),
            bridge_port: AtomicU16::new(0),
            bridge_auth_token: Mutex::new(uuid::Uuid::new_v4().to_string()),
            last_bridge_reachable: Mutex::new(None),
            active_subagents: Mutex::new(Vec::new()),
            cursor_subagent_conversations: Mutex::new(HashMap::new()),
            cursor_lifecycle_token_sessions: Mutex::new(HashSet::new()),
            last_subagent_snapshot_emit: Mutex::new(Instant::now() - Duration::from_secs(10)),
            snapshot_debounce_generation: AtomicU64::new(0),
            snapshot_debounce_worker_running: AtomicBool::new(false),
            last_subagent_reconcile: Mutex::new(Instant::now() - Duration::from_secs(10)),
            last_hook_activity: Mutex::new(Instant::now()),
            token_history_dirty: AtomicBool::new(false),
            transcript_cache: Mutex::new(TranscriptCache::default()),
            media_card_enabled: Mutex::new(true),
            artwork_backdrop_enabled: Mutex::new(false),
            clipboard_history: Mutex::new(Vec::new()),
            clipboard_history_limit: Mutex::new(clipboard_history::DEFAULT_MAX_ENTRIES),
            clipboard_history_enabled: Mutex::new(false),
            lyrics_enabled: Mutex::new(false),
            lyrics: Mutex::new(None),
            lyrics_track_key: Mutex::new(String::new()),
            approval_notice_mode: Mutex::new(APPROVAL_NOTICE_INTERRUPT.to_string()),
            notification_language: Mutex::new("en".to_string()),
            global_shortcuts: Mutex::new(shortcuts::GlobalShortcutsState::default()),
        }
    }

    #[test]
    fn payload_helpers_support_claude_and_cursor_fields() {
        let claude = json!({
            "agent_id": "agent-claude",
            "session_id": "sess-claude",
            "agent_type": "explore"
        });
        assert_eq!(payload_subagent_id(&claude), Some("agent-claude"));
        assert_eq!(
            payload_subagent_parent_session_id(&claude),
            Some("sess-claude")
        );
        assert_eq!(payload_subagent_type(&claude), "explore");

        let cursor = json!({
            "subagent_id": "sub-123",
            "conversation_id": "conv-parent",
            "subagent_type": "generalPurpose"
        });
        assert_eq!(payload_subagent_id(&cursor), Some("sub-123"));
        assert_eq!(
            payload_subagent_parent_session_id(&cursor),
            Some("conv-parent")
        );
        assert_eq!(payload_subagent_type(&cursor), "generalPurpose");
    }

    #[test]
    fn cursor_subagent_start_registers_subagent() {
        let state = test_app_state();
        let payload = json!({
            "hook_event_name": "subagentStart",
            "subagent_id": "sub-abc",
            "conversation_id": "conv-parent",
            "subagent_type": "explore",
            "transcript_path": "/tmp/main.jsonl"
        });
        register_subagent_start(&state, &payload, AgentKind::Cursor);

        let subagents = state.active_subagents.lock().expect("lock");
        assert_eq!(subagents.len(), 1);
        assert_eq!(subagents[0].agent_id, "sub-abc");
        assert_eq!(subagents[0].session_id, "conv-parent");
        assert_eq!(subagents[0].agent_type, "explore");
        assert!(subagents[0].completed_at.is_none());
    }

    #[test]
    fn cursor_subagent_stop_completes_without_agent_id() {
        let state = test_app_state();
        register_subagent_start(
            &state,
            &json!({
                "subagent_id": "sub-abc",
                "conversation_id": "conv-parent",
                "subagent_type": "explore"
            }),
            AgentKind::Cursor,
        );

        complete_subagent(
            &state,
            &json!({
                "hook_event_name": "subagentStop",
                "conversation_id": "conv-parent",
                "subagent_type": "explore",
                "summary": "Found auth module",
                "agent_transcript_path": "/tmp/subagents/agent-sub-abc.jsonl"
            }),
        );

        let subagents = state.active_subagents.lock().expect("lock");
        assert_eq!(subagents.len(), 1);
        assert!(subagents[0].completed_at.is_some());
        assert_eq!(
            subagents[0].agent_transcript_path.as_deref(),
            Some("/tmp/subagents/agent-sub-abc.jsonl")
        );
        assert_eq!(
            subagents[0].last_message.as_deref(),
            Some("Found auth module")
        );
    }

    #[test]
    fn cursor_subagent_conversation_maps_to_parent_session() {
        let state = test_app_state();
        register_subagent_start(
            &state,
            &json!({
                "subagent_id": "sub-abc",
                "conversation_id": "conv-parent",
                "subagent_type": "explore"
            }),
            AgentKind::Cursor,
        );

        let parent = resolve_cursor_session_for_payload(
            &state,
            &json!({
                "conversation_id": "conv-subagent-new",
                "hook_event_name": "preToolUse"
            }),
        );
        assert_eq!(parent.as_deref(), Some("conv-parent"));

        let map = state.cursor_subagent_conversations.lock().expect("lock");
        assert_eq!(
            map.get("conv-subagent-new").map(String::as_str),
            Some("conv-parent")
        );

        let subagents = state.active_subagents.lock().expect("lock");
        assert_eq!(
            subagents[0].conversation_id.as_deref(),
            Some("conv-subagent-new")
        );
    }

    #[test]
    fn derive_subagent_transcript_path_uses_parent_directory() {
        let parent_uuid = "819943d1-a823-47ce-bef3-97ca63fa0f34";
        let sub_uuid = "60bcad01-8db6-4e9f-91b3-d3e55f2b504c";
        let dir =
            std::env::temp_dir().join(format!("atoll-subagent-derive-{}", std::process::id()));
        let parent_dir = dir.join(parent_uuid);
        let subagents_dir = parent_dir.join("subagents");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
        let main = parent_dir.join(format!("{parent_uuid}.jsonl"));
        std::fs::write(&main, "{}").expect("write parent transcript");
        let sub_path = subagents_dir.join(format!("{sub_uuid}.jsonl"));
        std::fs::write(&sub_path, "{}").expect("write subagent transcript");
        let main_str = main.to_string_lossy().into_owned();

        let resolved =
            derive_subagent_transcript_path(Some(&main_str), "call_tool_id", Some(sub_uuid), None)
                .expect("resolved path");

        assert_eq!(resolved, sub_path.to_string_lossy().into_owned());
        assert!(
            !resolved.contains(&format!("{parent_uuid}/{parent_uuid}/subagents")),
            "should not nest an extra parent-uuid directory"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cursor_subagent_conversation_binding_updates_transcript_path() {
        let state = test_app_state();
        let parent_uuid = "conv-parent";
        let sub_uuid = "conv-subagent-new";
        let dir = std::env::temp_dir().join(format!("atoll-subagent-bind-{}", std::process::id()));
        let parent_dir = dir.join(parent_uuid);
        let subagents_dir = parent_dir.join("subagents");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
        let main = parent_dir.join(format!("{parent_uuid}.jsonl"));
        std::fs::write(&main, "{}").expect("write parent transcript");
        let sub_path = subagents_dir.join(format!("{sub_uuid}.jsonl"));
        std::fs::write(&sub_path, "{}").expect("write subagent transcript");
        let main_str = main.to_string_lossy().into_owned();

        register_known_session(
            &state,
            parent_uuid,
            AgentKind::Cursor,
            "/tmp/project",
            Some(&main_str),
        );
        register_subagent_start(
            &state,
            &json!({
                "subagent_id": "sub-abc",
                "conversation_id": "conv-parent",
                "subagent_type": "explore",
                "transcript_path": main_str
            }),
            AgentKind::Cursor,
        );

        let parent = resolve_cursor_session_for_payload(
            &state,
            &json!({
                "conversation_id": sub_uuid,
                "hook_event_name": "preToolUse"
            }),
        );
        assert_eq!(parent.as_deref(), Some(parent_uuid));

        let subagents = state.active_subagents.lock().expect("lock");
        assert_eq!(
            subagents[0].agent_transcript_path.as_deref(),
            Some(sub_path.to_string_lossy().as_ref())
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn complete_subagent_falls_back_to_conversation_transcript_path() {
        let state = test_app_state();
        let parent_uuid = "conv-parent";
        let sub_uuid = "conv-subagent-new";
        let dir =
            std::env::temp_dir().join(format!("atoll-subagent-complete-{}", std::process::id()));
        let parent_dir = dir.join(parent_uuid);
        let subagents_dir = parent_dir.join("subagents");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
        let main = parent_dir.join(format!("{parent_uuid}.jsonl"));
        std::fs::write(&main, "{}").expect("write parent transcript");
        let sub_path = subagents_dir.join(format!("{sub_uuid}.jsonl"));
        std::fs::write(&sub_path, "{}").expect("write subagent transcript");
        let main_str = main.to_string_lossy().into_owned();

        register_known_session(
            &state,
            parent_uuid,
            AgentKind::Cursor,
            "/tmp/project",
            Some(&main_str),
        );
        register_subagent_start(
            &state,
            &json!({
                "subagent_id": "sub-abc",
                "conversation_id": "conv-parent",
                "subagent_type": "explore",
                "transcript_path": main_str
            }),
            AgentKind::Cursor,
        );
        let _ = resolve_cursor_session_for_payload(
            &state,
            &json!({
                "conversation_id": sub_uuid,
                "hook_event_name": "preToolUse"
            }),
        );

        complete_subagent(
            &state,
            &json!({
                "hook_event_name": "subagentStop",
                "conversation_id": "conv-parent",
                "subagent_type": "explore",
                "summary": "Done"
            }),
        );

        let subagents = state.active_subagents.lock().expect("lock");
        assert!(subagents[0].completed_at.is_some());
        assert_eq!(
            subagents[0].agent_transcript_path.as_deref(),
            Some(sub_path.to_string_lossy().as_ref())
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parent_conversation_id_is_not_treated_as_subagent_session() {
        let state = test_app_state();
        register_subagent_start(
            &state,
            &json!({
                "subagent_id": "sub-abc",
                "conversation_id": "conv-parent",
                "subagent_type": "explore"
            }),
            AgentKind::Cursor,
        );

        let parent = resolve_cursor_session_for_payload(
            &state,
            &json!({
                "conversation_id": "conv-parent",
                "hook_event_name": "preToolUse"
            }),
        );
        assert!(parent.is_none());
    }

    #[test]
    fn cursor_subagent_pretooluse_request_attributes_to_parent() {
        let state = test_app_state();
        register_subagent_start(
            &state,
            &json!({
                "subagent_id": "sub-abc",
                "conversation_id": "conv-parent",
                "subagent_type": "explore"
            }),
            AgentKind::Cursor,
        );

        let payload = json!({
            "hook_event_name": "preToolUse",
            "conversation_id": "conv-subagent",
            "cwd": "/tmp/project",
            "tool_name": "Shell",
            "tool_input": { "command": "echo hi" },
            "tool_use_id": "tool-1"
        });
        let mut request = hook_bridge::permission_request_from_cursor_payload(
            "req-1".into(),
            payload.clone(),
            "2026-01-01T00:00:00Z".into(),
        )
        .expect("cursor request");
        assert_eq!(request.session, "conv-subagent");

        hook_bridge::attribute_cursor_request_to_parent_session(&state, &payload, &mut request);
        assert_eq!(request.session, "conv-parent");
    }

    #[test]
    fn bind_cursor_subagent_conversation_rewrites_ghost_requests() {
        let state = test_app_state();
        register_known_session(
            &state,
            "conv-subagent",
            AgentKind::Cursor,
            "/tmp/project",
            Some("/tmp/project/sub.jsonl"),
        );
        {
            let mut requests = state.requests.lock().expect("lock");
            requests.push(PermissionRequest {
                id: "req-ghost".into(),
                tool_use_id: None,
                agent: AgentKind::Cursor,
                session: "conv-subagent".into(),
                command: "Bash: echo".into(),
                detail: "echo".into(),
                cwd: "/tmp/project".into(),
                requested_at: "2026-06-10T08:00:00Z".into(),
                status: PermissionStatus::Approved,
                archived: false,
                supports_always: false,
                transcript_path: None,
                tool_input: None,
            });
        }

        bind_cursor_subagent_conversation(&state, "conv-subagent", "conv-parent");

        let requests = state.requests.lock().expect("lock");
        assert_eq!(requests[0].session, "conv-parent");
        assert!(!state
            .known_sessions
            .lock()
            .expect("lock")
            .contains_key("conv-subagent"));
        assert_eq!(
            state
                .cursor_subagent_conversations
                .lock()
                .expect("lock")
                .get("conv-subagent")
                .map(String::as_str),
            Some("conv-parent")
        );
    }

    #[test]
    fn snapshot_excludes_bound_subagent_conversation_ids() {
        let requests = vec![PermissionRequest {
            id: "req-parent".into(),
            tool_use_id: None,
            agent: AgentKind::Cursor,
            session: "conv-parent".into(),
            command: "Bash: ls".into(),
            detail: "ls".into(),
            cwd: "/tmp/project".into(),
            requested_at: "2026-06-10T08:00:00Z".into(),
            status: PermissionStatus::Approved,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        }];
        let mut known_sessions = HashMap::new();
        known_sessions.insert(
            "conv-subagent".into(),
            KnownSession {
                agent: AgentKind::Cursor,
                cwd: "/tmp/project".into(),
                transcript_path: Some("/tmp/sub.jsonl".into()),
                last_activity: "2026-06-10T08:01:00Z".into(),
                host: platform::SessionHost::CursorIde,
                conversation_id: Some("conv-subagent".into()),
            },
        );
        let mut excluded = HashSet::new();
        excluded.insert("conv-subagent".into());

        let snapshot = snapshot_from(
            &requests,
            &HashMap::new(),
            900,
            &HashMap::new(),
            &known_sessions,
            &HashSet::new(),
            true,
            &excluded,
        );

        let session_ids: Vec<&str> = snapshot
            .sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect();
        assert_eq!(session_ids, vec!["conv-parent"]);
        assert!(!session_ids.contains(&"conv-subagent"));
    }

    #[test]
    fn sanitize_subagent_id_strips_newlines_for_transcript_path() {
        let agent_id = "call_abc\nfc_def";
        let sanitized = sanitize_subagent_id_for_filename(agent_id);
        assert!(!sanitized.contains('\n'));
        assert!(sanitized.contains("call_abc"));
    }

    fn test_session_summary(session_id: &str) -> SessionSummary {
        SessionSummary {
            session_id: session_id.to_string(),
            agent: AgentKind::Cursor,
            cwd: "/tmp/project".into(),
            pending_count: 0,
            total_count: 0,
            last_activity: "2026-06-10T08:10:00Z".into(),
            transcript_path: None,
            pinned: false,
            session_host: platform::SessionHost::Unknown,
            active_subagents: Vec::new(),
        }
    }

    fn test_active_subagent(
        agent_id: &str,
        session_id: &str,
        completed_at: Option<String>,
        archived: bool,
    ) -> ActiveSubagent {
        ActiveSubagent {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            agent_kind: AgentKind::Cursor,
            agent_type: agent_id.to_string(),
            started_at: "2026-06-10T08:00:00Z".into(),
            agent_transcript_path: None,
            completed_at,
            archived,
            last_message: None,
            conversation_id: None,
        }
    }

    #[test]
    fn reconcile_incomplete_subagents_refreshes_path_and_terminal_message() {
        let state = test_app_state();
        let parent_uuid = "conv-parent-reconcile";
        let agent_id = "sub-reconcile";
        let dir =
            std::env::temp_dir().join(format!("atoll-subagent-reconcile-{}", std::process::id()));
        let parent_dir = dir.join(parent_uuid);
        let subagents_dir = parent_dir.join("subagents");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
        let main = parent_dir.join(format!("{parent_uuid}.jsonl"));
        std::fs::write(&main, "{}").expect("write parent transcript");
        let sub_path = subagents_dir.join(format!("agent-{agent_id}.jsonl"));
        let terminal_entry = json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "text",
                    "text": "Request interrupted by user for tool use"
                }]
            }
        });
        std::fs::write(&sub_path, format!("{terminal_entry}\n")).expect("write sub transcript");
        let main_str = main.to_string_lossy().into_owned();

        register_known_session(
            &state,
            parent_uuid,
            AgentKind::Cursor,
            "/tmp/project",
            Some(&main_str),
        );
        register_subagent_start(
            &state,
            &json!({
                "subagent_id": agent_id,
                "conversation_id": parent_uuid,
                "subagent_type": "explore"
            }),
            AgentKind::Cursor,
        );

        reconcile_incomplete_subagents(&state);

        let subagents = state.active_subagents.lock().expect("lock");
        assert_eq!(subagents.len(), 1);
        assert_eq!(
            subagents[0].agent_transcript_path.as_deref(),
            Some(sub_path.to_string_lossy().as_ref())
        );
        assert!(subagents[0].completed_at.is_some());
        assert_eq!(
            subagents[0].last_message.as_deref(),
            Some("Request interrupted by user for tool use")
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn archive_completed_subagents_keeps_running_sibling_visible() {
        let state = test_app_state();
        let now = parse_iso_timestamp_secs("2026-06-10T08:10:00Z");
        let completed_at = Some(format_unix_timestamp(now - 30));
        let mut completed = test_active_subagent("done", "session-a", completed_at, false);
        completed.conversation_id = Some("conv-done".into());
        let running = test_active_subagent("running", "session-a", None, false);
        {
            let mut subagents = state.active_subagents.lock().expect("lock");
            subagents.push(completed);
            subagents.push(running);
        }
        state
            .cursor_subagent_conversations
            .lock()
            .expect("lock")
            .insert("conv-done".into(), "session-a".into());

        let conv_ids = archive_completed_subagents_in_state(&state, "session-a");
        assert_eq!(conv_ids, vec!["conv-done".to_string()]);
        for conv_id in conv_ids {
            unbind_cursor_subagent_conversation(&state, Some(&conv_id));
        }

        assert!(!state
            .cursor_subagent_conversations
            .lock()
            .expect("lock")
            .contains_key("conv-done"));

        let active_subagents = state.active_subagents.lock().expect("lock").clone();
        assert!(active_subagents
            .iter()
            .any(|sub| sub.agent_id == "done" && sub.archived));
        assert!(active_subagents
            .iter()
            .any(|sub| sub.agent_id == "running" && !sub.archived));

        let mut sessions = vec![test_session_summary("session-a")];
        assign_active_subagents_to_sessions(&mut sessions, &active_subagents, 60, now);
        let visible_ids: Vec<&str> = sessions[0]
            .active_subagents
            .iter()
            .map(|sub| sub.agent_id.as_str())
            .collect();
        assert_eq!(visible_ids, vec!["running"]);
    }

    #[test]
    fn snapshot_subagent_assignment_groups_by_session_without_changing_filters_or_order() {
        let now = parse_iso_timestamp_secs("2026-06-10T08:10:00Z");
        let recent_completed = Some(format_unix_timestamp(now - 30));
        let old_completed = Some(format_unix_timestamp(now - 120));
        let active_subagents = vec![
            test_active_subagent("a-running", "session-a", None, false),
            test_active_subagent("b-running", "session-b", None, false),
            test_active_subagent("a-old", "session-a", old_completed.clone(), false),
            test_active_subagent("a-archived", "session-a", None, true),
            test_active_subagent("a-recent", "session-a", recent_completed, false),
            test_active_subagent("orphan", "missing-session", None, false),
        ];
        let mut sessions = vec![
            test_session_summary("session-a"),
            test_session_summary("session-b"),
        ];

        assign_active_subagents_to_sessions(&mut sessions, &active_subagents, 60, now);

        let session_a_ids: Vec<&str> = sessions[0]
            .active_subagents
            .iter()
            .map(|sub| sub.agent_id.as_str())
            .collect();
        let session_b_ids: Vec<&str> = sessions[1]
            .active_subagents
            .iter()
            .map(|sub| sub.agent_id.as_str())
            .collect();
        assert_eq!(session_a_ids, vec!["a-running", "a-recent"]);
        assert_eq!(session_b_ids, vec!["b-running"]);

        assign_active_subagents_to_sessions(&mut sessions, &active_subagents, 0, now);
        let session_a_ids: Vec<&str> = sessions[0]
            .active_subagents
            .iter()
            .map(|sub| sub.agent_id.as_str())
            .collect();
        assert_eq!(session_a_ids, vec!["a-running", "a-old", "a-recent"]);
    }
}

#[cfg(test)]
mod hook_bridge_tests {
    use serde_json::json;

    #[test]
    fn maps_claude_pre_tool_use_payload_to_permission_request() {
        let payload = json!({
            "session_id": "session-123",
            "cwd": "/tmp/project",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": "npm install",
                "description": "Install dependencies"
            },
            "tool_use_id": "tool-123"
        });

        let request = crate::hook_bridge::permission_request_from_claude_payload(
            "request-123".into(),
            payload,
            "2026-06-09T09:00:00Z".into(),
        )
        .expect("payload should map to a request");

        assert_eq!(request.id, "request-123");
        assert!(matches!(request.agent, crate::AgentKind::Claude));
        assert_eq!(request.session, "session-123");
        assert_eq!(request.command, "Bash: npm install");
        assert_eq!(request.detail, "Install dependencies");
        assert_eq!(request.cwd, "/tmp/project");
        assert_eq!(request.tool_use_id.as_deref(), Some("tool-123"));
        assert_eq!(request.status, crate::PermissionStatus::Pending);
    }

    #[test]
    fn maps_claude_permission_request_payload_to_permission_request() {
        let payload = json!({
            "session_id": "session-123",
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {
                "command": "npm install",
                "description": "Install dependencies"
            },
            "tool_use_id": "tool-123"
        });

        let request = crate::hook_bridge::permission_request_from_claude_payload(
            "request-123".into(),
            payload,
            "2026-06-09T09:00:00Z".into(),
        )
        .expect("payload should map to a request");

        assert_eq!(request.command, "Bash: npm install");
        assert_eq!(request.detail, "Install dependencies");
        assert_eq!(request.tool_use_id.as_deref(), Some("tool-123"));
        assert!(!request.supports_always);
    }

    #[test]
    fn supports_always_from_permission_suggestions() {
        let payload = json!({
            "session_id": "session-123",
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {
                "command": "npm install",
                "description": "Install dependencies"
            },
            "permission_suggestions": [
                {
                    "type": "addRules",
                    "rules": [{"toolName": "Bash", "ruleContent": "npm install"}],
                    "behavior": "allow",
                    "destination": "localSettings"
                }
            ]
        });

        let request = crate::hook_bridge::permission_request_from_claude_payload(
            "request-456".into(),
            payload,
            "2026-06-09T09:00:00Z".into(),
        )
        .expect("payload should map to a request");

        assert!(request.supports_always);
    }

    #[test]
    fn marks_pending_request_complete_from_claude_post_tool_use() {
        let mut requests = vec![crate::PermissionRequest {
            id: "request-123".into(),
            tool_use_id: Some("tool-123".into()),
            agent: crate::AgentKind::Claude,
            session: "session-123".into(),
            command: "Bash: npm install".into(),
            detail: "Install dependencies".into(),
            cwd: "/tmp/project".into(),
            requested_at: "2026-06-09T09:00:00Z".into(),
            status: crate::PermissionStatus::Pending,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        }];

        let payload = json!({
            "session_id": "session-123",
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": "npm install"
            },
            "tool_use_id": "tool-123"
        });

        let completed_id = crate::hook_bridge::mark_matching_pending_request_complete(
            &mut requests,
            &payload,
            "Completed in Claude.",
        );

        assert_eq!(completed_id.as_deref(), Some("request-123"));
        assert_eq!(requests[0].status, crate::PermissionStatus::Approved);
        assert!(requests[0].detail.contains("Completed in Claude."));
    }

    #[test]
    fn marks_only_pending_request_complete_when_post_tool_use_has_no_match_fields() {
        let mut requests = vec![crate::PermissionRequest {
            id: "request-123".into(),
            tool_use_id: None,
            agent: crate::AgentKind::Claude,
            session: "session-123".into(),
            command: "Bash: curl -s https://httpbin.org/ip".into(),
            detail: "Curl to external API to trigger permission request".into(),
            cwd: "/tmp/project".into(),
            requested_at: "2026-06-09T09:00:00Z".into(),
            status: crate::PermissionStatus::Pending,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        }];

        let payload = json!({
            "session_id": "session-123",
            "hook_event_name": "PostToolUse"
        });

        let completed_id = crate::hook_bridge::mark_matching_pending_request_complete(
            &mut requests,
            &payload,
            "Completed in Claude.",
        );

        assert_eq!(completed_id.as_deref(), Some("request-123"));
        assert_eq!(requests[0].status, crate::PermissionStatus::Approved);
    }

    #[test]
    fn falls_back_to_newest_session_pending_when_post_tool_use_has_no_match_fields() {
        // Requests are stored newest-first (see `requests.insert(0, …)`), so a
        // PostToolUse that carries no match fields falls back to completing the
        // session's newest pending request and leaves the older ones pending.
        let mut requests = vec![
            crate::PermissionRequest {
                id: "request-newer".into(),
                tool_use_id: None,
                agent: crate::AgentKind::Claude,
                session: "session-123".into(),
                command: "Bash: echo two".into(),
                detail: "two".into(),
                cwd: "/tmp/project".into(),
                requested_at: "2026-06-09T09:00:01Z".into(),
                status: crate::PermissionStatus::Pending,
                archived: false,
                supports_always: false,
                transcript_path: None,
                tool_input: None,
            },
            crate::PermissionRequest {
                id: "request-older".into(),
                tool_use_id: None,
                agent: crate::AgentKind::Claude,
                session: "session-123".into(),
                command: "Bash: echo one".into(),
                detail: "one".into(),
                cwd: "/tmp/project".into(),
                requested_at: "2026-06-09T09:00:00Z".into(),
                status: crate::PermissionStatus::Pending,
                archived: false,
                supports_always: false,
                transcript_path: None,
                tool_input: None,
            },
        ];

        let payload = json!({
            "session_id": "session-123",
            "hook_event_name": "PostToolUse"
        });

        let completed_id = crate::hook_bridge::mark_matching_pending_request_complete(
            &mut requests,
            &payload,
            "Completed in Claude.",
        );

        assert_eq!(completed_id.as_deref(), Some("request-newer"));
        assert_eq!(requests[0].status, crate::PermissionStatus::Approved);
        assert_eq!(requests[1].status, crate::PermissionStatus::Pending);
    }

    #[test]
    fn encodes_hook_decision_for_claude_hook_event() {
        let approved = crate::hook_bridge::permission_hook_response(
            "PermissionRequest",
            crate::Decision::Approved,
            "",
            None,
        );
        assert_eq!(
            approved,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow"
                    }
                }
            })
        );

        let denied = crate::hook_bridge::permission_hook_response(
            "PermissionRequest",
            crate::Decision::Denied,
            "",
            None,
        );
        assert_eq!(
            denied,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "deny",
                        "message": "Denied from Atoll"
                    }
                }
            })
        );
    }

    #[test]
    fn encodes_hook_decision_with_note_for_claude_hook_event() {
        let denied = crate::hook_bridge::permission_hook_response(
            "PermissionRequest",
            crate::Decision::Denied,
            "Please use a safer command",
            None,
        );
        assert_eq!(
            denied,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "deny",
                        "message": "Denied from Atoll: Please use a safer command"
                    }
                }
            })
        );
    }

    #[test]
    fn encodes_hook_decision_for_claude_pre_tool_use() {
        let approved = crate::hook_bridge::permission_hook_response(
            "PreToolUse",
            crate::Decision::Approved,
            "",
            None,
        );
        assert_eq!(
            approved,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "permissionDecisionReason": "Approved from Atoll"
                }
            })
        );
    }

    #[test]
    fn maps_codex_permission_request_payload_to_permission_request() {
        let payload = json!({
            "session_id": "codex-session-1",
            "cwd": "/Users/test/project",
            "hook_event_name": "PermissionRequest",
            "tool_name": "exec_command",
            "tool_input": {
                "command": "npm test",
                "description": "Run tests"
            },
            "tool_use_id": "tool-codex-1"
        });

        let request = crate::hook_bridge::permission_request_from_codex_payload(
            "request-codex-1".into(),
            payload,
            "2026-06-19T09:00:00Z".into(),
        )
        .expect("payload should map to a request");

        assert_eq!(request.id, "request-codex-1");
        assert!(matches!(request.agent, crate::AgentKind::Codex));
        assert_eq!(request.session, "codex-session-1");
        assert_eq!(request.command, "Bash: npm test");
        assert_eq!(request.detail, "Run tests");
        assert_eq!(request.cwd, "/Users/test/project");
        assert_eq!(request.tool_use_id.as_deref(), Some("tool-codex-1"));
        assert!(!request.supports_always);
    }

    #[test]
    fn encodes_codex_permission_allow_and_deny_responses() {
        let approved = crate::hook_bridge::permission_hook_response(
            "PermissionRequest",
            crate::Decision::Approved,
            "",
            None,
        );
        assert_eq!(
            approved,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow"
                    }
                }
            })
        );

        let denied = crate::hook_bridge::permission_hook_response(
            "PermissionRequest",
            crate::Decision::Denied,
            "too risky",
            None,
        );
        assert_eq!(
            denied,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "deny",
                        "message": "Denied from Atoll: too risky"
                    }
                }
            })
        );
    }

    #[test]
    fn codex_internal_permission_request_is_ignored() {
        let payload = json!({
            "session_id": "internal-thread",
            "hook_event_name": "PermissionRequest",
            "tool_name": "exec_command",
            "tool_input": {
                "command": "echo hi",
                "description": "internal"
            }
        });

        let request = crate::hook_bridge::permission_request_from_codex_payload(
            "request-internal".into(),
            payload,
            "2026-06-19T09:00:00Z".into(),
        );

        assert!(request.is_none());
    }

    #[test]
    fn encodes_permission_request_ask_as_empty_response() {
        let ask = crate::hook_bridge::hook_defer_response("PermissionRequest", "Atoll unavailable");

        assert_eq!(ask, json!({}));
    }

    #[test]
    fn encodes_pre_tool_use_ask_response() {
        let ask = crate::hook_bridge::hook_defer_response("PreToolUse", "Atoll unavailable");

        assert_eq!(
            ask,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "ask",
                    "permissionDecisionReason": "Atoll unavailable"
                }
            })
        );
    }
}

#[cfg(test)]
mod hook_script_path_tests {
    use super::{
        configured_atoll_hook_node_path, configured_atoll_hook_script_path,
        extract_node_script_path, should_flag_dev_hook_drift,
    };
    use serde_json::json;
    use std::fs;

    #[test]
    fn extract_node_script_path_handles_quoted_and_unquoted_commands() {
        assert_eq!(
            extract_node_script_path(
                "node \"/Applications/Atoll.app/Contents/Resources/scripts/atoll-codex-hook.mjs\""
            ),
            Some("/Applications/Atoll.app/Contents/Resources/scripts/atoll-codex-hook.mjs".into())
        );
        assert_eq!(
            extract_node_script_path(
                "node /Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs"
            ),
            Some("/Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs".into())
        );
        assert_eq!(
            extract_node_script_path(
                "\"/opt/homebrew/bin/node\" \"/Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs\""
            ),
            Some("/Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs".into())
        );
    }

    #[test]
    fn extract_node_script_path_handles_cmd_c_runner_commands() {
        assert_eq!(
            extract_node_script_path(
                "cmd /c \"C:/Atoll/scripts/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/Atoll/scripts/atoll-cursor-hook.mjs\""
            ),
            Some("C:/Atoll/scripts/atoll-cursor-hook.mjs".into())
        );
    }

    #[test]
    fn configured_atoll_hook_script_path_reads_hooks_json() {
        let config = json!({
            "hooks": {
                "PermissionRequest": [{
                    "matcher": "*",
                    "hooks": [{
                        "command": "node \"/Applications/Atoll.app/Contents/Resources/scripts/atoll-codex-hook.mjs\""
                    }]
                }]
            }
        });

        assert_eq!(
            configured_atoll_hook_script_path(&config, "atoll-codex-hook"),
            Some("/Applications/Atoll.app/Contents/Resources/scripts/atoll-codex-hook.mjs".into())
        );
    }

    #[test]
    fn configured_atoll_hook_script_path_reads_cursor_flat_hooks_json() {
        let config = json!({
            "version": 1,
            "hooks": {
                "preToolUse": [{
                    "command": "\"C:/runner/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/tmp/atoll-cursor-hook.mjs\"",
                    "timeout": 1800
                }]
            }
        });

        assert_eq!(
            configured_atoll_hook_script_path(&config, "atoll-cursor-hook"),
            Some("C:/tmp/atoll-cursor-hook.mjs".into())
        );
    }

    #[test]
    fn configured_atoll_hook_node_path_reads_cursor_flat_hooks_json() {
        let config = json!({
            "hooks": {
                "sessionStart": [{
                    "command": "\"C:/runner/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/tmp/atoll-cursor-hook.mjs\""
                }]
            }
        });

        assert_eq!(
            configured_atoll_hook_node_path(&config, "atoll-cursor-hook"),
            Some("C:/Program Files/nodejs/node.exe".into())
        );
    }

    #[test]
    fn should_flag_dev_hook_drift_when_configured_dev_path_missing() {
        let preferred = std::env::current_exe()
            .expect("current exe")
            .to_string_lossy()
            .into_owned();
        assert!(should_flag_dev_hook_drift(
            "C:/Users/test/Atoll/target/debug/scripts/atoll-codex-hook.mjs",
            &preferred,
        ));
    }

    #[test]
    fn should_not_flag_dev_hook_drift_when_configured_dev_path_exists() {
        let temp_root = std::env::temp_dir().join("atoll-drift-test-target-debug");
        let script_dir = temp_root.join("target").join("debug");
        fs::create_dir_all(&script_dir).expect("create temp script dir");
        let script_path = script_dir.join("atoll-codex-hook.mjs");
        fs::write(&script_path, "export {}").expect("write temp script");
        let configured = script_path.to_string_lossy().into_owned();
        let preferred = std::env::current_exe()
            .expect("current exe")
            .to_string_lossy()
            .into_owned();
        assert!(!should_flag_dev_hook_drift(&configured, &preferred));
        let _ = fs::remove_dir_all(temp_root);
    }
}

#[cfg(test)]
mod codex_hooks_tests {
    use super::{
        extract_node_script_path, format_hook_command, has_atoll_codex_hooks,
        normalize_hook_script_path, remove_atoll_codex_hooks, resolve_node_executable,
        upsert_codex_hook_events,
    };
    use serde_json::json;

    fn sample_atoll_codex_hooks() -> serde_json::Value {
        json!({
            "PermissionRequest": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": "node \"/tmp/atoll-codex-hook.mjs\"",
                    "timeout": 1800,
                    "statusMessage": "Atoll approval"
                }]
            }],
            "PostToolUse": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": "node \"/tmp/atoll-codex-hook.mjs\"",
                    "timeout": 30
                }]
            }],
            "Stop": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": "node \"/tmp/atoll-codex-hook.mjs\"",
                    "timeout": 30
                }]
            }],
            "SubagentStop": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": "node \"/tmp/atoll-codex-hook.mjs\"",
                    "timeout": 30
                }]
            }]
        })
    }

    #[test]
    fn has_atoll_codex_hooks_recognizes_powershell_launcher_command() {
        let config = json!({
            "hooks": {
                "PermissionRequest": [{
                    "matcher": "*",
                    "hooks": [{
                        "command": "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-codex-hook.ps1\""
                    }]
                }],
                "PostToolUse": [{
                    "matcher": "*",
                    "hooks": [{
                        "command": "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-codex-hook.ps1\""
                    }]
                }],
                "Stop": [{
                    "matcher": "*",
                    "hooks": [{
                        "command": "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-codex-hook.ps1\""
                    }]
                }],
                "SubagentStop": [{
                    "matcher": "*",
                    "hooks": [{
                        "command": "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-codex-hook.ps1\""
                    }]
                }]
            }
        });

        assert!(has_atoll_codex_hooks(&config));
    }

    #[test]
    fn upsert_installs_into_empty_codex_hook_arrays() {
        let mut hooks = json!({
            "PermissionRequest": [],
            "PostToolUse": [],
            "Stop": [],
            "SubagentStop": []
        });
        let atoll = sample_atoll_codex_hooks();

        upsert_codex_hook_events(&mut hooks, &atoll);

        let config = json!({ "hooks": hooks });
        assert!(has_atoll_codex_hooks(&config));
    }

    #[test]
    fn uninstall_removes_atoll_codex_hooks_and_empty_events() {
        let mut hooks = sample_atoll_codex_hooks();
        remove_atoll_codex_hooks(&mut hooks);

        assert!(hooks.as_object().unwrap().is_empty());
    }

    #[test]
    fn format_hook_command_quotes_paths_with_spaces_codex() {
        let command = format_hook_command(
            None,
            "/opt/homebrew/bin/node",
            "/Applications/Atoll.app/scripts/atoll-codex-hook.mjs",
        );
        assert_eq!(
            command,
            "\"/opt/homebrew/bin/node\" \"/Applications/Atoll.app/scripts/atoll-codex-hook.mjs\""
        );

        let windows_command = format_hook_command(
            None,
            r"C:\Program Files\nodejs\node.exe",
            r"C:\Program Files\Atoll\resources\scripts\atoll-claude-hook.mjs",
        );
        #[cfg(windows)]
        assert_eq!(
            windows_command,
            "\"C:/Program Files/nodejs/node.exe\" \"C:/Program Files/Atoll/resources/scripts/atoll-claude-hook.mjs\""
        );
        #[cfg(not(windows))]
        assert_eq!(
            windows_command,
            "\"C:\\Program Files\\nodejs\\node.exe\" \"C:\\Program Files\\Atoll\\resources\\scripts\\atoll-claude-hook.mjs\""
        );

        let runner_command = format_hook_command(
            Some(r"C:\Program Files\Atoll\resources\scripts\atoll-hook-runner.exe"),
            r"C:\Program Files\nodejs\node.exe",
            r"C:\Program Files\Atoll\resources\scripts\atoll-claude-hook.mjs",
        );
        #[cfg(windows)]
        assert_eq!(
            runner_command,
            "\"C:/Program Files/Atoll/resources/scripts/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/Program Files/Atoll/resources/scripts/atoll-claude-hook.mjs\""
        );
        #[cfg(not(windows))]
        assert_eq!(
            runner_command,
            "\"C:\\Program Files\\nodejs\\node.exe\" \"C:\\Program Files\\Atoll\\resources\\scripts\\atoll-claude-hook.mjs\""
        );

        let unc_command = format_hook_command(
            None,
            r"C:\Program Files\nodejs\node.exe",
            r"\\?\C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs",
        );
        #[cfg(windows)]
        assert_eq!(
            unc_command,
            "\"C:/Program Files/nodejs/node.exe\" \"C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs\""
        );
        #[cfg(not(windows))]
        assert_eq!(
            unc_command,
            "\"C:\\Program Files\\nodejs\\node.exe\" \"C:\\Program Files\\Atoll\\scripts\\atoll-claude-hook.mjs\""
        );
    }

    #[test]
    fn extract_node_script_path_strips_windows_unc_prefix() {
        assert_eq!(
            extract_node_script_path(
                r#""C:\Program Files\nodejs\node.exe" "\\?\C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs""#
            ),
            Some(r"C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs".into())
        );
        assert_eq!(
            extract_node_script_path(
                r#"node "\\?\C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs""#
            ),
            Some(r"C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs".into())
        );
        assert_eq!(
            extract_node_script_path(
                r#""C:/Program Files/nodejs/node.exe" "C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs""#
            ),
            Some(r"C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs".into())
        );
        assert_eq!(
            extract_node_script_path(
                r#""C:/Program Files/Atoll/resources/scripts/atoll-hook-runner.exe" "C:/Program Files/nodejs/node.exe" "C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs""#
            ),
            Some(r"C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs".into())
        );
    }

    #[test]
    fn resolve_node_executable_finds_standard_windows_install() {
        #[cfg(windows)]
        {
            let standard = r"C:\Program Files\nodejs\node.exe";
            if std::path::Path::new(standard).exists() {
                let resolved = resolve_node_executable().expect("node should resolve");
                assert_eq!(resolved, normalize_hook_script_path(standard));
            }
        }
    }

    #[test]
    fn resolve_node_executable_from_where_returns_existing_path() {
        #[cfg(windows)]
        {
            use super::resolve_node_executable_from_where;

            if std::path::Path::new(r"C:\Program Files\nodejs\node.exe").exists() {
                let resolved =
                    resolve_node_executable_from_where().expect("where should find node");
                assert!(std::path::Path::new(&resolved).exists());
            }
        }
    }
}

#[cfg(test)]
mod zcode_hooks_tests {
    use super::{
        configured_atoll_hook_command, has_atoll_zcode_hooks, remove_atoll_zcode_hooks,
        upsert_zcode_hook_events,
    };
    use serde_json::{json, Value};

    fn sample_atoll_zcode_hooks() -> Value {
        json!({
            "PermissionRequest": [
                { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs", "timeout": 1800 }] }
            ],
            "PostToolUse": [
                { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs", "timeout": 30 }] }
            ],
            "Stop": [
                { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs", "timeout": 30 }] }
            ]
        })
    }

    #[test]
    fn has_atoll_zcode_hooks_requires_enabled_flag() {
        let hooks = sample_atoll_zcode_hooks();
        let enabled_config = json!({ "hooks": { "enabled": true, "events": hooks } });
        assert!(has_atoll_zcode_hooks(&enabled_config));

        let disabled_config = json!({ "hooks": { "enabled": false, "events": hooks } });
        assert!(!has_atoll_zcode_hooks(&disabled_config));

        let missing_flag_config = json!({ "hooks": { "events": hooks } });
        assert!(!has_atoll_zcode_hooks(&missing_flag_config));
    }

    #[test]
    fn upsert_zcode_hook_events_is_idempotent_and_keeps_foreign_hooks() {
        let mut events = json!({
            "PermissionRequest": [
                { "hooks": [{ "type": "command", "command": "node /other/island-hook.mjs" }] }
            ]
        });
        let atoll = sample_atoll_zcode_hooks();

        upsert_zcode_hook_events(&mut events, &atoll);
        upsert_zcode_hook_events(&mut events, &atoll);

        let permission_matchers = events
            .get("PermissionRequest")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(permission_matchers.len(), 2);

        let config = json!({
            "hooks": { "enabled": true, "events": events }
        });
        assert!(has_atoll_zcode_hooks(&config));
    }

    #[test]
    fn remove_atoll_zcode_hooks_keeps_foreign_hooks() {
        let mut events = json!({
            "PermissionRequest": [
                { "hooks": [{ "type": "command", "command": "node /other/island-hook.mjs" }] },
                { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs" }] }
            ],
            "Stop": [
                { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs" }] }
            ]
        });

        remove_atoll_zcode_hooks(&mut events);

        let events_obj = events.as_object().unwrap();
        assert_eq!(events_obj.len(), 1);
        let remaining = events_obj
            .get("PermissionRequest")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("island-hook.mjs"));
    }

    #[test]
    fn configured_atoll_hook_command_reads_zcode_events_nesting() {
        let config = json!({
            "hooks": {
                "enabled": true,
                "timeoutMs": 60000,
                "events": {
                    "PermissionRequest": [
                        { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs" }] }
                    ]
                }
            }
        });

        let command = configured_atoll_hook_command(&config, "atoll-zcode-hook");
        assert_eq!(
            command.as_deref(),
            Some("node /opt/Atoll/hooks/atoll-zcode-hook.mjs")
        );
    }
}

#[cfg(test)]
mod gemini_hooks_tests {
    use super::{has_atoll_gemini_hooks, remove_atoll_gemini_hooks, upsert_gemini_hook_entries};
    use serde_json::{json, Value};

    fn sample_atoll_gemini_hooks() -> Value {
        json!({
            "BeforeTool": [
                {
                    "matcher": "run_shell_command|write_file|replace|web_fetch|save_memory|invoke_agent|mcp_",
                    "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs", "timeout": 1800000 }]
                }
            ],
            "SessionStart": [
                { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs", "timeout": 30000 }] }
            ],
            "AfterTool": [
                { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs", "timeout": 30000 }] }
            ]
        })
    }

    #[test]
    fn has_atoll_gemini_hooks_reads_flat_hooks_object() {
        let config = json!({ "hooks": sample_atoll_gemini_hooks() });
        assert!(has_atoll_gemini_hooks(&config));

        // Gemini keeps optional config keys (enabled/disabled/notifications)
        // alongside the event entries; they must not break detection.
        let with_extra_keys = json!({
            "hooks": {
                "notifications": true,
                "SessionStart": [{ "hooks": [{ "type": "command", "command": "other" }] }]
            }
        });
        assert!(!has_atoll_gemini_hooks(&with_extra_keys));
    }

    #[test]
    fn has_atoll_gemini_hooks_requires_all_core_events() {
        let mut hooks = sample_atoll_gemini_hooks();
        hooks
            .as_object_mut()
            .unwrap()
            .remove("AfterTool")
            .expect("AfterTool entry");
        let config = json!({ "hooks": hooks });
        assert!(!has_atoll_gemini_hooks(&config));
    }

    #[test]
    fn upsert_gemini_hook_entries_is_idempotent_and_keeps_foreign_hooks() {
        let mut hooks = json!({
            "BeforeTool": [
                {
                    "matcher": "run_shell_command",
                    "hooks": [{ "type": "command", "command": "node /other/island-hook.mjs" }]
                }
            ],
            "notifications": true
        });
        let atoll = sample_atoll_gemini_hooks();

        upsert_gemini_hook_entries(&mut hooks, &atoll);
        upsert_gemini_hook_entries(&mut hooks, &atoll);

        let hooks_obj = hooks.as_object().unwrap();
        // notifications is not an event array and must be preserved untouched.
        assert_eq!(hooks_obj.get("notifications"), Some(&json!(true)));

        let before_tool = hooks_obj
            .get("BeforeTool")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(before_tool.len(), 2);
        assert!(before_tool[0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("island-hook.mjs"));
        assert!(before_tool[1]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("atoll-gemini-hook"));

        assert!(has_atoll_gemini_hooks(&json!({ "hooks": hooks })));
    }

    #[test]
    fn remove_atoll_gemini_hooks_keeps_foreign_hooks_and_config_keys() {
        let mut hooks = json!({
            "notifications": true,
            "BeforeTool": [
                {
                    "matcher": "run_shell_command",
                    "hooks": [{ "type": "command", "command": "node /other/island-hook.mjs" }]
                },
                {
                    "matcher": "run_shell_command",
                    "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs" }]
                }
            ],
            "SessionStart": [
                { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs" }] }
            ]
        });

        remove_atoll_gemini_hooks(&mut hooks);

        let hooks_obj = hooks.as_object().unwrap();
        assert_eq!(hooks_obj.get("notifications"), Some(&json!(true)));
        assert_eq!(hooks_obj.len(), 2);
        let before_tool = hooks_obj
            .get("BeforeTool")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(before_tool.len(), 1);
        assert!(before_tool[0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("island-hook.mjs"));
    }
}

#[cfg(test)]
mod zcode_token_tests {
    use super::{
        refresh_session_token_usage, update_zcode_subagents, zcode_rollout_path, AgentKind,
        TOKEN_HISTORY_ENV_LOCK,
    };
    use crate::core_tests::test_app_state;
    use serde_json::{json, Value};

    const PARENT_SESSION: &str = "sess_11111111-2222-4333-8444-555555555555";
    const CHILD_SESSION: &str = "sess_subagent_agent_66666666-7777-4888-9999-000000000000";

    /// Fixture timestamps must be "now": the token parser drops rollout lines
    /// whose local day differs from the current one, so a fixed date would go
    /// stale as soon as the calendar moves past it.
    fn today_iso() -> String {
        crate::iso_timestamp_now()
    }

    fn zcode_line(session: &str, iso: &str, model: &str, usage: Value) -> String {
        json!({
            "type": "model_io",
            "sessionId": session,
            "completedAt": iso,
            "model": { "modelId": model },
            "response": { "usage": usage }
        })
        .to_string()
    }

    fn today_key() -> String {
        crate::local_time::local_day_key_from_iso(&today_iso()).expect("local day key")
    }

    /// Redirect HOME into a temp dir so `zcode_rollout_path` lands in fixtures.
    /// Serialized by TOKEN_HISTORY_ENV_LOCK like the other env-mutating tests.
    pub(crate) struct HomeGuard {
        previous: Option<std::ffi::OsString>,
        pub(crate) home: std::path::PathBuf,
    }

    impl HomeGuard {
        pub(crate) fn new(tag: &str) -> Self {
            let home = std::env::temp_dir().join(format!(
                "atoll-zcode-token-{}-{}",
                tag,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&home);
            std::fs::create_dir_all(&home).expect("create temp home");
            let previous = std::env::var_os("HOME");
            std::env::set_var("HOME", &home);
            let history_path = home.join("token-history.json");
            std::env::set_var(
                "ATOLL_TOKEN_HISTORY_PATH",
                history_path.to_string_lossy().as_ref(),
            );
            Self { previous, home }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(previous) => std::env::set_var("HOME", previous),
                None => std::env::remove_var("HOME"),
            }
            std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
            let _ = std::fs::remove_dir_all(&self.home);
        }
    }

    fn write_parent_rollout(home: &std::path::Path) -> std::path::PathBuf {
        let rollout = home
            .join(".zcode")
            .join("cli")
            .join("rollout")
            .join(format!("model-io-{PARENT_SESSION}.jsonl"));
        std::fs::create_dir_all(rollout.parent().unwrap()).expect("create rollout dir");
        std::fs::write(
            &rollout,
            format!(
                "{}\n{}\n",
                zcode_line(
                    PARENT_SESSION,
                    &today_iso(),
                    "GLM-5.3",
                    json!({ "inputTokens": 38758, "outputTokens": 147, "cacheReadTokens": 38400, "cacheWriteTokens": 12 })
                ),
                zcode_line(
                    PARENT_SESSION,
                    &today_iso(),
                    "GLM-4.7",
                    json!({ "inputTokens": 232, "outputTokens": 77, "cacheReadTokens": 0, "cacheWriteTokens": 0 })
                ),
            ),
        )
        .expect("write parent rollout");
        rollout
    }

    fn write_subagent_metadata(home: &std::path::Path, status: &str, completed_at: Option<&str>) {
        let mut metadata = json!({
            "parentSessionId": PARENT_SESSION,
            "childSessionId": CHILD_SESSION,
            "profileSnapshot": { "name": "Explore" },
            "prompt": "Search the codebase for usages",
            "status": status,
            "createdAt": today_iso(),
        });
        if let Some(completed_at) = completed_at {
            metadata["completedAt"] = json!(completed_at);
        }
        let metadata_path = home
            .join(".zcode")
            .join("cli")
            .join("agents")
            .join(PARENT_SESSION)
            .join("agent_abc123")
            .join("metadata.json");
        std::fs::create_dir_all(metadata_path.parent().unwrap()).expect("create agents dir");
        std::fs::write(&metadata_path, metadata.to_string()).expect("write metadata");
    }

    fn write_child_rollout(home: &std::path::Path) {
        let rollout = home
            .join(".zcode")
            .join("cli")
            .join("rollout")
            .join(format!("model-io-{CHILD_SESSION}.jsonl"));
        std::fs::create_dir_all(rollout.parent().unwrap()).expect("create rollout dir");
        std::fs::write(
            &rollout,
            format!(
                "{}\n",
                zcode_line(
                    CHILD_SESSION,
                    &today_iso(),
                    "GLM-5.3",
                    json!({ "inputTokens": 34456, "outputTokens": 2992, "cacheReadTokens": 32320, "cacheWriteTokens": 0 })
                ),
            ),
        )
        .expect("write child rollout");
    }

    #[test]
    fn zcode_rollout_path_rejects_unsafe_session_ids() {
        let valid = zcode_rollout_path("sess_6ea9e07c-3ff6-4ca8-9e02-8b24a06b401b")
            .expect("valid session id");
        assert_eq!(
            valid.file_name().unwrap().to_string_lossy(),
            "model-io-sess_6ea9e07c-3ff6-4ca8-9e02-8b24a06b401b.jsonl"
        );
        assert!(
            zcode_rollout_path("sess_subagent_agent_6ea9e07c-3ff6-4ca8-9e02-8b24a06b401b")
                .is_some()
        );
        for bad in [
            "../escape",
            "sess_../../escape",
            "/absolute/path",
            "sess_ with space",
            "claude",
            "",
        ] {
            assert!(
                zcode_rollout_path(bad).is_none(),
                "expected rejection: {bad}"
            );
        }
    }

    #[test]
    fn zcode_refresh_parses_rollout_from_session_id() {
        let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
        let home = HomeGuard::new("refresh");

        let rollout = write_parent_rollout(&home.home);
        let state = test_app_state();

        // The hook payload's transcript path is an ephemeral temp file; the
        // rollout path derived from the session id must be used instead.
        refresh_session_token_usage(
            &state,
            PARENT_SESSION,
            Some("/tmp/atoll-ephemeral-hook-transcript.jsonl"),
            Some(&AgentKind::Zcode),
        )
        .expect("refresh");

        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(PARENT_SESSION)
            .copied()
            .expect("usage");
        assert_eq!(usage.input_tokens, 358 + 232);
        assert_eq!(usage.output_tokens, 147 + 77);
        assert_eq!(usage.cache_read_tokens, 38400);
        assert_eq!(usage.cache_creation_tokens, 12);

        let by_model = state
            .session_token_usage_by_model
            .lock()
            .expect("lock")
            .get(PARENT_SESSION)
            .expect("by-model usage")
            .clone();
        assert_eq!(by_model.get("GLM-5.3").unwrap().input_tokens, 358);
        assert_eq!(by_model.get("GLM-4.7").unwrap().output_tokens, 77);

        let offsets = state.token_usage_file_offsets.lock().expect("lock");
        let stored = offsets
            .get(rollout.to_string_lossy().as_ref())
            .copied()
            .expect("rollout offset");
        assert_eq!(stored, std::fs::metadata(&rollout).unwrap().len());
        assert!(state
            .session_agent_map
            .lock()
            .expect("lock")
            .get(PARENT_SESSION)
            .map(|agent| agent == "zcode")
            .unwrap_or(false));
    }

    #[test]
    fn zcode_refresh_tolerates_missing_rollout() {
        let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new("missing-rollout");
        let state = test_app_state();

        refresh_session_token_usage(&state, PARENT_SESSION, None, Some(&AgentKind::Zcode))
            .expect("refresh without rollout file");

        // A zero entry may be registered, but nothing was counted.
        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(PARENT_SESSION)
            .copied()
            .unwrap_or_default();
        assert!(usage.is_zero());
    }

    #[test]
    fn zcode_subagent_usage_and_chips_follow_metadata() {
        let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
        let home = HomeGuard::new("subagent");
        let state = test_app_state();

        write_child_rollout(&home.home);
        write_subagent_metadata(&home.home, "running", None);

        let changed = update_zcode_subagents(&state, PARENT_SESSION, &today_key());
        assert!(changed, "running subagent should register a chip");

        {
            let usage = state
                .session_token_usage
                .lock()
                .expect("lock")
                .get(PARENT_SESSION)
                .copied()
                .expect("parent usage");
            assert_eq!(usage.input_tokens, 34456 - 32320);
            assert_eq!(usage.output_tokens, 2992);
            assert_eq!(usage.cache_read_tokens, 32320);
        }
        {
            let subagents = state.active_subagents.lock().expect("lock");
            assert_eq!(subagents.len(), 1);
            let chip = &subagents[0];
            assert_eq!(chip.agent_id, CHILD_SESSION);
            assert_eq!(chip.session_id, PARENT_SESSION);
            assert_eq!(chip.agent_type, "Explore");
            assert!(matches!(chip.agent_kind, AgentKind::Zcode));
            assert!(chip.completed_at.is_none());
            assert_eq!(
                chip.last_message.as_deref(),
                Some("Search the codebase for usages")
            );
        }

        // Subagent completes: chip closes, tokens must not be counted twice.
        write_subagent_metadata(&home.home, "completed", Some(&today_iso()));
        let changed = update_zcode_subagents(&state, PARENT_SESSION, &today_key());
        assert!(changed, "completion should close the chip");
        {
            let subagents = state.active_subagents.lock().expect("lock");
            assert_eq!(subagents.len(), 1);
            assert!(subagents[0].completed_at.is_some());
        }
        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(PARENT_SESSION)
            .copied()
            .expect("parent usage");
        assert_eq!(usage.input_tokens, 34456 - 32320);

        // Third pass: nothing new, nothing changed.
        let changed = update_zcode_subagents(&state, PARENT_SESSION, &today_key());
        assert!(!changed);
    }

    #[test]
    fn zcode_historical_subagents_do_not_become_chips() {
        let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
        let home = HomeGuard::new("historical");
        let state = test_app_state();

        write_child_rollout(&home.home);
        write_subagent_metadata(&home.home, "completed", Some(&today_iso()));

        let changed = update_zcode_subagents(&state, PARENT_SESSION, &today_key());
        assert!(!changed, "finished-before-seen subagents stay invisible");
        assert!(state.active_subagents.lock().expect("lock").is_empty());
        // But their token usage still lands on the parent session.
        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(PARENT_SESSION)
            .copied()
            .expect("parent usage");
        assert_eq!(usage.output_tokens, 2992);
    }
}

#[cfg(test)]
mod claude_hooks_tests {
    use super::{
        detect_competing_claude_hooks, format_hook_command, has_atoll_claude_hooks,
        hook_command_binary_exists, remove_atoll_claude_hooks,
        remove_dead_competing_hooks_from_config, upsert_claude_hook_events,
    };
    use serde_json::json;

    fn sample_atoll_claude_hooks() -> serde_json::Value {
        json!({
            "PermissionRequest": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs"),
                    "timeout": 1800
                }]
            }],
            "PostToolUse": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs"),
                    "timeout": 30
                }]
            }],
            "Stop": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs"),
                    "timeout": 30
                }]
            }]
        })
    }

    #[test]
    fn upsert_preserves_user_notification_hooks() {
        let mut hooks = json!({
            "Notification": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": "osascript -e 'display notification \"hi\"'"
                }]
            }],
            "PermissionRequest": [],
            "PostToolUse": [],
            "Stop": []
        });
        let atoll = sample_atoll_claude_hooks();

        upsert_claude_hook_events(&mut hooks, &atoll);

        let config = json!({ "hooks": hooks });
        assert!(has_atoll_claude_hooks(&config));
        let notification = hooks
            .get("Notification")
            .and_then(|value| value.as_array())
            .and_then(|arr| arr.first())
            .and_then(|matcher| matcher.get("hooks"))
            .and_then(|value| value.as_array())
            .and_then(|arr| arr.first())
            .and_then(|hook| hook.get("command"))
            .and_then(|value| value.as_str());
        assert!(notification.unwrap_or("").contains("display notification"));
    }

    #[test]
    fn uninstall_removes_only_atoll_claude_hooks() {
        let mut hooks = json!({
            "Notification": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": "osascript -e 'display notification \"hi\"'"
                }]
            }],
            "PermissionRequest": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs")
                }]
            }],
            "PostToolUse": [],
            "Stop": []
        });

        remove_atoll_claude_hooks(&mut hooks);

        assert!(hooks.get("Notification").is_some());
        let permission = hooks
            .get("PermissionRequest")
            .and_then(|value| value.as_array());
        assert!(permission.map(|arr| arr.is_empty()).unwrap_or(true));
    }

    #[test]
    fn detect_finds_non_atoll_hooks_and_flags_missing_binaries() {
        // A config with: Atoll (live), a dead competitor (binary missing), and a
        // real shell command (binary present on Unix test runners).
        let config = json!({
            "hooks": {
                "PermissionRequest": [
                    { "matcher": "*", "hooks": [
                        { "type": "command", "command": "/nonexistent/ping-island-bridge --source claude" },
                        { "type": "command", "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs") }
                    ]}
                ],
                "Notification": [
                    { "matcher": "*", "hooks": [
                        { "type": "command", "command": "/bin/echo hello" }
                    ]}
                ]
            }
        });

        let competing = detect_competing_claude_hooks(&config);
        assert_eq!(competing.len(), 2);
        let ping = competing
            .iter()
            .find(|c| c.command.contains("ping-island-bridge"))
            .unwrap();
        assert!(
            !ping.binary_exists,
            "ping-island binary should be flagged missing"
        );
        assert_eq!(ping.event, "PermissionRequest");
        let echo_hook = competing
            .iter()
            .find(|c| c.command.contains("/bin/echo"))
            .unwrap();
        assert!(
            echo_hook.binary_exists,
            "/bin/echo binary should be present"
        );
        assert_eq!(echo_hook.event, "Notification");
    }

    #[test]
    fn detect_ignores_atoll_hooks_and_empty_config() {
        let config = json!({
            "hooks": {
                "PermissionRequest": [
                    { "matcher": "*", "hooks": [
                        { "type": "command", "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs") }
                    ]}
                ]
            }
        });
        assert!(detect_competing_claude_hooks(&config).is_empty());

        assert!(detect_competing_claude_hooks(&json!({})).is_empty());
        assert!(detect_competing_claude_hooks(&json!({ "hooks": {} })).is_empty());
    }

    #[test]
    fn remove_strips_only_dead_competitors_preserves_atoll_and_live() {
        // The echo binary exists on the test runner, so it survives cleanup.
        // The nonexistent competitor binary does not, so it is removed. Atoll's hook is kept.
        let mut settings = json!({
            "hooks": {
                "PermissionRequest": [
                    { "matcher": "*", "hooks": [
                        { "type": "command", "command": "/nonexistent/ping-island-bridge --source claude" },
                        { "type": "command", "command": "/bin/echo hello" },
                        { "type": "command", "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs") }
                    ]}
                ]
            }
        });

        let removed = remove_dead_competing_hooks_from_config(&mut settings);
        assert!(removed);

        let hooks = settings.get("hooks").unwrap();
        let pr = hooks
            .get("PermissionRequest")
            .and_then(|v| v.as_array())
            .unwrap();
        let commands: Vec<&str> = pr[0]
            .get("hooks")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .map(|h| h.get("command").and_then(|c| c.as_str()).unwrap_or(""))
            .collect();
        assert!(
            commands.iter().any(|c| c.contains("atoll-claude-hook")),
            "Atoll hook must survive"
        );
        assert!(
            commands.iter().any(|c| c.contains("/bin/echo")),
            "live competitor must survive"
        );
        assert!(
            !commands.iter().any(|c| c.contains("ping-island-bridge")),
            "dead competitor must be removed"
        );
    }

    #[test]
    fn remove_drops_empty_event_after_last_dead_hook_removed() {
        // An event with ONLY a dead competitor should be removed entirely.
        let mut settings = json!({
            "hooks": {
                "PermissionDenied": [
                    { "matcher": "*", "hooks": [
                        { "type": "command", "command": "/nonexistent/ping-island-bridge --source claude" }
                    ]}
                ]
            }
        });

        let removed = remove_dead_competing_hooks_from_config(&mut settings);
        assert!(removed);
        assert!(
            settings
                .get("hooks")
                .map(|h| h.as_object().map(|o| o.is_empty()).unwrap_or(true))
                .unwrap_or(true),
            "hooks object should be empty or absent after removing the only dead hook"
        );
    }

    #[test]
    fn hook_command_binary_exists_handles_quotes_and_empty() {
        assert!(!hook_command_binary_exists(""));
        assert!(!hook_command_binary_exists("   "));
        // /bin/sh exists on all Unix test runners.
        assert!(hook_command_binary_exists("/bin/sh --flag"));
        assert!(hook_command_binary_exists("'/bin/sh' --flag"));
        assert!(hook_command_binary_exists("\"/bin/sh\" --flag"));
        assert!(!hook_command_binary_exists("/nonexistent/binary --flag"));
    }
}

#[cfg(test)]
mod cursor_hooks_tests {
    use super::{
        cursor_hook_command_needs_repair, cursor_hooks_need_command_repair,
        cursor_hooks_need_lifecycle_upgrade, cursor_hooks_need_timeout_repair,
        format_cursor_hook_command, has_atoll_cursor_hooks, hook_entry_has_atoll_cursor,
        remove_atoll_cursor_hooks, repair_cursor_hook_events_with_command,
        upsert_cursor_hook_events, CURSOR_HOOK_EVENTS, CURSOR_HOOK_TIMEOUT_SECONDS,
    };
    use serde_json::json;

    #[test]
    fn format_cursor_hook_command_uses_cmd_c_on_windows() {
        let command = format_cursor_hook_command(
            Some(r"C:\Atoll\scripts\atoll-hook-runner.exe"),
            r"C:\Program Files\nodejs\node.exe",
            r"C:\Atoll\scripts\atoll-cursor-hook.mjs",
        );
        #[cfg(windows)]
        assert!(
            command.starts_with("cmd /c "),
            "expected cmd /c prefix, got: {command}"
        );
        #[cfg(not(windows))]
        assert!(!command.starts_with("cmd /c "));
    }

    #[test]
    fn upsert_and_detect_cursor_hooks() {
        let mut hooks = json!({});
        upsert_cursor_hook_events(
            &mut hooks,
            &format_cursor_hook_command(
                Some("/tmp/atoll-hook-runner.exe"),
                "/opt/homebrew/bin/node",
                "/tmp/atoll-cursor-hook.mjs",
            ),
            "http://127.0.0.1:47777/cursor/hook",
        );

        let config = json!({ "version": 1, "hooks": hooks });
        assert!(has_atoll_cursor_hooks(&config));
        assert!(!cursor_hooks_need_lifecycle_upgrade(&config));
        assert!(hook_entry_has_atoll_cursor(
            &config["hooks"]["sessionStart"].as_array().unwrap()[0]
        ));
        assert!(hook_entry_has_atoll_cursor(
            &config["hooks"]["afterAgentResponse"].as_array().unwrap()[0]
        ));
        assert!(hook_entry_has_atoll_cursor(
            &config["hooks"]["beforeSubmitPrompt"].as_array().unwrap()[0]
        ));
        assert!(hook_entry_has_atoll_cursor(
            &config["hooks"]["afterAgentThought"].as_array().unwrap()[0]
        ));
        assert!(hook_entry_has_atoll_cursor(
            &config["hooks"]["preToolUse"].as_array().unwrap()[0]
        ));
        assert_eq!(
            config["hooks"]["preToolUse"].as_array().unwrap()[0]["env"]["ATOLL_HOOK_URL"],
            "http://127.0.0.1:47777/cursor/hook"
        );
        for (event, _) in CURSOR_HOOK_EVENTS {
            assert_eq!(
                config["hooks"][event].as_array().unwrap()[0]["timeout"],
                json!(CURSOR_HOOK_TIMEOUT_SECONDS),
                "event {event}"
            );
        }
    }

    #[test]
    fn remove_cursor_hooks_preserves_other_entries() {
        let mut hooks = json!({
            "preToolUse": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 1800 },
                { "command": "./custom-hook.sh", "timeout": 10 }
            ],
            "postToolUse": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "stop": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "subagentStop": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ]
        });

        remove_atoll_cursor_hooks(&mut hooks);

        let pre_tool_use = hooks["preToolUse"].as_array().unwrap();
        assert_eq!(pre_tool_use.len(), 1);
        assert_eq!(pre_tool_use[0]["command"], "./custom-hook.sh");
        assert!(!has_atoll_cursor_hooks(&json!({ "hooks": hooks })));
    }

    /// v0.1.31 installs only the five core events. After upgrading to v0.1.32,
    /// those installs must still count as "installed" so the online indicator
    /// and Cursor session display keep working until the user reinstalls.
    #[test]
    fn v0_1_31_core_only_cursor_hooks_count_as_installed() {
        let hooks = json!({
            "preToolUse": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 1800 }
            ],
            "postToolUse": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "stop": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "subagentStart": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "subagentStop": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ]
        });

        let config = json!({ "version": 1, "hooks": hooks });
        assert!(has_atoll_cursor_hooks(&config));
        assert!(cursor_hooks_need_lifecycle_upgrade(&config));
    }

    #[test]
    fn cursor_hook_repair_replaces_legacy_atoll_commands_and_preserves_custom_entries() {
        let preferred =
            "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-cursor-hook.ps1\"";
        let config = json!({
            "version": 1,
            "hooks": {
                "preToolUse": [
                    {
                        "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                        "timeout": 1800
                    },
                    {
                        "command": "./user-cursor-hook.sh",
                        "timeout": 5
                    }
                ],
                "postToolUse": [
                    {
                        "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                        "timeout": 30
                    }
                ],
                "stop": [
                    {
                        "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                        "timeout": 30
                    }
                ],
                "subagentStart": [
                    {
                        "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                        "timeout": 30
                    }
                ],
                "subagentStop": [
                    {
                        "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                        "timeout": 30
                    }
                ]
            }
        });

        let repaired = repair_cursor_hook_events_with_command(
            &config,
            preferred,
            "http://127.0.0.1:47777/cursor/hook",
        )
        .expect("repaired hooks");

        for (event, timeout) in CURSOR_HOOK_EVENTS {
            let entries = repaired["hooks"][event].as_array().expect("event entries");
            let atoll_entries: Vec<_> = entries
                .iter()
                .filter(|entry| hook_entry_has_atoll_cursor(entry))
                .collect();
            assert_eq!(atoll_entries.len(), 1, "event {event}");
            assert_eq!(atoll_entries[0]["command"], preferred);
            assert_eq!(atoll_entries[0]["timeout"], json!(timeout));
            assert_eq!(
                atoll_entries[0]["env"]["ATOLL_HOOK_URL"],
                "http://127.0.0.1:47777/cursor/hook"
            );
        }

        let pre_tool_entries = repaired["hooks"]["preToolUse"].as_array().unwrap();
        assert!(pre_tool_entries
            .iter()
            .any(|entry| entry["command"] == "./user-cursor-hook.sh"));
    }

    #[test]
    fn cursor_hook_command_repair_detects_windows_legacy_command() {
        let legacy =
            "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"";
        assert!(cursor_hook_command_needs_repair(
            legacy,
            Some("C:/Users/test/AppData/Local/Atoll/hooks/atoll-cursor-hook.mjs"),
            true,
        ));

        let config = json!({
            "version": 1,
            "hooks": {
                "preToolUse": [{ "command": legacy, "timeout": 1800 }],
                "postToolUse": [{ "command": legacy, "timeout": 30 }],
                "stop": [{ "command": legacy, "timeout": 30 }],
                "subagentStart": [{ "command": legacy, "timeout": 30 }],
                "subagentStop": [{ "command": legacy, "timeout": 30 }]
            }
        });
        assert!(cursor_hooks_need_command_repair(
            &config,
            Some("C:/Users/test/AppData/Local/Atoll/hooks/atoll-cursor-hook.mjs"),
            true,
        ));
    }

    #[test]
    fn cursor_hook_timeout_repair_detects_legacy_timeouts() {
        let config = json!({
            "version": 1,
            "hooks": {
                "preToolUse": [
                    { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 1800 },
                    { "command": "./user-cursor-hook.sh", "timeout": 1800 }
                ],
                "postToolUse": [
                    { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
                ],
                "stop": [
                    { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
                ],
                "subagentStart": [
                    { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
                ],
                "subagentStop": [
                    { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
                ]
            }
        });

        assert!(cursor_hooks_need_timeout_repair(&config));

        let repaired = repair_cursor_hook_events_with_command(
            &config,
            "node \"/tmp/atoll-cursor-hook.mjs\"",
            "http://127.0.0.1:47777/cursor/hook",
        )
        .expect("repaired hooks");

        for (event, _) in CURSOR_HOOK_EVENTS {
            let entries = repaired["hooks"][event].as_array().expect("event entries");
            let atoll_entries: Vec<_> = entries
                .iter()
                .filter(|entry| hook_entry_has_atoll_cursor(entry))
                .collect();
            assert_eq!(atoll_entries.len(), 1, "event {event}");
            assert_eq!(
                atoll_entries[0]["timeout"],
                json!(CURSOR_HOOK_TIMEOUT_SECONDS),
                "event {event}"
            );
        }

        let pre_tool_entries = repaired["hooks"]["preToolUse"].as_array().unwrap();
        assert!(pre_tool_entries
            .iter()
            .any(|entry| entry["command"] == "./user-cursor-hook.sh"));
    }

    /// Missing any one of the five core events means hooks are incomplete.
    #[test]
    fn missing_core_cursor_hook_event_is_not_installed() {
        let hooks = json!({
            "preToolUse": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 1800 }
            ],
            "postToolUse": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "stop": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "subagentStop": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ]
        });

        assert!(!has_atoll_cursor_hooks(
            &json!({ "version": 1, "hooks": hooks })
        ));
    }
}

#[cfg(test)]
mod zcode_chat_tests {
    use super::{
        parse_zcode_db_session_path, read_zcode_chat_messages, truncate_transcript_content,
        zcode_db_session_path, TOKEN_HISTORY_ENV_LOCK, TRANSCRIPT_MAX_MESSAGES,
        TRANSCRIPT_MESSAGE_MAX_CHARS,
    };
    use crate::zcode_token_tests::HomeGuard;
    use serde_json::{json, Value};
    use std::path::Path;

    const SESSION: &str = "sess_11111111-2222-4333-8444-555555555555";

    fn write_zcode_db(home: &Path, session_id: &str, messages: &[Value]) {
        let db_path = home.join(".zcode").join("cli").join("db").join("db.sqlite");
        std::fs::create_dir_all(db_path.parent().unwrap()).expect("create db dir");
        let connection = rusqlite::Connection::open(&db_path).expect("open fixture db");
        connection
            .execute_batch(
                "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, sequence INTEGER, data TEXT);
                 CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, sequence INTEGER, data TEXT);",
            )
            .expect("create fixture tables");
        for (index, message) in messages.iter().enumerate() {
            let message_id = message["id"].as_str().expect("message id");
            let parts = message["parts"].as_array().cloned().unwrap_or_default();
            connection
                .execute(
                    "INSERT INTO message (id, session_id, sequence, data) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![
                        message_id,
                        session_id,
                        index as i64,
                        json!({ "role": message["role"], "id": message_id }).to_string()
                    ],
                )
                .expect("insert message");
            for (part_index, part) in parts.iter().enumerate() {
                connection
                    .execute(
                        "INSERT INTO part (id, message_id, session_id, sequence, data) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            format!("{message_id}-part-{part_index}"),
                            message_id,
                            session_id,
                            part_index as i64,
                            part.to_string()
                        ],
                    )
                    .expect("insert part");
            }
        }
    }

    #[test]
    fn zcode_db_session_path_round_trips_and_rejects_unsafe_ids() {
        let path = zcode_db_session_path("sess_abc-123").expect("valid");
        assert_eq!(path, "zcode-db://sess_abc-123");
        assert_eq!(parse_zcode_db_session_path(&path), Some("sess_abc-123"));
        assert_eq!(parse_zcode_db_session_path("file:///tmp/x.jsonl"), None);
        assert_eq!(parse_zcode_db_session_path("zcode-db://../escape"), None);
        assert_eq!(parse_zcode_db_session_path("zcode-db://"), None);
        assert!(zcode_db_session_path("/abs/path").is_none());
    }

    #[test]
    fn reads_zcode_chat_from_sqlite() {
        let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
        let home = HomeGuard::new("chat");

        write_zcode_db(
            &home.home,
            SESSION,
            &[
                json!({
                    "id": "msg_u1", "role": "user",
                    "parts": [
                        { "type": "text", "text": "TodoWrite reminder noise", "synthetic": true },
                        { "type": "text", "text": "帮我看看这个文件" }
                    ]
                }),
                json!({
                    "id": "msg_a1", "role": "assistant",
                    "parts": [
                        { "type": "step-start" },
                        { "type": "reasoning", "text": "internal reasoning trace" },
                        { "type": "text", "text": "我先查一下" },
                        { "type": "tool", "tool": "Bash", "state": { "status": "completed", "input": { "command": "ls -la" }, "output": "total 0" } },
                        { "type": "tool", "tool": "AskUserQuestion", "state": { "status": "completed", "input": { "questions": [{ "question": "先做哪个?", "options": [] }] }, "output": "User has answered your questions: \"先做哪个?\"=\"Hook bridge, Plan mode UI\"" } },
                        { "type": "step-finish" }
                    ]
                }),
                json!({ "id": "msg_u2", "role": "user", "parts": [{ "type": "text", "text": "   " }] }),
            ],
        );

        let messages = read_zcode_chat_messages(SESSION).expect("read chat");
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "帮我看看这个文件");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "我先查一下");
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[2].content, "");
        assert_eq!(messages[2].tool_name.as_deref(), Some("Bash"));
        assert_eq!(messages[2].tool_input, Some(json!({ "command": "ls -la" })));
        // Non-question tool outputs stay out of the transcript.
        assert_eq!(messages[2].tool_output, None);
        assert_eq!(messages[3].tool_name.as_deref(), Some("AskUserQuestion"));
        assert_eq!(
            messages[3].tool_output.as_deref(),
            Some("User has answered your questions: \"先做哪个?\"=\"Hook bridge, Plan mode UI\"")
        );

        // Other sessions stay isolated; invalid ids are rejected before I/O.
        assert!(
            read_zcode_chat_messages("sess_ffffffff-ffff-4fff-8fff-ffffffffffff")
                .expect("unknown session reads empty")
                .is_empty()
        );
        assert!(read_zcode_chat_messages("../escape").is_err());
    }

    #[test]
    fn zcode_chat_keeps_only_the_newest_messages() {
        let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
        let home = HomeGuard::new("chat-limit");

        let total = TRANSCRIPT_MAX_MESSAGES + 7;
        let messages: Vec<Value> = (0..total)
            .map(|i| {
                json!({
                    "id": format!("msg_{i:04}"), "role": "user",
                    "parts": [{ "type": "text", "text": format!("message {i}") }]
                })
            })
            .collect();
        write_zcode_db(&home.home, SESSION, &messages);

        let read = read_zcode_chat_messages(SESSION).expect("read chat");
        assert_eq!(read.len(), TRANSCRIPT_MAX_MESSAGES);
        assert_eq!(
            read[0].content,
            format!("message {}", total - TRANSCRIPT_MAX_MESSAGES)
        );
        assert_eq!(
            read[read.len() - 1].content,
            format!("message {}", total - 1)
        );
    }

    #[test]
    fn zcode_chat_errors_without_database() {
        let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new("chat-missing-db");
        let error = read_zcode_chat_messages(SESSION).expect_err("no db fixture");
        assert!(error.contains("ZCode database"), "{error}");
    }

    #[test]
    fn transcript_truncation_marker_is_applied() {
        let long = "字".repeat(TRANSCRIPT_MESSAGE_MAX_CHARS + 1);
        let truncated = truncate_transcript_content(long.clone());
        assert_eq!(
            truncated.chars().take(100).collect::<String>(),
            long.chars().take(100).collect::<String>()
        );
        assert!(truncated.contains("[message truncated by Atoll]"));
        let short = "short message";
        assert_eq!(truncate_transcript_content(short.to_string()), short);
    }

    #[test]
    fn snapshot_exposes_zcode_db_path_for_chat() {
        use super::{
            iso_timestamp_now, snapshot_from, KnownSession, PermissionRequest, PermissionStatus,
        };
        use super::{platform, AgentKind};
        use std::collections::{HashMap, HashSet};

        let ephemeral = "/tmp/atoll-ephemeral-hook-transcript.jsonl";
        let virtual_path = "zcode-db://sess_11111111-2222-4333-8444-555555555555";
        let known_sessions = HashMap::from([(
            SESSION.to_string(),
            KnownSession {
                agent: AgentKind::Zcode,
                cwd: "/tmp/project".into(),
                transcript_path: Some(ephemeral.into()),
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::ZcodeCli,
                conversation_id: None,
            },
        )]);
        let make_request = |archived: bool| PermissionRequest {
            id: "req-zcode-1".into(),
            tool_use_id: None,
            agent: AgentKind::Zcode,
            session: SESSION.into(),
            command: "Bash: ls".into(),
            detail: "List files".into(),
            cwd: "/tmp/project".into(),
            requested_at: iso_timestamp_now(),
            status: PermissionStatus::Approved,
            archived,
            supports_always: false,
            transcript_path: Some(ephemeral.into()),
            tool_input: None,
        };

        // Live request source.
        let snapshot = snapshot_from(
            &[make_request(false)],
            &HashMap::new(),
            900,
            &HashMap::new(),
            &known_sessions,
            &HashSet::new(),
            true,
            &HashSet::new(),
        );
        let session = snapshot
            .sessions
            .iter()
            .find(|s| s.session_id == SESSION)
            .expect("zcode session in snapshot");
        assert_eq!(session.transcript_path.as_deref(), Some(virtual_path));

        // Archived-retention source: every request resolved, session rebuilt
        // from history carrying the raw ephemeral hook path.
        let snapshot = snapshot_from(
            &[make_request(true)],
            &HashMap::new(),
            900,
            &HashMap::new(),
            &known_sessions,
            &HashSet::new(),
            true,
            &HashSet::new(),
        );
        let session = snapshot
            .sessions
            .iter()
            .find(|s| s.session_id == SESSION)
            .expect("retained zcode session");
        assert_eq!(session.transcript_path.as_deref(), Some(virtual_path));

        // Known-session-only source (observer events, no requests at all).
        let snapshot = snapshot_from(
            &[],
            &HashMap::new(),
            900,
            &HashMap::new(),
            &known_sessions,
            &HashSet::new(),
            true,
            &HashSet::new(),
        );
        let session = snapshot
            .sessions
            .iter()
            .find(|s| s.session_id == SESSION)
            .expect("known zcode session");
        assert_eq!(session.transcript_path.as_deref(), Some(virtual_path));
    }
}
