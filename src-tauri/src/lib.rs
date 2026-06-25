use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::utils::config::Color;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalSize, State};

mod capture;
mod hook_bridge;
mod local_time;
mod platform;
mod token_history;
mod transcript;

const COMPACT_WINDOW_WIDTH: f64 = 132.0;
pub(crate) const COMPACT_WINDOW_HEIGHT: f64 = 36.0;
/// Windows-only super-collapsed strip; macOS never selects this mode.
pub(crate) const MICRO_WINDOW_WIDTH: f64 = 96.0;
pub(crate) const MICRO_WINDOW_HEIGHT: f64 = 32.0;
const EXPANDED_WINDOW_WIDTH: f64 = 560.0;
pub(crate) const EXPANDED_WINDOW_HEIGHT: f64 = 320.0;
const EXPANDED_IDLE_WINDOW_HEIGHT: f64 = 240.0;
const MIN_COMPACT_WINDOW_WIDTH: f64 = 72.0;
// Dormant pill height (width spans the notch + side padding on notched displays).
const DORMANT_WINDOW_HEIGHT: f64 = 36.0;
// Extra width beyond the notch on each side so edges are visible.
const DORMANT_NOTCH_PADDING: f64 = 30.0;
const WINDOW_ANIMATION_DURATION: Duration = Duration::from_millis(420);
const WINDOW_ANIMATION_FRAME: Duration = Duration::from_micros(16_667);
// Fallback notch width (logical pt) used when the auxiliary menu-bar areas
// can't be read but a notch height is reported.
pub(crate) const FALLBACK_NOTCH_WIDTH: f64 = 200.0;
// Used when auxiliary menu-bar areas are unavailable but a housing is present.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) const FALLBACK_NOTCH_HEIGHT: f64 = 38.0;
// Extra logical points added above the reported safe-area inset so the
// collapsed capsule fully covers the physical camera housing.
const NOTCH_COVER_PADDING: f64 = 16.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionRequest {
    id: String,
    tool_use_id: Option<String>,
    agent: AgentKind,
    session: String,
    command: String,
    detail: String,
    cwd: String,
    requested_at: String,
    status: PermissionStatus,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    supports_always: bool,
    #[serde(default)]
    transcript_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_input: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IslandSnapshot {
    online: bool,
    pending_count: usize,
    archived_count: usize,
    active_request: Option<PermissionRequest>,
    recent: Vec<PermissionRequest>,
    sessions: Vec<SessionSummary>,
    daily_tokens: TokenUsage,
    active_session_tokens: TokenUsage,
    hook_health: HookHealthSnapshot,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveSubagent {
    agent_id: String,
    session_id: String,
    agent_kind: AgentKind,
    agent_type: String,
    started_at: String,
    agent_transcript_path: Option<String>,
    completed_at: Option<String>,
    #[serde(default)]
    archived: bool,
    last_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubagentSummary {
    agent_id: String,
    agent_type: String,
    started_at: String,
    agent_transcript_path: Option<String>,
    completed_at: Option<String>,
    #[serde(default)]
    archived: bool,
    last_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionSummary {
    session_id: String,
    agent: AgentKind,
    cwd: String,
    pending_count: usize,
    total_count: usize,
    last_activity: String,
    transcript_path: Option<String>,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    session_host: platform::SessionHost,
    #[serde(default)]
    active_subagents: Vec<SubagentSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenUsage {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
}

impl TokenUsage {
    fn add_assign(&mut self, other: TokenUsage) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(other.cache_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
    }

    fn component_wise_max(self, other: TokenUsage) -> TokenUsage {
        TokenUsage {
            input_tokens: self.input_tokens.max(other.input_tokens),
            output_tokens: self.output_tokens.max(other.output_tokens),
            cache_read_tokens: self.cache_read_tokens.max(other.cache_read_tokens),
            cache_creation_tokens: self.cache_creation_tokens.max(other.cache_creation_tokens),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AgentKind {
    Claude,
    Codex,
    Gemini,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum PermissionStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Decision {
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum IslandWindowMode {
    Micro,
    Dormant,
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IslandHoverChanged {
    hovering: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_y: Option<f64>,
}

struct DecisionWithNote {
    decision: Decision,
    note: String,
    updated_input: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnownSession {
    agent: AgentKind,
    cwd: String,
    transcript_path: Option<String>,
    last_activity: String,
    #[serde(default)]
    host: platform::SessionHost,
}

pub(crate) struct AppState {
    requests: Mutex<Vec<PermissionRequest>>,
    hook_waiters: Mutex<HashMap<String, SyncSender<DecisionWithNote>>>,
    auto_approve_sessions: Mutex<HashSet<String>>,
    compact_width: Mutex<f64>,
    compact_left_width: Mutex<f64>,
    presentation_generation: Arc<AtomicU64>,
    home_bounds: Mutex<Option<HomeWindowBounds>>,
    notch_metrics: Mutex<NotchMetrics>,
    session_last_seen: Mutex<HashMap<String, u64>>,
    session_retention_secs: Mutex<u64>,
    subagent_retention_secs: Mutex<u64>,
    session_token_usage: Mutex<HashMap<String, TokenUsage>>,
    /// Sticky session → agent mapping that survives session purges within a day.
    session_agent_map: Mutex<HashMap<String, String>>,
    token_usage_file_offsets: Mutex<HashMap<String, u64>>,
    token_usage_day: Mutex<String>,
    /// Floor for daily_tokens loaded from token_history.json on startup, so the
    /// counter never drops below what was persisted before the last restart.
    daily_tokens_baseline: Mutex<TokenUsage>,
    known_sessions: Mutex<HashMap<String, KnownSession>>,
    pinned_sessions: Mutex<HashSet<String>>,
    /// Platform-specific focus restore target (macOS pid / Windows HWND).
    previous_app_pid: Mutex<Option<i64>>,
    /// Last emitted listening-online flag; used to push snapshot updates when hook/bridge health changes.
    last_listening_online: Mutex<Option<bool>>,
    /// Last emitted hook-health snapshot; used to detect external config drift.
    last_hook_health: Mutex<Option<HookHealthSnapshot>>,
    /// Local hook bridge TCP port (0 until bound).
    bridge_port: AtomicU16,
    active_subagents: Mutex<Vec<ActiveSubagent>>,
    /// Rate-limiter for SubagentStart/SubagentStop snapshot emissions.
    last_subagent_snapshot_emit: Mutex<Instant>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HomeWindowBounds {
    position: LogicalPosition<f64>,
    compact_size: PhysicalSize<u32>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    monitor_top_y: f64,
    monitor_center_x: f64,
    notch: NotchMetrics,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    screen_geometry: Option<platform::ScreenGeometry>,
}

/// Camera-housing ("notch") geometry for the display the island lives on, in
/// logical points. On non-notched displays `has_notch` is false and the island
/// keeps its original top-edge layout.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NotchMetrics {
    has_notch: bool,
    width: f64,
    height: f64,
    #[serde(default)]
    left_area_width: f64,
    #[serde(default)]
    right_area_width: f64,
}

#[tauri::command]
fn get_snapshot(app: AppHandle, state: State<'_, AppState>) -> IslandSnapshot {
    roll_over_token_usage_if_needed(&state);
    let tracked_sessions = {
        let requests = state.requests.lock().expect("state mutex poisoned");
        let known_sessions = state.known_sessions.lock().expect("state mutex poisoned");
        collect_session_transcript_paths(&requests, &known_sessions)
    };
    for (session_id, transcript_path, agent) in tracked_sessions {
        let _ = refresh_session_token_usage(
            &state,
            &session_id,
            Some(transcript_path.as_str()),
            Some(&agent),
        );
    }

    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    snapshot
}

/// 任一 Agent hook 已安装且 shim 存在，并且本地 bridge 可连接 → 在线监听。
pub(crate) fn compute_listening_online(app: &AppHandle) -> bool {
    if capture::listening_online() {
        return true;
    }
    let claude_ready = claude_hook_status(app);
    let codex_ready = codex_hook_status(app);
    let any_installed = claude_ready.installed || codex_ready.installed;
    let any_script_found = claude_ready.script_found || codex_ready.script_found;
    any_installed && any_script_found && hook_bridge::is_bridge_reachable(app)
}

pub(crate) fn build_snapshot(app: &AppHandle, state: &AppState) -> IslandSnapshot {
    roll_over_token_usage_if_needed(state);
    reconcile_incomplete_subagents(state);
    let requests = state.requests.lock().expect("state mutex poisoned");
    let last_seen = state
        .session_last_seen
        .lock()
        .expect("state mutex poisoned");
    let retention = *state
        .session_retention_secs
        .lock()
        .expect("state mutex poisoned");
    let token_usage = state
        .session_token_usage
        .lock()
        .expect("state mutex poisoned");
    let known_sessions = state.known_sessions.lock().expect("state mutex poisoned");
    let pinned = state.pinned_sessions.lock().expect("state mutex poisoned");
    let online = compute_listening_online(app);
    let hook_health = build_hook_health(app);
    let mut snapshot = snapshot_from(
        &requests,
        &last_seen,
        retention,
        &token_usage,
        &known_sessions,
        &pinned,
        online,
    );
    drop(pinned);
    drop(known_sessions);
    drop(token_usage);
    drop(last_seen);
    drop(requests);
    let subagent_retention = *state
        .subagent_retention_secs
        .lock()
        .expect("state mutex poisoned");
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let active_subagents = state.active_subagents.lock().expect("state mutex poisoned");
    for session in snapshot.sessions.iter_mut() {
        session.active_subagents = active_subagents
            .iter()
            .filter(|subagent| {
                if subagent.session_id != session.session_id {
                    return false;
                }
                if subagent.archived {
                    return false;
                }
                if subagent_retention > 0 {
                    if let Some(ref completed) = subagent.completed_at {
                        let completed_ts = parse_iso_timestamp_secs(completed);
                        if now_secs.saturating_sub(completed_ts) >= subagent_retention {
                            return false;
                        }
                    }
                }
                true
            })
            .map(|subagent| SubagentSummary {
                agent_id: subagent.agent_id.clone(),
                agent_type: subagent.agent_type.clone(),
                started_at: subagent.started_at.clone(),
                agent_transcript_path: subagent.agent_transcript_path.clone(),
                completed_at: subagent.completed_at.clone(),
                archived: subagent.archived,
                last_message: subagent.last_message.clone(),
            })
            .collect();
    }
    drop(active_subagents);
    persist_session_hosts(state, &snapshot.sessions);
    snapshot.hook_health = hook_health;
    let _ = token_history::sync_today_to_history(state);

    // Ensure daily_tokens never drops below what was persisted before restart.
    let mut baseline = state
        .daily_tokens_baseline
        .lock()
        .expect("state mutex poisoned");
    snapshot.daily_tokens = snapshot.daily_tokens.component_wise_max(*baseline);
    *baseline = snapshot.daily_tokens;
    drop(baseline);

    snapshot
}

fn persist_session_hosts(state: &AppState, sessions: &[SessionSummary]) {
    for session in sessions {
        if matches!(
            session.session_host,
            platform::SessionHost::ClaudeDesktop
                | platform::SessionHost::ClaudeCli
                | platform::SessionHost::CodexDesktop
                | platform::SessionHost::CodexCli
        ) {
            store_session_host(state, &session.session_id, session.session_host);
        }
    }
}

fn sync_listening_online_snapshot(app: &AppHandle, state: &AppState) {
    let online = compute_listening_online(app);
    let should_emit = {
        let mut last = state
            .last_listening_online
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let changed = last.map(|previous| previous != online).unwrap_or(true);
        if changed {
            *last = Some(online);
        }
        changed
    };
    if should_emit {
        let snapshot = build_snapshot(app, state);
        let _ = app.emit("snapshot-changed", &snapshot);
    }
}

fn remember_hook_health(state: &AppState, hook_health: &HookHealthSnapshot) {
    if let Ok(mut last) = state.last_hook_health.lock() {
        *last = Some(hook_health.clone());
    }
}

fn sync_hook_health_snapshot(app: &AppHandle, state: &AppState) {
    let hook_health = build_hook_health(app);
    let should_emit = {
        let mut last = state
            .last_hook_health
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let changed = last
            .as_ref()
            .map(|previous| previous != &hook_health)
            .unwrap_or(true);
        if changed {
            *last = Some(hook_health);
        }
        changed
    };
    if should_emit {
        let snapshot = build_snapshot(app, state);
        remember_hook_health(state, &snapshot.hook_health);
        let _ = app.emit("snapshot-changed", &snapshot);
    }
}

fn build_hook_health(app: &AppHandle) -> HookHealthSnapshot {
    if capture::force_hook_uninstalled() {
        let claude_script_path =
            resolve_hook_script_path(app, "atoll-claude-hook.mjs").unwrap_or_default();
        let codex_script_path =
            resolve_hook_script_path(app, "atoll-codex-hook.mjs").unwrap_or_default();
        return HookHealthSnapshot {
            claude: HookStatus {
                installed: false,
                script_found: !claude_script_path.is_empty()
                    && std::path::Path::new(&claude_script_path).exists(),
                settings_path: claude_settings_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                script_path: claude_script_path,
                node_path: String::new(),
                node_found: resolve_node_executable().is_ok(),
            },
            codex: HookStatus {
                installed: false,
                script_found: !codex_script_path.is_empty()
                    && std::path::Path::new(&codex_script_path).exists(),
                settings_path: codex_hooks_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                script_path: codex_script_path,
                node_path: String::new(),
                node_found: resolve_node_executable().is_ok(),
            },
        };
    }

    let claude_status = claude_hook_status(app);
    let codex_status = codex_hook_status(app);

    HookHealthSnapshot {
        claude: claude_status,
        codex: codex_status,
    }
}

fn claude_hook_status(app: &AppHandle) -> HookStatus {
    let settings_path = claude_settings_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let settings = read_json_file(&settings_path);
    let installed = settings
        .as_ref()
        .map(|config| has_atoll_claude_hooks(config))
        .unwrap_or(false);
    let (script_path, script_found) =
        resolve_hook_script_readiness(app, "atoll-claude-hook.mjs", settings.as_ref());
    build_hook_status(
        installed,
        script_found,
        settings_path,
        script_path,
        settings.as_ref(),
        "atoll-claude-hook",
    )
}

fn codex_hook_status(app: &AppHandle) -> HookStatus {
    let hooks_path = codex_hooks_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let config = read_json_file(&hooks_path);
    let installed = config
        .as_ref()
        .map(|hooks| has_atoll_codex_hooks(hooks))
        .unwrap_or(false);
    let (script_path, mut script_found) =
        resolve_hook_script_readiness(app, "atoll-codex-hook.mjs", config.as_ref());
    if installed {
        if let (Some(cfg), Ok(preferred)) = (
            config.as_ref(),
            resolve_install_hook_script_path(app, "atoll-codex-hook.mjs"),
        ) {
            if let Some(configured) = configured_atoll_hook_script_path(cfg, "atoll-codex-hook") {
                if is_dev_hook_script_path(&configured)
                    && configured != preferred
                    && std::path::Path::new(&preferred).is_file()
                {
                    script_found = false;
                }
            }
        }
    }
    build_hook_status(
        installed,
        script_found,
        hooks_path,
        script_path,
        config.as_ref(),
        "atoll-codex-hook",
    )
}

fn is_dev_hook_script_path(path: &str) -> bool {
    path.contains("/target/debug/")
        || path.contains("/target/release/")
        || path.contains("/src-tauri/target/")
}

fn build_hook_status(
    installed: bool,
    script_found: bool,
    settings_path: String,
    script_path: String,
    config: Option<&Value>,
    marker: &str,
) -> HookStatus {
    let node_path = config
        .and_then(|cfg| configured_atoll_hook_node_path(cfg, marker))
        .unwrap_or_default();
    let node_found = node_executable_ready(&node_path);
    HookStatus {
        installed,
        script_found,
        settings_path,
        script_path,
        node_path,
        node_found,
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
    roll_over_token_usage_if_needed(&state);
    drop(requests);
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
    roll_over_token_usage_if_needed(&state);
    drop(requests);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn uses_micro_island() -> bool {
    cfg!(target_os = "windows")
}

#[tauri::command]
async fn set_island_presentation(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: IslandWindowMode,
    compact_width: Option<f64>,
    compact_left_width: Option<f64>,
    expanded_idle: Option<bool>,
    animate: Option<bool>,
    snap: Option<bool>,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    if let Some(width) = compact_width {
        let mut saved_width = state
            .compact_width
            .lock()
            .map_err(|error| error.to_string())?;
        *saved_width = sanitize_compact_width(width);
    }

    if let Some(left_width) = compact_left_width {
        let mut saved_left = state
            .compact_left_width
            .lock()
            .map_err(|error| error.to_string())?;
        *saved_left = if left_width.is_finite() {
            left_width.max(0.0)
        } else {
            0.0
        };
    }

    if animate == Some(false) {
        if snap == Some(true)
            && matches!(
                mode,
                IslandWindowMode::Micro | IslandWindowMode::Compact | IslandWindowMode::Dormant
            )
        {
            let compact_width = *state
                .compact_width
                .lock()
                .map_err(|error| error.to_string())?;
            let compact_left_width = *state
                .compact_left_width
                .lock()
                .map_err(|error| error.to_string())?;
            // apply_island_window_mode touches AppKit; must run on the main thread.
            let (sync_tx, sync_rx) =
                std::sync::mpsc::sync_channel::<Result<Option<HomeWindowBounds>, String>>(0);
            let frame_window = window.clone();
            window
                .run_on_main_thread(move || {
                    let result = apply_island_window_mode(
                        &frame_window,
                        mode,
                        compact_width,
                        compact_left_width,
                    )
                    .map_err(|error| error.to_string());
                    let _ = sync_tx.send(result);
                })
                .map_err(|error| error.to_string())?;
            let home = sync_rx.recv().map_err(|error| error.to_string())??;
            if let Some(home) = home {
                if let Ok(mut home_bounds) = state.home_bounds.lock() {
                    *home_bounds = Some(home);
                }
            }
        }
        return Ok(());
    }

    let generation = state.presentation_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let presentation_generation = Arc::clone(&state.presentation_generation);
    let compact_width = *state
        .compact_width
        .lock()
        .map_err(|error| error.to_string())?;
    let compact_left_width = *state
        .compact_left_width
        .lock()
        .map_err(|error| error.to_string())?;
    let home_bounds = *state
        .home_bounds
        .lock()
        .map_err(|error| error.to_string())?;
    let expanded_idle = expanded_idle.unwrap_or(false);

    tauri::async_runtime::spawn_blocking(move || {
        animate_island_window_mode(
            &window,
            mode,
            generation,
            &presentation_generation,
            home_bounds,
            compact_width,
            compact_left_width,
            expanded_idle,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn get_notch_metrics(state: State<'_, AppState>) -> NotchMetrics {
    *state.notch_metrics.lock().expect("state mutex poisoned")
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
    roll_over_token_usage_if_needed(&state);
    drop(requests);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn get_session_requests(state: State<'_, AppState>, session_id: String) -> Vec<PermissionRequest> {
    let requests = state.requests.lock().expect("state mutex poisoned");
    requests
        .iter()
        .filter(|r| !r.archived && r.session == session_id)
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_input: Option<Value>,
}

const TRANSCRIPT_MAX_MESSAGES: usize = 50;

#[tauri::command]
fn get_session_transcript(transcript_path: String) -> Result<Vec<ChatMessage>, String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let format = transcript::detect_transcript_format(&transcript_path);
    let file = File::open(&transcript_path).map_err(|e| format!("Cannot open transcript: {e}"))?;
    let reader = BufReader::new(file);

    let mut messages: Vec<ChatMessage> = Vec::new();

    if format == transcript::TranscriptFormat::Codex {
        let lines: Vec<String> = reader
            .lines()
            .map(|line| line.map_err(|e| format!("Read error: {e}")))
            .collect::<Result<Vec<_>, _>>()?;
        for parsed in transcript::parse_codex_messages(&lines) {
            if parsed.content.is_empty() && parsed.tool_name.is_none() {
                continue;
            }
            messages.push(ChatMessage {
                role: parsed.role,
                content: parsed.content,
                tool_name: parsed.tool_name,
                tool_input: None,
            });
        }
    } else {
        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read error: {e}"))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let entry: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let Some(msg_type) = entry.get("type").and_then(Value::as_str) else {
                continue;
            };

            match msg_type {
                "human" | "user" => {
                    let content = extract_transcript_text(&entry);
                    if !content.is_empty() {
                        messages.push(ChatMessage {
                            role: "user".into(),
                            content,
                            tool_name: None,
                            tool_input: None,
                        });
                    }
                }
                "assistant" => {
                    let content = extract_transcript_text(&entry);
                    let (tool_name, tool_input) = entry
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(Value::as_array)
                        .and_then(|arr| {
                            arr.iter().find_map(|block| {
                                if block.get("type")?.as_str()? == "tool_use" {
                                    let name = block
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .map(String::from)?;
                                    let input = block.get("input").cloned();
                                    Some((name, input))
                                } else {
                                    None
                                }
                            })
                        })
                        .map(|(name, input)| (Some(name), input))
                        .unwrap_or((None, None));
                    if !content.is_empty() || tool_name.is_some() {
                        messages.push(ChatMessage {
                            role: "assistant".into(),
                            content,
                            tool_name,
                            tool_input,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    let start = messages.len().saturating_sub(TRANSCRIPT_MAX_MESSAGES);
    Ok(messages[start..].to_vec())
}

fn extract_transcript_text(entry: &Value) -> String {
    if let Some(message) = entry.get("message") {
        if let Some(content) = message.get("content") {
            if let Some(text) = content.as_str() {
                return text.to_string();
            }
            if let Some(arr) = content.as_array() {
                let parts: Vec<&str> = arr
                    .iter()
                    .filter_map(|block| {
                        if block.get("type")?.as_str()? == "text" {
                            block.get("text").and_then(Value::as_str)
                        } else {
                            None
                        }
                    })
                    .collect();
                return parts.join("\n");
            }
        }
    }
    String::new()
}

fn collect_session_transcript_paths(
    requests: &[PermissionRequest],
    known_sessions: &HashMap<String, KnownSession>,
) -> Vec<(String, String, AgentKind)> {
    let mut session_paths: HashMap<String, (String, AgentKind)> = HashMap::new();
    for request in requests {
        if request.archived {
            continue;
        }
        let Some(transcript_path) = request.transcript_path.as_deref() else {
            continue;
        };
        session_paths
            .entry(request.session.clone())
            .or_insert_with(|| (transcript_path.to_string(), request.agent.clone()));
    }
    for (session_id, known_session) in known_sessions {
        let Some(transcript_path) = known_session.transcript_path.as_deref() else {
            continue;
        };
        session_paths
            .entry(session_id.clone())
            .or_insert_with(|| (transcript_path.to_string(), known_session.agent.clone()));
    }
    session_paths
        .into_iter()
        .map(|(session_id, (path, agent))| (session_id, path, agent))
        .collect()
}

fn current_local_day_key() -> String {
    local_time::current_local_day_key()
}

fn roll_over_token_usage_if_needed(state: &AppState) {
    let today = current_local_day_key();
    let (needs_rollover, previous_day) = {
        let mut usage_day = state
            .token_usage_day
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *usage_day == today {
            (false, String::new())
        } else {
            let previous = usage_day.clone();
            *usage_day = today;
            (true, previous)
        }
    };

    if !needs_rollover {
        return;
    }

    let _ = token_history::flush_day_to_history(state, &previous_day);

    if let Ok(mut usage_by_session) = state.session_token_usage.lock() {
        usage_by_session.clear();
    }
    if let Ok(mut sticky) = state.session_agent_map.lock() {
        sticky.clear();
    }
    if let Ok(mut offsets) = state.token_usage_file_offsets.lock() {
        offsets.clear();
    }
    if let Ok(mut baseline) = state.daily_tokens_baseline.lock() {
        *baseline = TokenUsage::default();
    }
}

fn token_usage_from_transcript_entry(entry: &Value, local_today_key: &str) -> TokenUsage {
    if entry.get("type").and_then(Value::as_str) != Some("assistant") {
        return TokenUsage::default();
    }

    let Some(timestamp) = entry.get("timestamp").and_then(Value::as_str) else {
        return TokenUsage::default();
    };
    if !local_time::is_local_today(timestamp, local_today_key) {
        return TokenUsage::default();
    }

    let usage = entry
        .get("message")
        .and_then(|message| message.get("usage"));
    TokenUsage {
        input_tokens: usage
            .and_then(|value| value.get("input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: usage
            .and_then(|value| value.get("output_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cache_read_tokens: usage
            .and_then(|value| value.get("cache_read_input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cache_creation_tokens: usage
            .and_then(|value| value.get("cache_creation_input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
    }
}

fn parse_claude_token_usage_from_transcript(
    transcript_path: &str,
    offset: u64,
    today_key: &str,
) -> Result<(TokenUsage, u64, bool), String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader, Seek, SeekFrom};

    let mut file =
        File::open(transcript_path).map_err(|error| format!("Cannot open transcript: {error}"))?;
    let file_len = file
        .metadata()
        .map_err(|error| format!("Cannot read transcript metadata: {error}"))?
        .len();
    let start_offset = if offset > file_len { 0 } else { offset };
    let is_full_scan = start_offset == 0;

    file.seek(SeekFrom::Start(start_offset))
        .map_err(|error| format!("Cannot seek transcript: {error}"))?;

    let mut reader = BufReader::new(file);
    let mut usage = TokenUsage::default();
    let mut next_offset = start_offset;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| format!("Cannot read transcript: {error}"))?;
        if bytes == 0 {
            break;
        }

        next_offset = next_offset.saturating_add(bytes as u64);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let entry: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        usage.add_assign(token_usage_from_transcript_entry(&entry, today_key));
    }

    Ok((usage, next_offset, is_full_scan))
}

fn token_usage_from_codex_delta(delta: transcript::TokenUsageDelta) -> TokenUsage {
    TokenUsage {
        input_tokens: delta.input_tokens,
        output_tokens: delta.output_tokens,
        cache_read_tokens: delta.cache_read_tokens,
        cache_creation_tokens: delta.cache_creation_tokens,
    }
}

fn parse_codex_token_usage_from_transcript(
    transcript_path: &str,
    offset: u64,
    today_key: &str,
) -> Result<(TokenUsage, u64, bool), String> {
    use std::fs::File;
    use std::io::BufReader;

    let file =
        File::open(transcript_path).map_err(|error| format!("Cannot open transcript: {error}"))?;
    let file_len = file
        .metadata()
        .map_err(|error| format!("Cannot read transcript metadata: {error}"))?
        .len();
    let start_offset = if offset > file_len { 0 } else { offset };
    let is_full_scan = start_offset == 0;

    let mut reader = BufReader::new(file);
    let parsed = transcript::parse_codex_tokens_from_reader(&mut reader, start_offset, today_key)?;
    Ok((
        token_usage_from_codex_delta(parsed.daily_delta),
        parsed.next_offset,
        is_full_scan,
    ))
}

pub(crate) fn refresh_session_token_usage(
    state: &AppState,
    session_id: &str,
    transcript_path: Option<&str>,
    agent: Option<&AgentKind>,
) -> Result<(), String> {
    let Some(transcript_path) = transcript_path else {
        return Ok(());
    };

    roll_over_token_usage_if_needed(state);
    let today_key = current_local_day_key();
    let last_offset = state
        .token_usage_file_offsets
        .lock()
        .map_err(|error| error.to_string())?
        .get(transcript_path)
        .copied()
        .unwrap_or(0);

    let format = match agent {
        Some(AgentKind::Codex) => transcript::TranscriptFormat::Codex,
        Some(AgentKind::Claude) => transcript::TranscriptFormat::Claude,
        _ => transcript::detect_transcript_format(transcript_path),
    };

    let (parsed_usage, next_offset, is_full_scan) = match format {
        transcript::TranscriptFormat::Codex => {
            parse_codex_token_usage_from_transcript(transcript_path, last_offset, &today_key)?
        }
        transcript::TranscriptFormat::Claude => {
            parse_claude_token_usage_from_transcript(transcript_path, last_offset, &today_key)?
        }
    };

    {
        let mut offsets = state
            .token_usage_file_offsets
            .lock()
            .map_err(|error| error.to_string())?;
        offsets.insert(transcript_path.to_string(), next_offset);
    }

    let mut usage_by_session = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    let usage_entry = usage_by_session.entry(session_id.to_string()).or_default();
    if is_full_scan {
        *usage_entry = parsed_usage;
    } else {
        usage_entry.add_assign(parsed_usage);
    }
    drop(usage_by_session);

    if let Some(agent) = agent {
        if let Ok(mut sticky) = state.session_agent_map.lock() {
            sticky
                .entry(session_id.to_string())
                .or_insert_with(|| token_history::agent_kind_key(agent));
        }
    }

    token_history::sync_today_to_history(state)?;

    Ok(())
}

#[tauri::command]
fn get_token_history(days: u32) -> Result<token_history::TokenHistoryResponse, String> {
    token_history::get_token_history(days)
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

const DEFAULT_SESSION_RETENTION_SECS: u64 = 900;
const DEFAULT_SUBAGENT_RETENTION_SECS: u64 = 600;

fn atoll_settings_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".atoll").join("settings.json"))
}

fn load_persisted_retention_secs() -> u64 {
    let Some(path) = atoll_settings_path() else {
        return DEFAULT_SESSION_RETENTION_SECS;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return DEFAULT_SESSION_RETENTION_SECS;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return DEFAULT_SESSION_RETENTION_SECS;
    };
    let minutes = value
        .get("sessionRetentionMinutes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_SESSION_RETENTION_SECS / 60);
    minutes.clamp(1, 60) * 60
}

fn load_persisted_subagent_retention_secs() -> u64 {
    let Some(path) = atoll_settings_path() else {
        return DEFAULT_SUBAGENT_RETENTION_SECS;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return DEFAULT_SUBAGENT_RETENTION_SECS;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return DEFAULT_SUBAGENT_RETENTION_SECS;
    };
    let minutes = value
        .get("subagentRetentionMinutes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_SUBAGENT_RETENTION_SECS / 60);
    minutes.clamp(1, 60) * 60
}

fn persist_settings(session_minutes: Option<u64>, subagent_minutes: Option<u64>) {
    let Some(path) = atoll_settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut config: Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    let obj = config.as_object_mut().unwrap();
    if let Some(m) = session_minutes {
        obj.insert("sessionRetentionMinutes".into(), Value::from(m));
    }
    if let Some(m) = subagent_minutes {
        obj.insert("subagentRetentionMinutes".into(), Value::from(m));
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, formatted);
    }
}

fn persist_retention_minutes(minutes: u64) {
    persist_settings(Some(minutes.clamp(1, 60)), None);
}

#[tauri::command]
fn get_session_retention(state: State<'_, AppState>) -> u64 {
    *state
        .session_retention_secs
        .lock()
        .expect("state mutex poisoned")
}

#[tauri::command]
fn set_session_retention(state: State<'_, AppState>, minutes: u64) -> u64 {
    let clamped_minutes = minutes.clamp(1, 60);
    let secs = clamped_minutes * 60;
    let mut retention = state
        .session_retention_secs
        .lock()
        .expect("state mutex poisoned");
    *retention = secs;
    persist_retention_minutes(clamped_minutes);
    secs
}

#[tauri::command]
fn get_subagent_retention(state: State<'_, AppState>) -> u64 {
    *state
        .subagent_retention_secs
        .lock()
        .expect("state mutex poisoned")
}

#[tauri::command]
fn set_subagent_retention(state: State<'_, AppState>, minutes: u64) -> u64 {
    let clamped_minutes = minutes.clamp(1, 60);
    let secs = clamped_minutes * 60;
    let mut retention = state
        .subagent_retention_secs
        .lock()
        .expect("state mutex poisoned");
    *retention = secs;
    persist_settings(None, Some(clamped_minutes));
    secs
}

#[tauri::command]
fn archive_subagent(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<IslandSnapshot, String> {
    if let Ok(mut subagents) = state.active_subagents.lock() {
        if let Some(sub) = subagents.iter_mut().find(|s| s.agent_id == agent_id) {
            sub.archived = true;
        }
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn archive_completed_subagents(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<IslandSnapshot, String> {
    if let Ok(mut subagents) = state.active_subagents.lock() {
        for sub in subagents.iter_mut() {
            if sub.session_id == session_id && sub.completed_at.is_some() && !sub.archived {
                sub.archived = true;
            }
        }
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

/// Codex background threads (memories, subagents, etc.) often omit `cwd` in hook payloads.
/// Atoll defaults missing cwd to `"."`, which would show up as a stray "." session.
/// Resolve the real workspace from `transcript_path` when possible, and only ignore
/// known Codex-internal directories under `~/.codex/`.
const CODEX_INTERNAL_DIR_NAMES: &[&str] = &[
    "memories",
    "process_manager",
    "computer-use",
    "computer-use-turn-ended",
];

fn normalize_codex_cwd(cwd: &str) -> String {
    cwd.replace('\\', "/")
}

pub(crate) fn resolve_codex_session_cwd(cwd: &str, transcript_path: Option<&str>) -> String {
    let normalized = normalize_codex_cwd(cwd);
    if !normalized.is_empty() && normalized != "." && normalized != "./" {
        return normalized;
    }

    transcript_path
        .and_then(transcript::read_codex_cwd_from_transcript)
        .map(|resolved| normalize_codex_cwd(&resolved))
        .filter(|resolved| !resolved.is_empty())
        .unwrap_or(normalized)
}

fn is_codex_internal_cwd(cwd: &str) -> bool {
    let normalized = normalize_codex_cwd(cwd);
    if normalized.is_empty() || normalized == "." || normalized == "./" {
        return true;
    }

    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let codex_home = normalize_codex_cwd(&home.join(".codex").to_string_lossy());

    for dir_name in CODEX_INTERNAL_DIR_NAMES {
        let internal = format!("{codex_home}/{dir_name}");
        if normalized == internal || normalized.starts_with(&(internal.clone() + "/")) {
            return true;
        }
    }

    false
}

pub(crate) fn is_codex_internal_session(
    agent: &AgentKind,
    cwd: &str,
    transcript_path: Option<&str>,
) -> bool {
    if !matches!(agent, AgentKind::Codex) {
        return false;
    }

    let resolved = resolve_codex_session_cwd(cwd, transcript_path);
    is_codex_internal_cwd(&resolved)
}

pub(crate) fn purge_tracked_session(
    state: &AppState,
    session_id: &str,
    transcript_path: Option<&str>,
) {
    if let Ok(mut known) = state.known_sessions.lock() {
        known.remove(session_id);
    }
    if let Ok(mut last_seen) = state.session_last_seen.lock() {
        last_seen.remove(session_id);
    }
    // Keep session_token_usage so auto-archived / retention-purged sessions still
    // count toward daily totals until UTC day rollover.
    if let Some(path) = transcript_path {
        if let Ok(mut offsets) = state.token_usage_file_offsets.lock() {
            offsets.remove(path);
        }
    }
}

pub(crate) fn register_known_session(
    state: &AppState,
    session_id: &str,
    agent: AgentKind,
    cwd: &str,
    transcript_path: Option<&str>,
) {
    let resolved_cwd = if matches!(agent, AgentKind::Codex) {
        resolve_codex_session_cwd(cwd, transcript_path)
    } else {
        cwd.to_string()
    };

    if is_codex_internal_session(&agent, &resolved_cwd, None) {
        purge_tracked_session(state, session_id, transcript_path);
        return;
    }
    if let Ok(mut known) = state.known_sessions.lock() {
        let entry = known
            .entry(session_id.to_string())
            .or_insert_with(|| KnownSession {
                agent: agent.clone(),
                cwd: resolved_cwd.clone(),
                transcript_path: transcript_path.map(str::to_string),
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::Unknown,
            });
        if !resolved_cwd.is_empty() && resolved_cwd != "." {
            entry.cwd = resolved_cwd.clone();
        }
        if let Some(path) = transcript_path {
            entry.transcript_path = Some(path.to_string());
        }
    }
    if let Ok(mut sticky) = state.session_agent_map.lock() {
        sticky
            .entry(session_id.to_string())
            .or_insert_with(|| token_history::agent_kind_key(&agent));
    }
}

pub(crate) fn claude_session_host(
    state: &AppState,
    session_id: &str,
    cwd: &str,
) -> platform::SessionHost {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if entry.host != platform::SessionHost::Unknown {
                return entry.host;
            }
            if let Some(path) = entry.transcript_path.as_deref() {
                if let Some(host) = host_from_claude_transcript_path(path) {
                    drop(known);
                    store_session_host(state, session_id, host);
                    return host;
                }
            }
        }
    }

    let detected = platform::detect_claude_session_host(cwd);
    if detected != platform::SessionHost::Unknown {
        store_session_host(state, session_id, detected);
    }
    detected
}

pub(crate) fn codex_session_host(
    state: &AppState,
    session_id: &str,
    cwd: &str,
) -> platform::SessionHost {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if entry.host != platform::SessionHost::Unknown {
                return entry.host;
            }
            if let Some(path) = entry.transcript_path.as_deref() {
                if let Some(host) = host_from_codex_transcript_path(path) {
                    drop(known);
                    store_session_host(state, session_id, host);
                    return host;
                }
            }
        }
    }

    let detected = platform::detect_codex_session_host(cwd);
    if detected != platform::SessionHost::Unknown {
        store_session_host(state, session_id, detected);
    }
    detected
}

pub(crate) fn store_session_host(state: &AppState, session_id: &str, host: platform::SessionHost) {
    if let Ok(mut known) = state.known_sessions.lock() {
        if let Some(entry) = known.get_mut(session_id) {
            entry.host = host;
        }
    }
}

fn session_host_for_summary(
    known_sessions: &HashMap<String, KnownSession>,
    session_id: &str,
    cwd: &str,
    agent: &AgentKind,
) -> platform::SessionHost {
    match agent {
        AgentKind::Claude => {
            if let Some(entry) = known_sessions.get(session_id) {
                if entry.host != platform::SessionHost::Unknown {
                    return entry.host;
                }
                if let Some(path) = entry.transcript_path.as_deref() {
                    if let Some(host) = host_from_claude_transcript_path(path) {
                        return host;
                    }
                }
            }
            let detected = platform::detect_claude_session_host(cwd);
            if detected != platform::SessionHost::Unknown {
                return detected;
            }
            if platform::is_claude_desktop_app_running() && !platform::frontmost_is_terminal() {
                return platform::SessionHost::ClaudeDesktop;
            }
            platform::SessionHost::Unknown
        }
        AgentKind::Codex => {
            if let Some(entry) = known_sessions.get(session_id) {
                if entry.host != platform::SessionHost::Unknown {
                    return entry.host;
                }
                if let Some(path) = entry.transcript_path.as_deref() {
                    if let Some(host) = host_from_codex_transcript_path(path) {
                        return host;
                    }
                }
            }
            let detected = platform::detect_codex_session_host(cwd);
            if detected != platform::SessionHost::Unknown {
                return detected;
            }
            if platform::is_codex_desktop_app_running() && !platform::frontmost_is_terminal() {
                return platform::SessionHost::CodexDesktop;
            }
            platform::SessionHost::Unknown
        }
        _ => platform::SessionHost::Unknown,
    }
}

fn host_from_claude_transcript_path(path: &str) -> Option<platform::SessionHost> {
    if path.contains("/Application Support/") && !path.contains("/.claude/") {
        return Some(platform::SessionHost::ClaudeDesktop);
    }
    if path.contains("Claude-3p")
        || path.contains("local-agent-mode-sessions")
        || path.contains("com.anthropic.claude")
        || path.contains("agent-sessions")
    {
        return Some(platform::SessionHost::ClaudeDesktop);
    }
    // /.claude/projects/ is used by BOTH Claude CLI and Claude Desktop (newer versions).
    // Only treat it as CLI if Claude Desktop is definitely not running.
    if path.contains("/.claude/")
        || (path.contains("/claude/projects/") && !path.contains("/Application Support/"))
    {
        if !platform::is_claude_desktop_app_running() {
            return Some(platform::SessionHost::ClaudeCli);
        }
        // Ambiguous: Desktop is running and path looks like CLI — return None
        // so the caller uses other detection methods.
        return None;
    }
    None
}

fn host_from_codex_transcript_path(path: &str) -> Option<platform::SessionHost> {
    if path.contains("com.openai.codex")
        || (path.contains("/Application Support/") && path.contains("codex"))
    {
        return Some(platform::SessionHost::CodexDesktop);
    }
    if path.contains("/.codex/sessions/") || path.contains("/.codex/") {
        if !platform::is_codex_desktop_app_running() {
            return Some(platform::SessionHost::CodexCli);
        }
        return None;
    }
    None
}

pub(crate) fn touch_session_last_seen(state: &AppState, session_id: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Ok(mut last_seen) = state.session_last_seen.lock() {
        last_seen.insert(session_id.to_string(), now);
    }
}

/// Bumps retention clocks for user-visible session activity (turn end, approvals).
pub(crate) fn touch_session_activity(state: &AppState, session_id: &str) {
    touch_session_last_seen(state, session_id);
    let now_iso = iso_timestamp_now();
    if let Ok(mut known) = state.known_sessions.lock() {
        if let Some(entry) = known.get_mut(session_id) {
            entry.last_activity = now_iso;
        }
    }
}

fn derive_subagent_transcript_path(
    main_transcript: Option<&str>,
    agent_id: &str,
) -> Option<String> {
    let main = main_transcript?;
    let stem = main.strip_suffix(".jsonl").unwrap_or(main);
    let filename = if agent_id.starts_with("agent-") {
        format!("{agent_id}.jsonl")
    } else {
        format!("agent-{agent_id}.jsonl")
    };
    let path = format!("{stem}/subagents/{filename}");
    if std::path::Path::new(&path).exists() {
        return Some(path);
    }
    let alt = format!("{stem}/subagents/{agent_id}.jsonl");
    if std::path::Path::new(&alt).exists() {
        return Some(alt);
    }
    Some(path)
}

pub(crate) fn register_subagent_start(
    state: &AppState,
    payload: &serde_json::Value,
    agent_kind: AgentKind,
) {
    let agent_id = payload
        .get("agent_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let session_id = payload
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| payload.get("sessionId").and_then(serde_json::Value::as_str))
        .unwrap_or("")
        .to_string();
    let agent_type = payload
        .get("agent_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let agent_transcript_path = payload
        .get("agent_transcript_path")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            let main_path = payload
                .get("transcript_path")
                .and_then(serde_json::Value::as_str);
            derive_subagent_transcript_path(main_path, &agent_id)
        });

    if agent_id.is_empty() || session_id.is_empty() {
        return;
    }

    let subagent = ActiveSubagent {
        agent_id,
        session_id,
        agent_kind,
        agent_type,
        started_at: iso_timestamp_now(),
        agent_transcript_path,
        completed_at: None,
        archived: false,
        last_message: None,
    };

    if let Ok(mut subagents) = state.active_subagents.lock() {
        if !subagents.iter().any(|s| s.agent_id == subagent.agent_id) {
            subagents.push(subagent);
        }
    }
}

pub(crate) fn complete_subagent(state: &AppState, payload: &serde_json::Value) {
    let agent_id = payload
        .get("agent_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if agent_id.is_empty() {
        return;
    }
    let transcript_path = payload
        .get("agent_transcript_path")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let last_message = payload
        .get("last_assistant_message")
        .and_then(serde_json::Value::as_str)
        .map(|s| s.chars().take(200).collect::<String>());
    if let Ok(mut subagents) = state.active_subagents.lock() {
        if let Some(sub) = subagents.iter_mut().find(|s| s.agent_id == agent_id) {
            mark_subagent_complete(sub, transcript_path, last_message);
        }
    }
}

fn mark_subagent_complete(
    sub: &mut ActiveSubagent,
    transcript_path: Option<String>,
    last_message: Option<String>,
) {
    if sub.completed_at.is_some() {
        return;
    }
    sub.completed_at = Some(iso_timestamp_now());
    if let Some(path) = transcript_path {
        sub.agent_transcript_path = Some(path);
    }
    if let Some(message) = last_message {
        sub.last_message = Some(message);
    }
}

pub(crate) fn reconcile_incomplete_subagents(state: &AppState) {
    let pending_paths: Vec<(usize, String)> = {
        let subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        subagents
            .iter()
            .enumerate()
            .filter(|(_, sub)| sub.completed_at.is_none() && !sub.archived)
            .filter_map(|(i, sub)| sub.agent_transcript_path.as_ref().map(|p| (i, p.clone())))
            .collect()
    };

    if pending_paths.is_empty() {
        return;
    }

    let results: Vec<(usize, String)> = pending_paths
        .into_iter()
        .filter_map(|(i, path)| {
            transcript::extract_subagent_terminal_message(&path).map(|msg| (i, msg))
        })
        .collect();

    if results.is_empty() {
        return;
    }

    let mut subagents = match state.active_subagents.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    for (i, message) in results {
        if let Some(sub) = subagents.get_mut(i) {
            if sub.completed_at.is_none() && !sub.archived {
                mark_subagent_complete(sub, None, Some(message));
            }
        }
    }
}

const SUBAGENT_SNAPSHOT_MIN_INTERVAL: Duration = Duration::from_millis(300);

/// Emit a snapshot for subagent lifecycle events with rate-limiting.
/// Returns true if a snapshot was emitted, false if throttled.
pub(crate) fn emit_subagent_snapshot(app: &AppHandle, state: &AppState) -> bool {
    let mut last = state
        .last_subagent_snapshot_emit
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if now.duration_since(*last) < SUBAGENT_SNAPSHOT_MIN_INTERVAL {
        return false;
    }
    *last = now;
    drop(last);
    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HookStatus {
    installed: bool,
    script_found: bool,
    settings_path: String,
    script_path: String,
    #[serde(default)]
    node_path: String,
    #[serde(default = "default_node_found")]
    node_found: bool,
}

fn default_node_found() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
struct HookHealthSnapshot {
    claude: HookStatus,
    codex: HookStatus,
}

impl Default for HookStatus {
    fn default() -> Self {
        Self {
            installed: false,
            script_found: false,
            settings_path: String::new(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: true,
        }
    }
}

#[tauri::command]
fn get_claude_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    if capture::force_hook_uninstalled() {
        let script_path =
            resolve_hook_script_path(&app, "atoll-claude-hook.mjs").unwrap_or_default();
        return Ok(HookStatus {
            installed: false,
            script_found: !script_path.is_empty() && std::path::Path::new(&script_path).exists(),
            settings_path: claude_settings_path()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            script_path,
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
        });
    }
    Ok(claude_hook_status(&app))
}

#[tauri::command]
fn install_claude_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let script_path = resolve_install_hook_script_path(&app, "atoll-claude-hook.mjs")?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = resolve_node_executable()?;

    let settings_path =
        claude_settings_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create ~/.claude directory: {e}"))?;
    }

    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Cannot read settings: {e}"))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    let hook_command = format_hook_command(
        hook_runner_for_command(&app).as_deref(),
        &node_path,
        &script_path,
    );
    let atoll_hooks = serde_json::json!({
        "PermissionRequest": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 1800
                    }
                ]
            }
        ],
        "PostToolUse": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "PostToolUseFailure": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "Stop": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "StopFailure": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "SubagentStop": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "SubagentStart": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ]
    });

    let settings_obj = settings
        .as_object_mut()
        .ok_or_else(|| "Settings file is not a JSON object".to_string())?;
    let hooks_entry = settings_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    upsert_claude_hook_events(hooks_entry, &atoll_hooks);

    let formatted = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize settings: {e}"))?;
    std::fs::write(&settings_path, formatted).map_err(|e| format!("Cannot write settings: {e}"))?;

    let written =
        std::fs::read_to_string(&settings_path).map_err(|e| format!("Cannot verify settings: {e}"))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse settings after write: {e}"))?;
    if !has_atoll_claude_hooks(&verify) {
        return Err(
            "Claude hooks were not saved correctly. Check permissions on ~/.claude/settings.json."
                .into(),
        );
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after Claude hook install: {error}");
    }

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(claude_hook_status(&app))
}

#[tauri::command]
fn uninstall_claude_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let settings_path =
        claude_settings_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !settings_path.exists() {
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: settings_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
        });
    }

    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot read settings: {e}"))?;
    let mut settings: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    if let Some(obj) = settings.as_object_mut() {
        if let Some(hooks) = obj.get_mut("hooks") {
            remove_atoll_claude_hooks(hooks);
            if hooks.as_object().map(|map| map.is_empty()).unwrap_or(false) {
                obj.remove("hooks");
            }
        }
    }

    let formatted = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize settings: {e}"))?;
    std::fs::write(&settings_path, formatted).map_err(|e| format!("Cannot write settings: {e}"))?;

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(claude_hook_status(&app))
}

fn claude_settings_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("settings.json"))
}

fn codex_hooks_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".codex").join("hooks.json"))
}

#[tauri::command]
fn get_codex_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    if capture::force_hook_uninstalled() {
        let script_path =
            resolve_hook_script_path(&app, "atoll-codex-hook.mjs").unwrap_or_default();
        return Ok(HookStatus {
            installed: false,
            script_found: !script_path.is_empty() && std::path::Path::new(&script_path).exists(),
            settings_path: codex_hooks_path()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            script_path,
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
        });
    }
    Ok(codex_hook_status(&app))
}

#[tauri::command]
fn install_codex_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let script_path = resolve_install_hook_script_path(&app, "atoll-codex-hook.mjs")?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = resolve_node_executable_for_codex()?;

    let hooks_path =
        codex_hooks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = hooks_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create ~/.codex directory: {e}"))?;
    }

    let mut config: Value = if hooks_path.exists() {
        let content =
            std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot read hooks: {e}"))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    let hook_command = format_hook_command(
        hook_runner_for_command(&app).as_deref(),
        &node_path,
        &script_path,
    );
    let atoll_hooks = serde_json::json!({
        "PermissionRequest": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 1800,
                        "statusMessage": "Atoll approval"
                    }
                ]
            }
        ],
        "PostToolUse": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30,
                        "statusMessage": "Atoll session sync"
                    }
                ]
            }
        ],
        "Stop": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30,
                        "statusMessage": "Atoll session sync"
                    }
                ]
            }
        ],
        "SubagentStop": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30,
                        "statusMessage": "Atoll session sync"
                    }
                ]
            }
        ],
        "SubagentStart": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30,
                        "statusMessage": "Atoll session sync"
                    }
                ]
            }
        ]
    });

    let config_obj = config
        .as_object_mut()
        .ok_or_else(|| "hooks.json is not a JSON object".to_string())?;
    let hooks_obj = config_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    upsert_codex_hook_events(hooks_obj, &atoll_hooks);

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize hooks: {e}"))?;
    std::fs::write(&hooks_path, formatted).map_err(|e| format!("Cannot write hooks: {e}"))?;

    let written =
        std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot verify hooks: {e}"))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse hooks after write: {e}"))?;
    if !has_atoll_codex_hooks(&verify) {
        return Err(
            "Codex hooks were not saved correctly. Check permissions on ~/.codex/hooks.json."
                .into(),
        );
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after Codex hook install: {error}");
    }

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(codex_hook_status(&app))
}

#[tauri::command]
fn uninstall_codex_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let hooks_path =
        codex_hooks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !hooks_path.exists() {
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: hooks_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
        });
    }

    let content =
        std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot read hooks: {e}"))?;
    let mut config: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    if let Some(hooks) = config.get_mut("hooks") {
        remove_atoll_codex_hooks(hooks);
    }

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize hooks: {e}"))?;
    std::fs::write(&hooks_path, formatted).map_err(|e| format!("Cannot write hooks: {e}"))?;

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(codex_hook_status(&app))
}

fn normalize_hook_script_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return String::new();
    }
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    dunce::simplified(std::path::Path::new(path))
        .to_string_lossy()
        .into_owned()
}

#[cfg(windows)]
fn resolve_node_executable_from_path() -> Option<String> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("node.exe");
            if candidate.is_file() {
                return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
            }
        }
    }
    None
}

fn resolve_node_executable() -> Result<String, String> {
    #[cfg(windows)]
    {
        if let Some(path) = resolve_node_executable_from_path() {
            return Ok(path);
        }

        for candidate in [
            r"C:\Program Files\nodejs\node.exe",
            r"C:\Program Files (x86)\nodejs\node.exe",
        ] {
            if std::path::Path::new(candidate).exists() {
                return Ok(normalize_hook_script_path(candidate));
            }
        }

        return Err(
            "Node.js not found. Install Node.js and ensure it is on PATH, then retry.".into(),
        );
    }

    #[cfg(not(windows))]
    {
        let output = std::process::Command::new("sh")
            .args(["-lc", "command -v node"])
            .output()
            .map_err(|error| format!("Cannot locate node: {error}"))?;
        if !output.status.success() {
            return Err(
                "Node.js not found. Install Node.js and ensure it is on PATH, then retry.".into(),
            );
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() || !std::path::Path::new(&path).exists() {
            return Err(
                "Node.js not found. Install Node.js and ensure it is on PATH, then retry.".into(),
            );
        }
        Ok(normalize_hook_script_path(&path))
    }
}

/// Prefer Codex Desktop's bundled Node when available so hooks work in the app sandbox.
fn resolve_codex_desktop_node_executable() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        for candidate in [
            "/Applications/Codex.app/Contents/Resources/cua_node/bin/node",
            "/Applications/Codex.app/Contents/Resources/node/bin/node",
        ] {
            if std::path::Path::new(candidate).is_file() {
                return Some(normalize_hook_script_path(candidate));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let candidate = std::path::PathBuf::from(local_app_data)
                .join("Programs")
                .join("Codex")
                .join("resources")
                .join("cua_node")
                .join("bin")
                .join("node.exe");
            if candidate.is_file() {
                return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
            }
        }
        for candidate in [
            r"C:\Program Files\Codex\resources\cua_node\bin\node.exe",
            r"C:\Program Files (x86)\Codex\resources\cua_node\bin\node.exe",
        ] {
            if std::path::Path::new(candidate).is_file() {
                return Some(normalize_hook_script_path(candidate));
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = ();
    }

    None
}

fn resolve_node_executable_for_codex() -> Result<String, String> {
    if let Some(path) = resolve_codex_desktop_node_executable() {
        return Ok(path);
    }
    resolve_node_executable()
}

/// Prefer the bundled hook script from the running Atoll app over dev build paths.
fn resolve_install_hook_script_path(app: &AppHandle, script_name: &str) -> Result<String, String> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join("scripts").join(script_name);
        if candidate.is_file() {
            return Ok(normalize_hook_script_path(&candidate.to_string_lossy()));
        }
    }
    resolve_hook_script_path(app, script_name)
        .ok_or_else(|| format!("Cannot locate hook script: {script_name}"))
}

fn format_hook_command(runner_path: Option<&str>, node_path: &str, script_path: &str) -> String {
    let node_path = {
        let mut path = normalize_hook_script_path(node_path);
        #[cfg(windows)]
        {
            path = path.replace('\\', "/");
        }
        path
    };
    let script_path = {
        let mut path = normalize_hook_script_path(script_path);
        #[cfg(windows)]
        {
            path = path.replace('\\', "/");
        }
        path
    };

    #[cfg(windows)]
    if let Some(runner_path) = runner_path {
        let runner_path = normalize_hook_script_path(runner_path).replace('\\', "/");
        return format!(
            "\"{}\" \"{}\" \"{}\"",
            runner_path.replace('"', "\\\""),
            node_path.replace('"', "\\\""),
            script_path.replace('"', "\\\"")
        );
    }

    format!(
        "\"{}\" \"{}\"",
        node_path.replace('"', "\\\""),
        script_path.replace('"', "\\\"")
    )
}

fn upsert_claude_hook_events(existing_hooks: &mut Value, atoll_hooks: &Value) {
    let Some(atoll_map) = atoll_hooks.as_object() else {
        return;
    };
    let hooks_obj = existing_hooks
        .as_object_mut()
        .expect("hooks value should be object");

    for (event, atoll_matchers) in atoll_map {
        let Some(atoll_array) = atoll_matchers.as_array() else {
            continue;
        };

        let mut merged: Vec<Value> = hooks_obj
            .get(event)
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter(|matcher| !matcher_group_has_atoll_claude(matcher))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        for matcher in atoll_array {
            merged.push(matcher.clone());
        }

        hooks_obj.insert(event.clone(), Value::Array(merged));
    }
}

fn remove_atoll_claude_hooks(hooks: &mut Value) {
    let Some(hooks_obj) = hooks.as_object_mut() else {
        return;
    };

    for matchers in hooks_obj.values_mut() {
        if let Some(arr) = matchers.as_array_mut() {
            for matcher in arr.iter_mut() {
                if let Some(hook_arr) = matcher.get_mut("hooks").and_then(Value::as_array_mut) {
                    hook_arr.retain(|hook| {
                        !hook
                            .get("command")
                            .and_then(Value::as_str)
                            .map(|cmd| cmd.contains("atoll-claude-hook"))
                            .unwrap_or(false)
                    });
                }
            }
            arr.retain(|matcher| {
                matcher
                    .get("hooks")
                    .and_then(Value::as_array)
                    .map(|hooks| !hooks.is_empty())
                    .unwrap_or(false)
            });
        }
    }

    hooks_obj.retain(|_, matchers| {
        matchers
            .as_array()
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    });
}

fn matcher_group_has_atoll_claude(matcher: &Value) -> bool {
    matcher
        .get("hooks")
        .and_then(Value::as_array)
        .map(|hook_arr| {
            hook_arr.iter().any(|hook| {
                hook.get("command")
                    .and_then(Value::as_str)
                    .map(|cmd| cmd.contains("atoll-claude-hook"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn upsert_codex_hook_events(existing_hooks: &mut Value, atoll_hooks: &Value) {
    let Some(atoll_map) = atoll_hooks.as_object() else {
        return;
    };
    let hooks_obj = existing_hooks
        .as_object_mut()
        .expect("hooks value should be object");

    for (event, atoll_matchers) in atoll_map {
        let Some(atoll_array) = atoll_matchers.as_array() else {
            continue;
        };

        let mut merged: Vec<Value> = hooks_obj
            .get(event)
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter(|matcher| !matcher_group_has_atoll_codex(matcher))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        for matcher in atoll_array {
            merged.push(matcher.clone());
        }

        hooks_obj.insert(event.clone(), Value::Array(merged));
    }
}

fn remove_atoll_codex_hooks(hooks: &mut Value) {
    let Some(hooks_obj) = hooks.as_object_mut() else {
        return;
    };

    for matchers in hooks_obj.values_mut() {
        if let Some(arr) = matchers.as_array_mut() {
            for matcher in arr.iter_mut() {
                if let Some(hook_arr) = matcher.get_mut("hooks").and_then(Value::as_array_mut) {
                    hook_arr.retain(|hook| {
                        !hook
                            .get("command")
                            .and_then(Value::as_str)
                            .map(|cmd| cmd.contains("atoll-codex-hook"))
                            .unwrap_or(false)
                    });
                }
            }
            arr.retain(|matcher| {
                matcher
                    .get("hooks")
                    .and_then(Value::as_array)
                    .map(|hooks| !hooks.is_empty())
                    .unwrap_or(false)
            });
        }
    }

    hooks_obj.retain(|_, matchers| {
        matchers
            .as_array()
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    });
}

fn read_json_file(path: &str) -> Option<Value> {
    if path.is_empty() || !std::path::Path::new(path).exists() {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn extract_first_quoted_value(input: &str) -> Option<(String, &str)> {
    let input = input.trim();
    let inner = input.strip_prefix('"')?;
    let end = inner.find('"')?;
    let value = inner[..end].replace("\\\"", "\"");
    let rest = inner[end + 1..].trim_start();
    Some((value, rest))
}

fn extract_hook_command_parts(command: &str) -> Option<(String, String)> {
    let trimmed = command.trim();
    if let Some(rest) = trimmed.strip_prefix("node ") {
        let script = if rest.starts_with('"') {
            extract_first_quoted_value(rest)?.0
        } else {
            rest.split_whitespace().next()?.to_string()
        };
        return Some(("node".to_string(), normalize_hook_script_path(&script)));
    }

    let (first, rest) = extract_first_quoted_value(trimmed)?;
    if is_hook_runner_path(&first) {
        let (node, script_rest) = extract_first_quoted_value(rest)?;
        let (script, _) = extract_first_quoted_value(script_rest)?;
        return Some((
            normalize_hook_script_path(&node),
            normalize_hook_script_path(&script),
        ));
    }

    let (node, rest) = (first, rest);
    let (script, _) = extract_first_quoted_value(rest)?;
    Some((
        normalize_hook_script_path(&node),
        normalize_hook_script_path(&script),
    ))
}

fn is_hook_runner_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with("/atoll-hook-runner.exe")
        || normalized.ends_with("/atoll-hook-runner")
        || normalized.contains("/atoll-hook-runner-")
}

fn extract_node_script_path(command: &str) -> Option<String> {
    extract_hook_command_parts(command).map(|(_, script)| script)
}

fn configured_atoll_hook_node_path(config: &Value, marker: &str) -> Option<String> {
    let hooks = config.get("hooks")?.as_object()?;
    for matchers in hooks.values() {
        let arr = matchers.as_array()?;
        for matcher in arr {
            let hook_arr = matcher.get("hooks")?.as_array()?;
            for hook in hook_arr {
                let cmd = hook.get("command")?.as_str()?;
                if cmd.contains(marker) {
                    if let Some((node, _)) = extract_hook_command_parts(cmd) {
                        return Some(node);
                    }
                }
            }
        }
    }
    None
}

fn node_executable_ready(node_path: &str) -> bool {
    if node_path.is_empty() {
        return resolve_node_executable().is_ok();
    }
    if node_path == "node" {
        return resolve_node_executable().is_ok();
    }
    std::path::Path::new(node_path).exists()
}

fn configured_atoll_hook_script_path(config: &Value, marker: &str) -> Option<String> {
    let hooks = config.get("hooks")?.as_object()?;
    for matchers in hooks.values() {
        let arr = matchers.as_array()?;
        for matcher in arr {
            let hook_arr = matcher.get("hooks")?.as_array()?;
            for hook in hook_arr {
                let cmd = hook.get("command")?.as_str()?;
                if cmd.contains(marker) {
                    if let Some(path) = extract_node_script_path(cmd) {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn resolve_hook_script_readiness(
    app: &AppHandle,
    script_name: &str,
    config: Option<&Value>,
) -> (String, bool) {
    let marker = script_name.trim_end_matches(".mjs");
    let mut script_path = resolve_hook_script_path(app, script_name).unwrap_or_default();
    let mut script_found = !script_path.is_empty() && std::path::Path::new(&script_path).exists();

    if !script_found {
        if let Some(configured) =
            config.and_then(|cfg| configured_atoll_hook_script_path(cfg, marker))
        {
            if std::path::Path::new(&configured).exists() {
                script_found = true;
                if script_path.is_empty() {
                    script_path = configured;
                }
            }
        }
    }

    (script_path, script_found)
}

fn resolve_hook_script_path(app: &AppHandle, script_name: &str) -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join(script_name));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("resources").join("scripts").join(script_name));
            candidates.push(exe_dir.join("scripts").join(script_name));
        }
        for ancestor in exe.ancestors().skip(1) {
            candidates.push(ancestor.join("Resources").join("scripts").join(script_name));
            candidates.push(ancestor.join("scripts").join(script_name));
            if ancestor.join("src-tauri").exists() {
                candidates.push(ancestor.join("scripts").join(script_name));
            }
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
        }
    }

    None
}

#[cfg(windows)]
fn resolve_hook_runner_path(app: &AppHandle) -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join("atoll-hook-runner.exe"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(
                exe_dir
                    .join("resources")
                    .join("scripts")
                    .join("atoll-hook-runner.exe"),
            );
            candidates.push(exe_dir.join("scripts").join("atoll-hook-runner.exe"));
        }
        for ancestor in exe.ancestors().skip(1) {
            if ancestor.join("src-tauri").exists() {
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("generated")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("target")
                        .join("debug")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("target")
                        .join("release")
                        .join("atoll-hook-runner.exe"),
                );
            }
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
        }
    }

    None
}

#[cfg(not(windows))]
fn resolve_hook_runner_path(_app: &AppHandle) -> Option<String> {
    None
}

fn hook_runner_for_command(app: &AppHandle) -> Option<String> {
    resolve_hook_runner_path(app)
}

fn has_atoll_claude_hooks(settings: &Value) -> bool {
    let Some(hooks) = settings.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    let has_atoll_command = |matchers: &Value| {
        matchers
            .as_array()
            .map(|arr| {
                arr.iter().any(|matcher| {
                    matcher
                        .get("hooks")
                        .and_then(Value::as_array)
                        .map(|hook_arr| {
                            hook_arr.iter().any(|hook| {
                                hook.get("command")
                                    .and_then(Value::as_str)
                                    .map(|cmd| cmd.contains("atoll-claude-hook"))
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    };

    // "Stop" is required for token refresh on normal (no-tool) turns.
    ["PermissionRequest", "PostToolUse", "Stop"]
        .iter()
        .all(|event| hooks.get(*event).map(has_atoll_command).unwrap_or(false))
}

fn has_atoll_codex_hooks(config: &Value) -> bool {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    ["PermissionRequest", "PostToolUse", "Stop", "SubagentStop"]
        .iter()
        .all(|event| {
            hooks
                .get(*event)
                .map(|matchers| {
                    matchers
                        .as_array()
                        .map(|arr| arr.iter().any(matcher_group_has_atoll_codex))
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        })
}

fn matcher_group_has_atoll_codex(matcher: &Value) -> bool {
    matcher
        .get("hooks")
        .and_then(Value::as_array)
        .map(|hook_arr| {
            hook_arr.iter().any(|hook| {
                hook.get("command")
                    .and_then(Value::as_str)
                    .map(|cmd| cmd.contains("atoll-codex-hook"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

#[tauri::command]
fn open_in_terminal(cwd: String) -> Result<(), String> {
    platform::open_in_terminal(&cwd)
}

#[tauri::command]
fn focus_claude_app(app: AppHandle) -> Result<(), String> {
    platform::focus_claude_app(&app)
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    platform::open_url(&app, &url)
}

#[tauri::command]
fn is_autostart_enabled() -> Result<bool, String> {
    platform::autostart::is_enabled()
}

#[tauri::command]
fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        platform::autostart::enable()
    } else {
        platform::autostart::disable()
    }
}

#[tauri::command]
fn quit_atoll(app: AppHandle) {
    exit_atoll(&app);
}

#[tauri::command]
fn deactivate_atoll(
    app: AppHandle,
    state: State<'_, AppState>,
    agent: Option<String>,
    session: Option<String>,
    cwd: Option<String>,
) {
    platform::restore_focus_after_approval(
        &app,
        &state,
        agent.as_deref(),
        session.as_deref(),
        cwd.as_deref(),
    );
}

#[tauri::command]
fn open_agent_app(
    app: AppHandle,
    state: State<'_, AppState>,
    agent: String,
    cwd: String,
    session: Option<String>,
) -> Result<(), String> {
    platform::open_agent_app(&app, &state, &agent, &cwd, session.as_deref())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            requests: Mutex::new(Vec::new()),
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
            session_agent_map: Mutex::new(HashMap::new()),
            token_usage_file_offsets: Mutex::new(HashMap::new()),
            token_usage_day: Mutex::new(current_local_day_key()),
            daily_tokens_baseline: Mutex::new(token_history::load_today_baseline()),
            known_sessions: Mutex::new(HashMap::new()),
            pinned_sessions: Mutex::new(HashSet::new()),
            previous_app_pid: Mutex::new(None),
            last_listening_online: Mutex::new(None),
            last_hook_health: Mutex::new(None),
            bridge_port: AtomicU16::new(0),
            active_subagents: Mutex::new(Vec::new()),
            last_subagent_snapshot_emit: Mutex::new(Instant::now() - Duration::from_secs(10)),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_session_requests,
            get_session_transcript,
            resolve_permission_request,
            resolve_permission_with_input,
            set_session_auto_approve,
            archive_request,
            archive_all_resolved,
            archive_session,
            pin_session,
            set_island_presentation,
            get_notch_metrics,
            uses_micro_island,
            get_claude_hook_status,
            install_claude_hooks,
            uninstall_claude_hooks,
            get_codex_hook_status,
            install_codex_hooks,
            uninstall_codex_hooks,
            get_session_retention,
            set_session_retention,
            get_subagent_retention,
            set_subagent_retention,
            archive_subagent,
            archive_completed_subagents,
            get_token_history,
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
            }

            if !platform::setup_app(app) {
                std::process::exit(0);
            }

            build_tray(app.handle())?;
            hook_bridge::start_server(app.handle().clone());
            start_island_hover_monitor(app.handle().clone());
            {
                let state = app.state::<AppState>();
                let retention = load_persisted_retention_secs();
                *state
                    .session_retention_secs
                    .lock()
                    .expect("state mutex poisoned") = retention;
                let sub_retention = load_persisted_subagent_retention_secs();
                *state
                    .subagent_retention_secs
                    .lock()
                    .expect("state mutex poisoned") = sub_retention;
            }
            start_auto_archive_timer(app.handle().clone());
            start_token_refresh_timer(app.handle().clone());

            if capture::enabled() {
                let state = app.state::<AppState>();
                capture::seed_approval_demo(app.handle(), &state);
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_shadow(false);
                let _ = window.set_skip_taskbar(true);
                let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
                let _ = window.show();
                // Apply island style AFTER show() so the window number is
                // assigned and the NSPanel promotion takes effect.
                platform::apply_island_window_style(&window);
                eprintln!("[Atoll] step: island style applied, now applying mode...");
                let initial_mode = if cfg!(target_os = "windows") {
                    IslandWindowMode::Micro
                } else {
                    IslandWindowMode::Compact
                };
                if let Ok(Some(home)) =
                    apply_island_window_mode(&window, initial_mode, COMPACT_WINDOW_WIDTH, 0.0)
                {
                    eprintln!("[Atoll] step: island window mode applied");
                    let state = app.state::<AppState>();
                    if let Ok(mut home_bounds) = state.home_bounds.lock() {
                        *home_bounds = Some(home);
                    };
                    if let Ok(mut notch_metrics) = state.notch_metrics.lock() {
                        *notch_metrics = home.notch;
                    };
                }
                eprintln!("[Atoll] step: setup window complete");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Atoll");
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let [(show_id, show_label), (quit_id, quit_label)] = tray_menu_entries();
    let show = MenuItem::with_id(app, show_id, show_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, quit_id, quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => exit_atoll(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            match event {
                TrayIconEvent::Enter { .. } => {
                    show_main_window(&app);
                }
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    show_main_window(&app);
                }
                _ => {}
            }
        });

    if let Some(icon) = platform::tray_icon(app) {
        builder = builder.icon(icon);
    }

    builder.build(app)?;

    Ok(())
}

fn tray_menu_entries() -> [(&'static str, &'static str); 2] {
    [("show", "Show Atoll"), ("quit", "Quit")]
}

const AUTO_ARCHIVE_INTERVAL: Duration = Duration::from_secs(10);
const TOKEN_REFRESH_INTERVAL: Duration = Duration::from_millis(900);
const TOKEN_SNAPSHOT_MIN_INTERVAL: Duration = Duration::from_secs(2);

fn start_auto_archive_timer(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(AUTO_ARCHIVE_INTERVAL);

        let state = app.state::<AppState>();
        let (changed, expired, stale_pending_ids) = {
            let Ok(mut requests) = state.requests.lock() else {
                continue;
            };
            let retention_secs = *state
                .session_retention_secs
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let pinned = state
                .pinned_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let last_seen_map = state
                .session_last_seen
                .lock()
                .unwrap_or_else(|e| e.into_inner());

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut changed = false;
            let mut stale_pending_ids: Vec<String> = Vec::new();
            for request in requests.iter_mut() {
                if request.archived {
                    continue;
                }
                // Skip pinned sessions from auto-archive.
                if pinned.contains(&request.session) {
                    continue;
                }
                let last_seen_ts = last_seen_map
                    .get(&request.session)
                    .copied()
                    .unwrap_or_else(|| parse_iso_timestamp_secs(&request.requested_at));
                if now.saturating_sub(last_seen_ts) < retention_secs {
                    continue;
                }
                if request.status == PermissionStatus::Pending {
                    request.status = PermissionStatus::Denied;
                    if !request.detail.contains("Auto-archived") {
                        request.detail =
                            format!("{} Auto-archived after idle timeout.", request.detail);
                    }
                    stale_pending_ids.push(request.id.clone());
                }
                request.archived = true;
                changed = true;
            }
            drop(last_seen_map);

            // Collect expired known sessions while locks are held; purge after dropping
            // all guards so purge_tracked_session can acquire them independently.
            let expired = {
                let last_seen = state
                    .session_last_seen
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let mut known = state
                    .known_sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let mut expired: Vec<(String, Option<String>)> = Vec::new();
                known.retain(|session_id, info| {
                    if pinned.contains(session_id) {
                        return true;
                    }
                    let last_seen_ts = last_seen
                        .get(session_id)
                        .copied()
                        .unwrap_or_else(|| parse_iso_timestamp_secs(&info.last_activity));
                    if now.saturating_sub(last_seen_ts) >= retention_secs {
                        expired.push((session_id.clone(), info.transcript_path.clone()));
                        false
                    } else {
                        true
                    }
                });
                expired
            };
            if !expired.is_empty() {
                changed = true;
            }

            (changed, expired, stale_pending_ids)
        };

        if !stale_pending_ids.is_empty() {
            if let Ok(mut waiters) = state.hook_waiters.lock() {
                for request_id in stale_pending_ids {
                    if let Some(waiter) = waiters.remove(&request_id) {
                        let _ = waiter.send(DecisionWithNote {
                            decision: Decision::Denied,
                            note: "Auto-archived after idle timeout.".into(),
                            updated_input: None,
                        });
                    }
                }
            }
        }

        for (session_id, transcript_path) in expired {
            purge_tracked_session(&state, &session_id, transcript_path.as_deref());
        }

        if changed {
            roll_over_token_usage_if_needed(&state);
            let snapshot = build_snapshot(&app, &state);
            let _ = app.emit("snapshot-changed", &snapshot);
        }
        sync_hook_health_snapshot(&app, &state);
        sync_listening_online_snapshot(&app, &state);
    });
}

fn start_token_refresh_timer(app: AppHandle) {
    thread::spawn(move || {
        let mut last_snapshot_emit = Instant::now() - TOKEN_SNAPSHOT_MIN_INTERVAL;

        loop {
            thread::sleep(TOKEN_REFRESH_INTERVAL);

            let state = app.state::<AppState>();
            let tracked_sessions = {
                let requests = state.requests.lock().unwrap_or_else(|e| e.into_inner());
                let known_sessions = state
                    .known_sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                collect_session_transcript_paths(&requests, &known_sessions)
            };
            if tracked_sessions.is_empty() {
                continue;
            }

            let usage_before = state
                .session_token_usage
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();

            for (session_id, transcript_path, agent) in tracked_sessions {
                if let Err(error) = refresh_session_token_usage(
                    &state,
                    &session_id,
                    Some(transcript_path.as_str()),
                    Some(&agent),
                ) {
                    eprintln!("Atoll token usage refresh failed: {error}");
                }
            }

            let usage_after = state
                .session_token_usage
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if usage_after == usage_before {
                continue;
            }

            roll_over_token_usage_if_needed(&state);
            let now = Instant::now();
            if now.duration_since(last_snapshot_emit) < TOKEN_SNAPSHOT_MIN_INTERVAL {
                continue;
            }
            last_snapshot_emit = now;
            let snapshot = build_snapshot(&app, &state);
            let _ = app.emit("snapshot-changed", &snapshot);
        }
    });
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

fn exit_atoll(app: &AppHandle) {
    app.cleanup_before_exit();
    std::process::exit(0);
}

fn show_main_window_with_focus(app: &AppHandle, request_focus: bool) {
    if let Some(window) = app.get_webview_window("main") {
        platform::finish_show_for_approval(&window, app, request_focus);
        let _ = app.emit("island-open-requested", ());
    }
}

fn show_main_window(app: &AppHandle) {
    show_main_window_with_focus(app, false);
}

pub(crate) fn show_main_window_for_approval(app: &AppHandle) {
    show_main_window_with_focus(app, true);
}

fn start_island_hover_monitor(app: AppHandle) {
    thread::spawn(move || {
        let mut last_hovering = false;
        #[cfg(target_os = "windows")]
        let mut compact_hover_since: Option<Instant> = None;

        loop {
            #[cfg(target_os = "windows")]
            thread::sleep(Duration::from_millis(16));
            #[cfg(not(target_os = "windows"))]
            thread::sleep(Duration::from_millis(80));

            let Some(window) = app.get_webview_window("main") else {
                continue;
            };

            let cursor_over_window = is_cursor_over_window(&window).unwrap_or(false);
            #[cfg(target_os = "windows")]
            let hovering = if platform::is_island_expanded() {
                cursor_over_window
            } else if cursor_over_window {
                let now = Instant::now();
                if compact_hover_since.is_none() {
                    compact_hover_since = Some(now);
                }
                compact_hover_since.is_some_and(|since| {
                    now.duration_since(since) >= platform::compact_hover_expand_dwell()
                })
            } else {
                compact_hover_since = None;
                false
            };
            #[cfg(not(target_os = "windows"))]
            let hovering = cursor_over_window;

            #[cfg(target_os = "windows")]
            platform::sync_cursor_pass_through(&window, cursor_over_window);
            let client = if hovering {
                cursor_client_point(&window)
            } else {
                None
            };

            if hovering {
                let _ = app.emit(
                    "island-hover-changed",
                    IslandHoverChanged {
                        hovering: true,
                        client_x: client.map(|(x, _)| x),
                        client_y: client.map(|(_, y)| y),
                    },
                );
                last_hovering = true;
            } else if last_hovering {
                let _ = app.emit(
                    "island-hover-changed",
                    IslandHoverChanged {
                        hovering: false,
                        client_x: None,
                        client_y: None,
                    },
                );
                last_hovering = false;
            }
        }
    });
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

fn apply_island_window_mode(
    window: &tauri::WebviewWindow,
    mode: IslandWindowMode,
    compact_width: f64,
    compact_left_width: f64,
) -> tauri::Result<Option<HomeWindowBounds>> {
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(None);
    };

    platform::apply_island_window_style(window);
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));

    let scale_factor = monitor.scale_factor();
    let monitor_position = monitor.position().to_logical::<f64>(scale_factor);
    let monitor_size = monitor.size().to_logical::<f64>(scale_factor);
    let monitor_top = platform::monitor_top_y(window, &monitor);
    let notch = platform::detect_notch_metrics(window, monitor_position.x, monitor_size.width);

    window.set_size(island_window_logical_size(
        mode,
        compact_width,
        notch,
        false,
    ))?;
    platform::set_island_cursor_events_ignored(window, is_collapsed_pass_through_mode(mode));

    let window_size = island_window_physical_size(mode, scale_factor, compact_width, notch, false);
    let logical_window_size = window_size.to_logical::<f64>(scale_factor);
    let left_pane_width = if compact_left_width > 0.0 {
        compact_left_width
    } else {
        default_compact_left_pane_width(logical_window_size.width, notch)
    };
    let centered_x = compact_window_origin_x(
        monitor_position.x + monitor_size.width / 2.0,
        logical_window_size.width,
        notch,
        left_pane_width,
        mode,
    );
    // Keep the window flush with the physical top edge so the capsule overlaps
    // the notch / menu-bar band; the actual content is pushed below the notch
    // height inside the web view. On non-notched screens this is unchanged.
    let centered_y = monitor_top;
    let position = LogicalPosition::new(centered_x, centered_y);
    let home = HomeWindowBounds {
        position,
        // Fixed reference size for animation scale-factor recovery:
        //   compact_size.width / COMPACT_WINDOW_WIDTH == scale_factor
        // Computed directly so the FALLBACK_NOTCH_WIDTH minimum width floor
        // that island_window_logical_size applies does not distort the ratio.
        compact_size: PhysicalSize::new(
            (COMPACT_WINDOW_WIDTH * scale_factor).round() as u32,
            (COMPACT_WINDOW_HEIGHT * scale_factor).round() as u32,
        ),
        monitor_top_y: monitor_top,
        monitor_center_x: monitor_position.x + monitor_size.width / 2.0,
        notch,
        screen_geometry: platform::screen_geometry_for_monitor(
            window,
            monitor_position.x,
            monitor_size.width,
        ),
    };

    platform::set_island_window_frame_now(window, position, window_size, scale_factor, home)?;
    Ok(Some(home))
}

fn animate_island_window_mode(
    window: &tauri::WebviewWindow,
    mode: IslandWindowMode,
    generation: u64,
    presentation_generation: &AtomicU64,
    home_bounds: Option<HomeWindowBounds>,
    compact_width: f64,
    compact_left_width: f64,
    expanded_idle: bool,
) -> tauri::Result<()> {
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    if matches!(mode, IslandWindowMode::Expanded) {
        platform::set_island_cursor_events_ignored(window, false);
    } else {
        #[cfg(target_os = "windows")]
        platform::set_island_cursor_events_ignored(window, true);
    }

    let scale_factor = home_bounds
        .map(|home| home.compact_size.width as f64 / COMPACT_WINDOW_WIDTH)
        .unwrap_or_else(|| window.scale_factor().unwrap_or(1.0));
    let start_position = window.outer_position()?.to_logical::<f64>(scale_factor);
    let start_size = window.outer_size()?;
    let notch = home_bounds.map(|home| home.notch).unwrap_or_default();
    let target_size =
        island_window_physical_size(mode, scale_factor, compact_width, notch, expanded_idle);
    let target_logical_size = target_size.to_logical::<f64>(scale_factor);
    // Center the target window on the screen center.  Using monitor_center_x
    // instead of deriving from home.position avoids mis-centering when the
    // initial home position was set from a narrower mode (e.g. dormant 260px
    // vs compact 460px).
    let (target_x, target_y) = home_bounds
        .map(|home| {
            let left_pane_width = if compact_left_width > 0.0 {
                compact_left_width
            } else {
                default_compact_left_pane_width(target_logical_size.width, home.notch)
            };
            (
                compact_window_origin_x(
                    home.monitor_center_x,
                    target_logical_size.width,
                    home.notch,
                    left_pane_width,
                    mode,
                ),
                home.position.y,
            )
        })
        .unwrap_or_else(|| {
            (
                start_position.x
                    + (start_size.width as f64 / scale_factor - target_logical_size.width) / 2.0,
                start_position.y,
            )
        });
    let started_at = Instant::now();
    let mut next_frame_at = started_at;

    loop {
        if presentation_generation.load(Ordering::SeqCst) != generation {
            return Ok(());
        }

        let progress =
            (started_at.elapsed().as_secs_f64() / WINDOW_ANIMATION_DURATION.as_secs_f64()).min(1.0);
        let eased = ease_out_cubic(progress);
        let size = PhysicalSize::new(
            interpolate_u32(start_size.width, target_size.width, eased),
            interpolate_u32(start_size.height, target_size.height, eased),
        );
        let position = LogicalPosition::new(
            interpolate_f64(start_position.x, target_x, eased),
            interpolate_f64(start_position.y, target_y, eased),
        );

        platform::set_island_window_frame(window, position, size, scale_factor, home_bounds)?;

        if progress >= 1.0 {
            break;
        }

        next_frame_at += WINDOW_ANIMATION_FRAME;
        if let Some(delay) = next_frame_at.checked_duration_since(Instant::now()) {
            thread::sleep(delay);
        }
    }

    let (sync_tx, sync_rx) = std::sync::mpsc::sync_channel::<()>(0);
    let _ = window.run_on_main_thread(move || {
        let _ = sync_tx.send(());
    });
    let _ = sync_rx.recv();

    platform::set_island_cursor_events_ignored(window, is_collapsed_pass_through_mode(mode));
    Ok(())
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

fn interpolate_u32(start: u32, end: u32, progress: f64) -> u32 {
    (start as f64 + (end as f64 - start as f64) * progress).round() as u32
}

fn interpolate_f64(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress
}

fn expanded_window_height(expanded_idle: bool) -> f64 {
    if expanded_idle {
        EXPANDED_IDLE_WINDOW_HEIGHT
    } else {
        EXPANDED_WINDOW_HEIGHT
    }
}

fn island_window_logical_size(
    mode: IslandWindowMode,
    compact_width: f64,
    notch: NotchMetrics,
    expanded_idle: bool,
) -> LogicalSize<f64> {
    let compact_width = sanitize_compact_width(compact_width);
    let extra_top = if notch.has_notch {
        notch.height + NOTCH_COVER_PADDING
    } else {
        0.0
    };
    let min_notch_width = if notch.has_notch { notch.width } else { 0.0 };
    match mode {
        // Windows-only super-collapsed strip; keeps a minimal top-edge footprint
        // so full-screen apps stay clickable underneath.
        IslandWindowMode::Micro => {
            let w = compact_width.max(MICRO_WINDOW_WIDTH);
            LogicalSize::new(w, MICRO_WINDOW_HEIGHT)
        }
        // Dormant sits within the menu-bar band. On notched displays the pill
        // spans the notch plus padding on each side; logo is left-aligned inside
        // the pill so it stays in the left wing, not under the camera housing.
        IslandWindowMode::Dormant => {
            let reference_notch = if notch.has_notch {
                notch.width
            } else {
                FALLBACK_NOTCH_WIDTH
            };
            let w = reference_notch + 2.0 * DORMANT_NOTCH_PADDING;
            LogicalSize::new(w, DORMANT_WINDOW_HEIGHT)
        }
        IslandWindowMode::Compact => {
            // Compact sits in the menu-bar band (same as dormant) — no extra_top.
            // On notched displays the capsule must be at least as wide as the
            // camera housing so it visually fuses with it (Dynamic-Island style).
            let w = if notch.has_notch {
                compact_width.max(notch.width)
            } else {
                compact_width
            };
            LogicalSize::new(w, COMPACT_WINDOW_HEIGHT)
        }
        IslandWindowMode::Expanded => {
            let w = EXPANDED_WINDOW_WIDTH.max(min_notch_width);
            LogicalSize::new(w, expanded_window_height(expanded_idle) + extra_top)
        }
    }
}

fn island_window_physical_size(
    mode: IslandWindowMode,
    scale_factor: f64,
    compact_width: f64,
    notch: NotchMetrics,
    expanded_idle: bool,
) -> PhysicalSize<u32> {
    let logical_size = island_window_logical_size(mode, compact_width, notch, expanded_idle);

    PhysicalSize::new(
        (logical_size.width * scale_factor).round() as u32,
        (logical_size.height * scale_factor).round() as u32,
    )
}

fn sanitize_compact_width(width: f64) -> f64 {
    if !width.is_finite() {
        return COMPACT_WINDOW_WIDTH;
    }
    width.clamp(MIN_COMPACT_WINDOW_WIDTH, EXPANDED_WINDOW_WIDTH)
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

    for session in sessions.iter_mut() {
        session.session_host = session_host_for_summary(
            known_sessions,
            &session.session_id,
            &session.cwd,
            &session.agent,
        );
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
            if is_codex_internal_session(
                &request.agent,
                &request.cwd,
                request.transcript_path.as_deref(),
            ) {
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
            if is_codex_internal_session(&info.agent, &info.cwd, info.transcript_path.as_deref()) {
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
        hook_health: HookHealthSnapshot::default(),
    }
}

fn build_session_summaries(visible: &[&PermissionRequest]) -> Vec<SessionSummary> {
    let mut session_map: HashMap<&str, (String, usize, usize, String, Option<String>, AgentKind)> =
        HashMap::new();

    for request in visible {
        if is_codex_internal_session(
            &request.agent,
            &request.cwd,
            request.transcript_path.as_deref(),
        ) {
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
        );

        assert_eq!(snapshot.daily_tokens.input_tokens, 300);
        assert_eq!(snapshot.daily_tokens.output_tokens, 130);
        assert_eq!(snapshot.active_session_tokens.input_tokens, 100);
        assert_eq!(snapshot.active_session_tokens.output_tokens, 50);
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
        );

        assert!(snapshot.sessions.is_empty());
        assert_eq!(snapshot.daily_tokens.input_tokens, 400);
        assert_eq!(snapshot.daily_tokens.output_tokens, 100);
        assert_eq!(snapshot.active_session_tokens.input_tokens, 0);
        assert_eq!(snapshot.active_session_tokens.output_tokens, 0);
    }

    fn test_app_state() -> AppState {
        AppState {
            requests: Mutex::new(Vec::new()),
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
            session_agent_map: Mutex::new(HashMap::new()),
            token_usage_file_offsets: Mutex::new(HashMap::new()),
            token_usage_day: Mutex::new(current_local_day_key()),
            daily_tokens_baseline: Mutex::new(TokenUsage::default()),
            known_sessions: Mutex::new(HashMap::new()),
            pinned_sessions: Mutex::new(HashSet::new()),
            previous_app_pid: Mutex::new(None),
            last_listening_online: Mutex::new(None),
            last_hook_health: Mutex::new(None),
            bridge_port: AtomicU16::new(0),
            active_subagents: Mutex::new(Vec::new()),
            last_subagent_snapshot_emit: Mutex::new(Instant::now() - Duration::from_secs(10)),
        }
    }

    #[test]
    fn rollover_flushes_previous_local_day_before_clearing_usage() {
        use chrono::{Duration, Local};

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
        let live_daily = TokenUsage::default();
        let protected = live_daily.component_wise_max(baseline);
        assert_eq!(protected.input_tokens, 3000);
        assert_eq!(protected.output_tokens, 1200);

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
        );

        assert_eq!(size, LogicalSize::new(560.0, EXPANDED_IDLE_WINDOW_HEIGHT));
    }

    #[test]
    fn tray_contains_only_show_and_quit() {
        assert_eq!(
            tray_menu_entries(),
            [("show", "Show Atoll"), ("quit", "Quit")]
        );
    }

    #[test]
    fn window_animation_interpolates_to_exact_endpoints() {
        assert_eq!(interpolate_u32(132, 560, ease_out_cubic(0.0)), 132);
        assert_eq!(interpolate_u32(132, 560, ease_out_cubic(1.0)), 560);
        assert_eq!(interpolate_f64(100.0, -20.0, ease_out_cubic(0.0)), 100.0);
        assert_eq!(interpolate_f64(100.0, -20.0, ease_out_cubic(1.0)), -20.0);
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
        let compact = island_window_logical_size(IslandWindowMode::Compact, 132.0, notch, false);
        // Compact sits in the menu-bar band (like dormant) — no extra_top.
        assert_eq!(compact.height, COMPACT_WINDOW_HEIGHT);
        // Width is clamped up to the notch width so the capsule visually
        // fuses with the camera housing (Dynamic-Island style).
        assert_eq!(compact.width, 200.0);

        // Content wider than the notch keeps its own width.
        let wide = island_window_logical_size(IslandWindowMode::Compact, 300.0, notch, false);
        assert_eq!(wide.width, 300.0);

        // Dormant is slightly wider than the notch (padding on each side).
        let dormant = island_window_logical_size(IslandWindowMode::Dormant, 132.0, notch, false);
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
        let compact = island_window_logical_size(IslandWindowMode::Compact, 132.0, no_notch, false);
        assert_eq!(compact.width, 132.0);
        assert_eq!(compact.height, COMPACT_WINDOW_HEIGHT);

        // A compact_width that already exceeds the floor is kept as-is.
        let wide = island_window_logical_size(IslandWindowMode::Compact, 250.0, no_notch, false);
        assert_eq!(wide.width, 250.0);

        // Dormant: uses the same FALLBACK_NOTCH_WIDTH reference + padding.
        let dormant = island_window_logical_size(IslandWindowMode::Dormant, 132.0, no_notch, false);
        assert_eq!(
            dormant.width,
            FALLBACK_NOTCH_WIDTH + 2.0 * DORMANT_NOTCH_PADDING
        );
        assert_eq!(dormant.height, DORMANT_WINDOW_HEIGHT);
    }

    #[test]
    fn micro_window_is_a_thin_top_strip() {
        let micro = island_window_logical_size(
            IslandWindowMode::Micro,
            132.0,
            NotchMetrics::default(),
            false,
        );
        assert_eq!(micro.width, 132.0);
        assert_eq!(micro.height, MICRO_WINDOW_HEIGHT);
        let narrow = island_window_logical_size(
            IslandWindowMode::Micro,
            48.0,
            NotchMetrics::default(),
            false,
        );
        assert_eq!(narrow.width, MICRO_WINDOW_WIDTH);
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
    use super::{configured_atoll_hook_script_path, extract_node_script_path};
    use serde_json::json;

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
}

#[cfg(test)]
mod codex_hooks_tests {
    use super::{
        extract_node_script_path, format_hook_command, has_atoll_codex_hooks,
        remove_atoll_codex_hooks, upsert_codex_hook_events,
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
}

#[cfg(test)]
mod claude_hooks_tests {
    use super::{
        format_hook_command, has_atoll_claude_hooks, remove_atoll_claude_hooks,
        upsert_claude_hook_events,
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
}
