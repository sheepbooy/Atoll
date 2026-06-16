use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
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

mod hook_bridge;

#[cfg(target_os = "macos")]
mod panel_store {
    use std::sync::atomic::{AtomicPtr, Ordering};

    static PANEL: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

    pub fn set(ptr: *mut std::ffi::c_void) {
        PANEL.store(ptr, Ordering::Release);
    }

    pub fn get_raw() -> *mut std::ffi::c_void {
        PANEL.load(Ordering::Acquire)
    }
}

const COMPACT_WINDOW_WIDTH: f64 = 132.0;
const COMPACT_WINDOW_HEIGHT: f64 = 28.0;
const EXPANDED_WINDOW_WIDTH: f64 = 560.0;
const EXPANDED_WINDOW_HEIGHT: f64 = 320.0;
const MIN_COMPACT_WINDOW_WIDTH: f64 = 72.0;
// "Super-collapsed" drawer shown when there are no active sessions: a tiny
// handle peeking from the top edge of the screen.
const DORMANT_WINDOW_WIDTH: f64 = 80.0;
const DORMANT_WINDOW_HEIGHT: f64 = 10.0;
const WINDOW_ANIMATION_DURATION: Duration = Duration::from_millis(420);
const WINDOW_ANIMATION_FRAME: Duration = Duration::from_micros(16_667);
// Fallback notch width (logical pt) used when the auxiliary menu-bar areas
// can't be read but a notch height is reported.
const FALLBACK_NOTCH_WIDTH: f64 = 200.0;

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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct TokenUsage {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
}

impl TokenUsage {
    fn add_assign(&mut self, other: TokenUsage) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_read_tokens = self.cache_read_tokens.saturating_add(other.cache_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AgentKind {
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
    Dormant,
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IslandHoverChanged {
    hovering: bool,
}

struct DecisionWithNote {
    decision: Decision,
    note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnownSession {
    agent: AgentKind,
    cwd: String,
    transcript_path: Option<String>,
    last_activity: String,
}

struct AppState {
    requests: Mutex<Vec<PermissionRequest>>,
    hook_waiters: Mutex<HashMap<String, SyncSender<DecisionWithNote>>>,
    auto_approve_sessions: Mutex<HashSet<String>>,
    compact_width: Mutex<f64>,
    presentation_generation: Arc<AtomicU64>,
    home_bounds: Mutex<Option<HomeWindowBounds>>,
    notch_metrics: Mutex<NotchMetrics>,
    session_last_seen: Mutex<HashMap<String, u64>>,
    session_retention_secs: Mutex<u64>,
    session_token_usage: Mutex<HashMap<String, TokenUsage>>,
    token_usage_file_offsets: Mutex<HashMap<String, u64>>,
    token_usage_day: Mutex<String>,
    known_sessions: Mutex<HashMap<String, KnownSession>>,
    /// pid of the app that was frontmost before Atoll grabbed focus for an
    /// approval, so focus can be handed back when the user is done (macOS).
    previous_app_pid: Mutex<Option<i32>>,
}

#[derive(Debug, Clone, Copy)]
struct HomeWindowBounds {
    position: LogicalPosition<f64>,
    compact_size: PhysicalSize<u32>,
    monitor_top_y: f64,
    notch: NotchMetrics,
    #[cfg(target_os = "macos")]
    screen_geometry: Option<MacosScreenGeometry>,
}

/// Camera-housing ("notch") geometry for the display the island lives on, in
/// logical points. On non-notched displays `has_notch` is false and the island
/// keeps its original top-edge layout.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct NotchMetrics {
    has_notch: bool,
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct MacosScreenGeometry {
    origin_y: f64,
    height: f64,
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> IslandSnapshot {
    roll_over_token_usage_if_needed(&state);
    let tracked_sessions = {
        let requests = state.requests.lock().expect("state mutex poisoned");
        let known_sessions = state.known_sessions.lock().expect("state mutex poisoned");
        collect_session_transcript_paths(&requests, &known_sessions)
    };
    for (session_id, transcript_path) in tracked_sessions {
        let _ = refresh_session_token_usage(&state, &session_id, Some(transcript_path.as_str()));
    }

    let requests = state.requests.lock().expect("state mutex poisoned");
    let last_seen = state.session_last_seen.lock().expect("state mutex poisoned");
    let retention = *state.session_retention_secs.lock().expect("state mutex poisoned");
    let token_usage = state
        .session_token_usage
        .lock()
        .expect("state mutex poisoned");
    let known_sessions = state
        .known_sessions
        .lock()
        .expect("state mutex poisoned");
    snapshot_from(&requests, &last_seen, retention, &token_usage, &known_sessions)
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
        });
    }

    touch_session_last_seen(&state, &session_id);
    roll_over_token_usage_if_needed(&state);
    let last_seen = state.session_last_seen.lock().map_err(|error| error.to_string())?;
    let retention = *state.session_retention_secs.lock().map_err(|error| error.to_string())?;
    let token_usage = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    let known_sessions = state
        .known_sessions
        .lock()
        .map_err(|error| error.to_string())?;
    let snapshot = snapshot_from(&requests, &last_seen, retention, &token_usage, &known_sessions);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
async fn set_island_presentation(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: IslandWindowMode,
    compact_width: Option<f64>,
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

    let generation = state.presentation_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let presentation_generation = Arc::clone(&state.presentation_generation);
    let compact_width = *state
        .compact_width
        .lock()
        .map_err(|error| error.to_string())?;
    let home_bounds = *state
        .home_bounds
        .lock()
        .map_err(|error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || {
        animate_island_window_mode(
            &window,
            mode,
            generation,
            &presentation_generation,
            home_bounds,
            compact_width,
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
    let last_seen = state.session_last_seen.lock().map_err(|error| error.to_string())?;
    let retention = *state.session_retention_secs.lock().map_err(|error| error.to_string())?;
    let token_usage = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    let known_sessions = state
        .known_sessions
        .lock()
        .map_err(|error| error.to_string())?;
    let snapshot = snapshot_from(&requests, &last_seen, retention, &token_usage, &known_sessions);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn get_session_requests(
    state: State<'_, AppState>,
    session_id: String,
) -> Vec<PermissionRequest> {
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
}

const TRANSCRIPT_MAX_MESSAGES: usize = 50;

#[tauri::command]
fn get_session_transcript(transcript_path: String) -> Result<Vec<ChatMessage>, String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(&transcript_path)
        .map_err(|e| format!("Cannot open transcript: {e}"))?;
    let reader = BufReader::new(file);

    let mut messages: Vec<ChatMessage> = Vec::new();

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
                    });
                }
            }
            "assistant" => {
                let content = extract_transcript_text(&entry);
                let tool_name = entry
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array)
                    .and_then(|arr| {
                        arr.iter().find_map(|block| {
                            if block.get("type")?.as_str()? == "tool_use" {
                                block.get("name").and_then(Value::as_str).map(String::from)
                            } else {
                                None
                            }
                        })
                    });
                if !content.is_empty() || tool_name.is_some() {
                    messages.push(ChatMessage {
                        role: "assistant".into(),
                        content,
                        tool_name,
                    });
                }
            }
            _ => {}
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
) -> Vec<(String, String)> {
    let mut session_paths: HashMap<String, String> = HashMap::new();
    for request in requests {
        let Some(transcript_path) = request.transcript_path.as_deref() else {
            continue;
        };
        session_paths
            .entry(request.session.clone())
            .or_insert_with(|| transcript_path.to_string());
    }
    for (session_id, known_session) in known_sessions {
        let Some(transcript_path) = known_session.transcript_path.as_deref() else {
            continue;
        };
        session_paths
            .entry(session_id.clone())
            .or_insert_with(|| transcript_path.to_string());
    }
    session_paths.into_iter().collect()
}

fn current_utc_day_key() -> String {
    iso_timestamp_now().chars().take(10).collect()
}

fn roll_over_token_usage_if_needed(state: &AppState) {
    let today = current_utc_day_key();
    let needs_rollover = {
        let mut usage_day = state
            .token_usage_day
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *usage_day == today {
            false
        } else {
            *usage_day = today;
            true
        }
    };

    if !needs_rollover {
        return;
    }

    if let Ok(mut usage_by_session) = state.session_token_usage.lock() {
        usage_by_session.clear();
    }
    if let Ok(mut offsets) = state.token_usage_file_offsets.lock() {
        offsets.clear();
    }
}

fn token_usage_from_transcript_entry(entry: &Value, today_key: &str) -> TokenUsage {
    if entry.get("type").and_then(Value::as_str) != Some("assistant") {
        return TokenUsage::default();
    }

    let Some(timestamp) = entry.get("timestamp").and_then(Value::as_str) else {
        return TokenUsage::default();
    };
    if !timestamp.starts_with(today_key) {
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

fn parse_token_usage_from_transcript(
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

pub(crate) fn refresh_session_token_usage(
    state: &AppState,
    session_id: &str,
    transcript_path: Option<&str>,
) -> Result<(), String> {
    let Some(transcript_path) = transcript_path else {
        return Ok(());
    };

    roll_over_token_usage_if_needed(state);
    let today_key = current_utc_day_key();
    let last_offset = state
        .token_usage_file_offsets
        .lock()
        .map_err(|error| error.to_string())?
        .get(transcript_path)
        .copied()
        .unwrap_or(0);

    let (parsed_usage, next_offset, is_full_scan) =
        parse_token_usage_from_transcript(transcript_path, last_offset, &today_key)?;

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

    Ok(())
}

#[tauri::command]
fn archive_all_resolved(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    // Archive-all should feel immediate in UI: keep only pending requests and
    // drop all resolved/archived items from in-memory list.
    requests.retain(|request| request.status == PermissionStatus::Pending);
    roll_over_token_usage_if_needed(&state);
    let last_seen = state.session_last_seen.lock().map_err(|error| error.to_string())?;
    let retention = *state.session_retention_secs.lock().map_err(|error| error.to_string())?;
    let token_usage = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    let known_sessions = state
        .known_sessions
        .lock()
        .map_err(|error| error.to_string())?;
    let snapshot = snapshot_from(&requests, &last_seen, retention, &token_usage, &known_sessions);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

const DEFAULT_SESSION_RETENTION_SECS: u64 = 300;

#[tauri::command]
fn get_session_retention(state: State<'_, AppState>) -> u64 {
    *state.session_retention_secs.lock().expect("state mutex poisoned")
}

#[tauri::command]
fn set_session_retention(
    state: State<'_, AppState>,
    minutes: u64,
) -> u64 {
    let secs = minutes.clamp(1, 30) * 60;
    let mut retention = state.session_retention_secs.lock().expect("state mutex poisoned");
    *retention = secs;
    secs
}

pub(crate) fn register_known_session(
    state: &AppState,
    session_id: &str,
    agent: AgentKind,
    cwd: &str,
    transcript_path: Option<&str>,
) {
    if let Ok(mut known) = state.known_sessions.lock() {
        let entry = known
            .entry(session_id.to_string())
            .or_insert_with(|| KnownSession {
                agent: agent.clone(),
                cwd: cwd.to_string(),
                transcript_path: transcript_path.map(str::to_string),
                last_activity: iso_timestamp_now(),
            });
        entry.last_activity = iso_timestamp_now();
        if !cwd.is_empty() && cwd != "." {
            entry.cwd = cwd.to_string();
        }
        if let Some(path) = transcript_path {
            entry.transcript_path = Some(path.to_string());
        }
    }
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookStatus {
    installed: bool,
    script_found: bool,
    settings_path: String,
    script_path: String,
}

#[tauri::command]
fn get_claude_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    let script_path = resolve_hook_script_path(&app);
    let script_found = script_path
        .as_ref()
        .map(|p| std::path::Path::new(p).exists())
        .unwrap_or(false);

    let settings_path = claude_settings_path()
        .ok_or_else(|| "Cannot determine home directory".to_string())?;

    let installed = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Cannot read settings: {e}"))?;
        let settings: Value = serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));
        has_atoll_hooks(&settings)
    } else {
        false
    };

    Ok(HookStatus {
        installed,
        script_found,
        settings_path: settings_path.to_string_lossy().into(),
        script_path: script_path.unwrap_or_default(),
    })
}

#[tauri::command]
fn install_claude_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let script_path = resolve_hook_script_path(&app)
        .ok_or_else(|| "Cannot locate hook script".to_string())?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let settings_path = claude_settings_path()
        .ok_or_else(|| "Cannot determine home directory".to_string())?;

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

    let hook_command = format!("node {script_path}");
    let hooks = serde_json::json!({
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
        ]
    });

    settings
        .as_object_mut()
        .ok_or_else(|| "Settings file is not a JSON object".to_string())?
        .insert("hooks".into(), hooks);

    let formatted = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize settings: {e}"))?;
    std::fs::write(&settings_path, formatted)
        .map_err(|e| format!("Cannot write settings: {e}"))?;

    Ok(HookStatus {
        installed: true,
        script_found: true,
        settings_path: settings_path.to_string_lossy().into(),
        script_path,
    })
}

#[tauri::command]
fn uninstall_claude_hooks() -> Result<HookStatus, String> {
    let settings_path = claude_settings_path()
        .ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !settings_path.exists() {
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: settings_path.to_string_lossy().into(),
            script_path: String::new(),
        });
    }

    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot read settings: {e}"))?;
    let mut settings: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    if let Some(obj) = settings.as_object_mut() {
        obj.remove("hooks");
    }

    let formatted = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize settings: {e}"))?;
    std::fs::write(&settings_path, formatted)
        .map_err(|e| format!("Cannot write settings: {e}"))?;

    Ok(HookStatus {
        installed: false,
        script_found: false,
        settings_path: settings_path.to_string_lossy().into(),
        script_path: String::new(),
    })
}

fn claude_settings_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("settings.json"))
}

fn resolve_hook_script_path(app: &AppHandle) -> Option<String> {
    let resource_path = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("scripts").join("atoll-claude-hook.mjs"));
    if let Some(ref path) = resource_path {
        if path.exists() {
            return Some(path.to_string_lossy().into());
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().skip(1) {
            let candidate = ancestor.join("scripts").join("atoll-claude-hook.mjs");
            if candidate.exists() {
                return Some(candidate.to_string_lossy().into());
            }
            if ancestor.join("src-tauri").exists() {
                return Some(candidate.to_string_lossy().into());
            }
        }
    }

    None
}

fn has_atoll_hooks(settings: &Value) -> bool {
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

#[tauri::command]
fn open_in_terminal(cwd: String) -> Result<(), String> {
    use std::process::Command;

    if let Some(app) = detect_terminal_app_for_cwd(&cwd) {
        Command::new("open")
            .arg("-a")
            .arg(&app)
            .spawn()
            .map_err(|e| format!("Failed to activate {}: {}", app, e))?;
    } else {
        Command::new("open")
            .arg("-a")
            .arg("Terminal")
            .arg(&cwd)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }
    Ok(())
}

fn detect_terminal_app_for_cwd(cwd: &str) -> Option<String> {
    use std::process::Command;

    let output = Command::new("lsof")
        .args(["-d", "cwd", "+c", "0"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);

    let mut pids: Vec<u32> = Vec::new();
    for line in text.lines().skip(1) {
        if line.contains(cwd) {
            let pid_str = line.split_whitespace().nth(1)?;
            if let Ok(pid) = pid_str.parse::<u32>() {
                pids.push(pid);
            }
        }
    }

    for pid in pids {
        if let Some(app) = find_terminal_ancestor(pid) {
            return Some(app);
        }
    }
    None
}

const KNOWN_TERMINALS: &[(&str, &str)] = &[
    ("ghostty", "Ghostty"),
    ("Ghostty", "Ghostty"),
    ("iTerm2", "iTerm2"),
    ("iTerm2-Server", "iTerm2"),
    ("Terminal", "Terminal"),
    ("kitty", "kitty"),
    ("alacritty", "Alacritty"),
    ("Alacritty", "Alacritty"),
    ("wezterm-gui", "WezTerm"),
    ("WezTerm", "WezTerm"),
    ("Hyper", "Hyper"),
    ("tabby", "Tabby"),
    ("rio", "Rio"),
];

fn find_terminal_ancestor(mut pid: u32) -> Option<String> {
    use std::process::Command;

    for _ in 0..20 {
        if pid <= 1 {
            return None;
        }
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "ppid=,comm="])
            .output()
            .ok()?;
        let line = String::from_utf8_lossy(&output.stdout);
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let ppid_str = parts.next()?.trim();
        let comm = parts.next()?.trim();

        let basename = comm.rsplit('/').next().unwrap_or(comm);
        for &(pattern, app_name) in KNOWN_TERMINALS {
            if basename == pattern {
                return Some(app_name.to_string());
            }
        }

        pid = ppid_str.parse::<u32>().ok()?;
    }
    None
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    use std::process::Command;
    Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Failed to open URL: {}", e))?;
    Ok(())
}

#[tauri::command]
fn quit_atoll(app: AppHandle) {
    exit_atoll(&app);
}

/// Record the currently frontmost application so focus can be restored to it
/// once Atoll is done with an approval interaction.
#[cfg(target_os = "macos")]
fn remember_frontmost_app(app: &AppHandle) {
    let own_pid = std::process::id() as i32;
    unsafe {
        let Some(ws_class) = objc2::runtime::AnyClass::get(c"NSWorkspace") else {
            return;
        };
        let workspace: *mut objc2::runtime::AnyObject =
            objc2::msg_send![ws_class, sharedWorkspace];
        if workspace.is_null() {
            return;
        }
        let front: *mut objc2::runtime::AnyObject =
            objc2::msg_send![workspace, frontmostApplication];
        if front.is_null() {
            return;
        }
        let pid: i32 = objc2::msg_send![front, processIdentifier];
        if pid <= 0 || pid == own_pid {
            return;
        }
        if let Ok(mut guard) = app.state::<AppState>().previous_app_pid.lock() {
            *guard = Some(pid);
        }
    }
}

/// Activate the app with the given pid (used to hand focus back). Returns
/// whether activation was issued successfully.
#[cfg(target_os = "macos")]
unsafe fn activate_app_by_pid(pid: i32) -> bool {
    let Some(cls) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else {
        return false;
    };
    let running: *mut objc2::runtime::AnyObject =
        objc2::msg_send![cls, runningApplicationWithProcessIdentifier: pid];
    if running.is_null() {
        return false;
    }
    // NSApplicationActivateIgnoringOtherApps = 1 << 1
    let options: usize = 1 << 1;
    let ok: objc2::runtime::Bool = objc2::msg_send![running, activateWithOptions: options];
    ok.as_bool()
}

#[tauri::command]
fn deactivate_atoll(state: State<'_, AppState>) {
    #[cfg(target_os = "macos")]
    {
        let previous = state
            .previous_app_pid
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        unsafe {
            // Prefer handing focus back to the app the user came from.
            if let Some(pid) = previous {
                if activate_app_by_pid(pid) {
                    return;
                }
            }
            // Fallback: just resign our own activation.
            let cls =
                objc2::runtime::AnyClass::get(c"NSApplication").expect("NSApplication class");
            let ns_app: *mut objc2::runtime::AnyObject =
                objc2::msg_send![cls, sharedApplication];
            if !ns_app.is_null() {
                let _: () = objc2::msg_send![ns_app, deactivate];
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = state;
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            requests: Mutex::new(Vec::new()),
            hook_waiters: Mutex::new(HashMap::new()),
            auto_approve_sessions: Mutex::new(HashSet::new()),
            compact_width: Mutex::new(COMPACT_WINDOW_WIDTH),
            presentation_generation: Arc::new(AtomicU64::new(0)),
            home_bounds: Mutex::new(None),
            notch_metrics: Mutex::new(NotchMetrics::default()),
            session_last_seen: Mutex::new(HashMap::new()),
            session_retention_secs: Mutex::new(DEFAULT_SESSION_RETENTION_SECS),
            session_token_usage: Mutex::new(HashMap::new()),
            token_usage_file_offsets: Mutex::new(HashMap::new()),
            token_usage_day: Mutex::new(current_utc_day_key()),
            known_sessions: Mutex::new(HashMap::new()),
            previous_app_pid: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_session_requests,
            get_session_transcript,
            resolve_permission_request,
            set_session_auto_approve,
            archive_request,
            archive_all_resolved,
            set_island_presentation,
            get_notch_metrics,
            get_claude_hook_status,
            install_claude_hooks,
            uninstall_claude_hooks,
            get_session_retention,
            set_session_retention,
            open_in_terminal,
            open_url,
            quit_atoll,
            deactivate_atoll
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            hook_bridge::start_server(app.handle().clone());
            start_island_hover_monitor(app.handle().clone());
            start_auto_archive_timer(app.handle().clone());
            start_token_refresh_timer(app.handle().clone());

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_shadow(false);
                let _ = window.set_skip_taskbar(true);
                let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
                let _ = window.show();
                // Apply island style AFTER show() so the window number is
                // assigned and the NSPanel promotion takes effect.
                apply_macos_island_window_style(&window);
                eprintln!("[Atoll] step: island style applied, now applying mode...");
                if let Ok(Some(home)) =
                    apply_island_window_mode(&window, IslandWindowMode::Compact, COMPACT_WINDOW_WIDTH)
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

    TrayIconBuilder::new()
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
        })
        .build(app)?;

    Ok(())
}

fn tray_menu_entries() -> [(&'static str, &'static str); 2] {
    [("show", "Show Atoll"), ("quit", "Quit")]
}

const AUTO_ARCHIVE_INTERVAL: Duration = Duration::from_secs(10);
const AUTO_ARCHIVE_AGE: Duration = Duration::from_secs(60);
const TOKEN_REFRESH_INTERVAL: Duration = Duration::from_millis(900);

fn start_auto_archive_timer(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(AUTO_ARCHIVE_INTERVAL);

        let state = app.state::<AppState>();
        let changed = {
            let Ok(mut requests) = state.requests.lock() else {
                continue;
            };

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut changed = false;
            for request in requests.iter_mut() {
                if request.archived || request.status == PermissionStatus::Pending {
                    continue;
                }
                let requested_at_secs = parse_iso_timestamp_secs(&request.requested_at);
                if now.saturating_sub(requested_at_secs) >= AUTO_ARCHIVE_AGE.as_secs() {
                    request.archived = true;
                    changed = true;
                }
            }

            if changed {
                roll_over_token_usage_if_needed(&state);
                let last_seen = state.session_last_seen.lock().unwrap_or_else(|e| e.into_inner());
                let retention = *state.session_retention_secs.lock().unwrap_or_else(|e| e.into_inner());
                let token_usage = state
                    .session_token_usage
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let known_sessions = state
                    .known_sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let snapshot = snapshot_from(&requests, &last_seen, retention, &token_usage, &known_sessions);
                let _ = app.emit("snapshot-changed", &snapshot);
            }
            changed
        };
        let _ = changed;
    });
}

fn start_token_refresh_timer(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(TOKEN_REFRESH_INTERVAL);

        let state = app.state::<AppState>();
        let tracked_sessions = {
            let requests = state.requests.lock().unwrap_or_else(|e| e.into_inner());
            let known_sessions = state.known_sessions.lock().unwrap_or_else(|e| e.into_inner());
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

        for (session_id, transcript_path) in tracked_sessions {
            if let Err(error) =
                refresh_session_token_usage(&state, &session_id, Some(transcript_path.as_str()))
            {
                eprintln!("Atoll token usage refresh failed: {error}");
            }
        }

        let usage_after = state
            .session_token_usage
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if usage_after == usage_before {
            // Avoid noisy snapshot events when token usage has not changed.
            continue;
        }

        roll_over_token_usage_if_needed(&state);
        let requests = state.requests.lock().unwrap_or_else(|e| e.into_inner());
        let last_seen = state.session_last_seen.lock().unwrap_or_else(|e| e.into_inner());
        let retention = *state.session_retention_secs.lock().unwrap_or_else(|e| e.into_inner());
        let token_usage = state
            .session_token_usage
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let known_sessions = state.known_sessions.lock().unwrap_or_else(|e| e.into_inner());
        let snapshot = snapshot_from(&requests, &last_seen, retention, &token_usage, &known_sessions);
        let _ = app.emit("snapshot-changed", &snapshot);
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
        let window_for_main_thread = window.clone();
        #[cfg(target_os = "macos")]
        let app_for_focus = app.clone();
        // Permission hooks arrive on a background thread; AppKit window APIs
        // must run on the main thread to avoid macOS crashes.
        let _ = window.run_on_main_thread(move || {
            let _ = window_for_main_thread.show();
            if request_focus {
                // Capture who was frontmost *before* we steal focus so we can
                // restore it after the user resolves the approval.
                #[cfg(target_os = "macos")]
                remember_frontmost_app(&app_for_focus);
                let _ = window_for_main_thread.set_focus();
            }
            #[cfg(target_os = "macos")]
            {
                let panel_ptr = panel_store::get_raw();
                if !panel_ptr.is_null() {
                    unsafe {
                        let panel_ptr = panel_ptr as *mut objc2::runtime::AnyObject;
                        let _: () = objc2::msg_send![
                            panel_ptr,
                            orderFrontRegardless
                        ];
                        if request_focus {
                            if let Some(ns_app_class) = objc2::runtime::AnyClass::get(c"NSApplication") {
                                let ns_app: *mut objc2::runtime::AnyObject =
                                    objc2::msg_send![ns_app_class, sharedApplication];
                                if !ns_app.is_null() {
                                    let _: () = objc2::msg_send![
                                        ns_app,
                                        activateIgnoringOtherApps: objc2::runtime::Bool::YES
                                    ];
                                }
                            }
                            let _: () = objc2::msg_send![
                                panel_ptr,
                                makeKeyAndOrderFront: std::ptr::null_mut::<objc2::runtime::AnyObject>()
                            ];
                        }
                    }
                }
            }
        });
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

        loop {
            thread::sleep(Duration::from_millis(80));

            let hovering = app
                .get_webview_window("main")
                .and_then(|window| is_cursor_over_window(&window).ok())
                .unwrap_or(false);

            if hovering == last_hovering {
                continue;
            }

            last_hovering = hovering;
            let _ = app.emit("island-hover-changed", IslandHoverChanged { hovering });
        }
    });
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

fn apply_island_window_mode(
    window: &tauri::WebviewWindow,
    mode: IslandWindowMode,
    compact_width: f64,
) -> tauri::Result<Option<HomeWindowBounds>> {
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(None);
    };

    apply_macos_island_window_style(window);
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));

    let scale_factor = monitor.scale_factor();
    let monitor_position = monitor.position().to_logical::<f64>(scale_factor);
    let monitor_size = monitor.size().to_logical::<f64>(scale_factor);
    let notch = detect_notch_metrics(window, monitor_position.x, monitor_size.width);

    window.set_size(island_window_logical_size(mode, compact_width, notch))?;
    set_island_cursor_events_ignored(window, matches!(
        mode,
        IslandWindowMode::Compact | IslandWindowMode::Dormant
    ));

    let window_size = island_window_physical_size(mode, scale_factor, compact_width, notch);
    let logical_window_size = window_size.to_logical::<f64>(scale_factor);
    let centered_x = monitor_position.x + (monitor_size.width - logical_window_size.width) / 2.0;
    // Keep the window flush with the physical top edge so the capsule overlaps
    // the notch / menu-bar band; the actual content is pushed below the notch
    // height inside the web view. On non-notched screens this is unchanged.
    let centered_y = monitor_position.y;
    let position = LogicalPosition::new(centered_x, centered_y);
    let home = HomeWindowBounds {
        position,
        // Un-notched compact size keeps the animation's scale-factor recovery
        // (compact_size.width / COMPACT_WINDOW_WIDTH) correct.
        compact_size: island_window_physical_size(
            IslandWindowMode::Compact,
            scale_factor,
            COMPACT_WINDOW_WIDTH,
            NotchMetrics::default(),
        ),
        monitor_top_y: monitor_position.y,
        notch,
        #[cfg(target_os = "macos")]
        screen_geometry: macos_screen_geometry(window, monitor_position.x, monitor_size.width),
    };

    set_island_window_frame_now(window, position, window_size, scale_factor, home)?;
    Ok(Some(home))
}

fn animate_island_window_mode(
    window: &tauri::WebviewWindow,
    mode: IslandWindowMode,
    generation: u64,
    presentation_generation: &AtomicU64,
    home_bounds: Option<HomeWindowBounds>,
    compact_width: f64,
) -> tauri::Result<()> {
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    if matches!(mode, IslandWindowMode::Expanded) {
        set_island_cursor_events_ignored(window, false);
    }

    let scale_factor = home_bounds
        .map(|home| home.compact_size.width as f64 / COMPACT_WINDOW_WIDTH)
        .unwrap_or_else(|| window.scale_factor().unwrap_or(1.0));
    let start_position = window.outer_position()?.to_logical::<f64>(scale_factor);
    let start_size = window.outer_size()?;
    let notch = home_bounds.map(|home| home.notch).unwrap_or_default();
    let target_size = island_window_physical_size(mode, scale_factor, compact_width, notch);
    let target_logical_size = target_size.to_logical::<f64>(scale_factor);
    // Center every mode on the same axis as the compact capsule (which may be
    // widened to straddle the notch).
    let compact_logical_width =
        island_window_logical_size(IslandWindowMode::Compact, compact_width, notch).width;
    let (target_x, target_y) = home_bounds
        .map(|home| {
            (
                home.position.x + (compact_logical_width - target_logical_size.width) / 2.0,
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

        set_island_window_frame(window, position, size, scale_factor, home_bounds)?;

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

    set_island_cursor_events_ignored(window, matches!(
        mode,
        IslandWindowMode::Compact | IslandWindowMode::Dormant
    ));
    Ok(())
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

fn island_window_logical_size(
    mode: IslandWindowMode,
    compact_width: f64,
    notch: NotchMetrics,
) -> LogicalSize<f64> {
    let compact_width = sanitize_compact_width(compact_width);
    // Reserve the notch height at the top of the window so the content can sit
    // *below* the camera housing (the cutout itself has no pixels). The width
    // keeps following the content so the capsule grows and shrinks with the
    // number of agents instead of being forced to the notch width.
    let extra_top = if notch.has_notch { notch.height } else { 0.0 };
    match mode {
        IslandWindowMode::Dormant => {
            LogicalSize::new(DORMANT_WINDOW_WIDTH, DORMANT_WINDOW_HEIGHT + extra_top)
        }
        IslandWindowMode::Compact => {
            LogicalSize::new(compact_width, COMPACT_WINDOW_HEIGHT + extra_top)
        }
        IslandWindowMode::Expanded => {
            LogicalSize::new(EXPANDED_WINDOW_WIDTH, EXPANDED_WINDOW_HEIGHT + extra_top)
        }
    }
}

fn island_window_physical_size(
    mode: IslandWindowMode,
    scale_factor: f64,
    compact_width: f64,
    notch: NotchMetrics,
) -> PhysicalSize<u32> {
    let logical_size = island_window_logical_size(mode, compact_width, notch);

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
fn has_camera_housing(frame_width: f64, aux_left_width: f64, aux_right_width: f64) -> bool {
    aux_left_width > 0.0
        && aux_right_width > 0.0
        && aux_left_width + aux_right_width < frame_width - 1.0
}

/// Notch width in logical points, derived from the gap between the auxiliary
/// menu-bar areas (matches ping-island's detection). Falls back when the
/// auxiliary areas are unavailable.
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

#[cfg(target_os = "macos")]
fn with_nsscreen_for_monitor<R>(
    window: &tauri::WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
    inspect: impl FnOnce(&objc2_app_kit::NSScreen) -> R,
) -> Option<R> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSScreen, NSWindow};

    if let Some(main_thread_marker) = MainThreadMarker::new() {
        let screens = NSScreen::screens(main_thread_marker);
        if let Some(screen) = screens.iter().find(|screen| {
            let frame = screen.frame();
            (frame.origin.x - monitor_x).abs() < 1.0
                && (frame.size.width - monitor_width).abs() < 1.0
        }) {
            return Some(inspect(&screen));
        }
    }

    let ns_window = window.ns_window().ok()?;
    if ns_window.is_null() {
        return None;
    }

    unsafe {
        let ns_window = &*(ns_window.cast::<NSWindow>());
        ns_window.screen().map(|screen| inspect(&screen))
    }
}

#[cfg(target_os = "macos")]
fn macos_screen_geometry(
    window: &tauri::WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
) -> Option<MacosScreenGeometry> {
    with_nsscreen_for_monitor(window, monitor_x, monitor_width, |screen| {
        let frame = screen.frame();
        MacosScreenGeometry {
            origin_y: frame.origin.y,
            height: frame.size.height,
        }
    })
}

#[cfg(target_os = "macos")]
fn detect_notch_metrics(
    window: &tauri::WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
) -> NotchMetrics {
    with_nsscreen_for_monitor(window, monitor_x, monitor_width, |screen| {
        let safe_top = screen.safeAreaInsets().top;
        if safe_top <= 0.0 {
            return NotchMetrics::default();
        }
        let frame = screen.frame();
        // `auxiliaryTop*Area` returns NSZeroRect (width 0) when there is no
        // notch / menu bar on this screen.
        let aux_left_width = screen.auxiliaryTopLeftArea().size.width;
        let aux_right_width = screen.auxiliaryTopRightArea().size.width;
        if !has_camera_housing(frame.size.width, aux_left_width, aux_right_width) {
            return NotchMetrics::default();
        }
        NotchMetrics {
            has_notch: true,
            width: notch_logical_width(
                frame.size.width,
                aux_left_width,
                aux_right_width,
                FALLBACK_NOTCH_WIDTH,
            ),
            height: safe_top.ceil(),
        }
    })
    .unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn detect_notch_metrics(
    _window: &tauri::WebviewWindow,
    _monitor_x: f64,
    _monitor_width: f64,
) -> NotchMetrics {
    NotchMetrics::default()
}

fn set_island_cursor_events_ignored(window: &tauri::WebviewWindow, ignore: bool) {
    #[cfg(target_os = "macos")]
    {
        let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            // setIgnoresMouseEvents: MUST run on the main thread.
            // animate_island_window_mode calls us from a tokio worker,
            // so dispatch via run_on_main_thread.
            let ptr_val = panel_ptr as usize;
            let _ = window.run_on_main_thread(move || unsafe {
                use objc2::runtime::{AnyObject, Bool};
                let ptr = ptr_val as *mut AnyObject;
                let val = if ignore { Bool::YES } else { Bool::NO };
                let _: () = objc2::msg_send![ptr, setIgnoresMouseEvents: val];
            });
            return;
        }
    }
    let _ = window.set_ignore_cursor_events(ignore);
}

#[cfg(target_os = "macos")]
fn set_island_window_frame_now(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    home: HomeWindowBounds,
) -> tauri::Result<()> {
    use objc2_app_kit::NSWindow;

    let Some(screen_geometry) = home.screen_geometry else {
        window.set_size(size)?;
        return window.set_position(position);
    };
    let ns_window = window.ns_window()?;
    if ns_window.is_null() {
        return Ok(());
    }

    let logical_size = size.to_logical::<f64>(scale_factor);
    let origin_y = appkit_window_origin_y(
        screen_geometry.origin_y,
        screen_geometry.height,
        logical_size.height,
        position.y,
        home.monitor_top_y,
    );

    unsafe {
        let ns_window = &*(ns_window.cast::<NSWindow>());
        let mut frame = ns_window.frame();
        frame.origin.x = position.x;
        frame.origin.y = origin_y;
        frame.size.width = logical_size.width;
        frame.size.height = logical_size.height;

        // Keep the Tauri window frame in sync (for position/size queries).
        ns_window.setFrame_display(frame, true);

        let height_progress = ((logical_size.height - COMPACT_WINDOW_HEIGHT)
            / (EXPANDED_WINDOW_HEIGHT - COMPACT_WINDOW_HEIGHT))
            .clamp(0.0, 1.0);
        let corner_radius = 15.0 + 7.0 * height_progress;

        // If a floating panel exists, also update its frame and apply
        // the corner mask there (that's where the WKWebView lives).
        let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            let panel = &*(panel_ptr as *const NSWindow);
            panel.setFrame_display(frame, true);
            apply_content_view_corner_mask(panel, corner_radius);
        } else {
            apply_content_view_corner_mask(ns_window, corner_radius);
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set_island_window_frame_now(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    _scale_factor: f64,
    _home: HomeWindowBounds,
) -> tauri::Result<()> {
    window.set_size(size)?;
    window.set_position(position)
}

#[cfg(target_os = "macos")]
fn set_island_window_frame(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    home: Option<HomeWindowBounds>,
) -> tauri::Result<()> {
    let Some(home) = home else {
        window.set_size(size)?;
        return window.set_position(position);
    };

    let frame_window = window.clone();
    window.run_on_main_thread(move || {
        let _ = set_island_window_frame_now(&frame_window, position, size, scale_factor, home);
    })?;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set_island_window_frame(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    _scale_factor: f64,
    _home: Option<HomeWindowBounds>,
) -> tauri::Result<()> {
    window.set_size(size)?;
    window.set_position(position)
}

fn appkit_window_origin_y(
    screen_origin_y: f64,
    screen_height: f64,
    window_height: f64,
    desired_top_y: f64,
    monitor_top_y: f64,
) -> f64 {
    screen_origin_y + screen_height - (desired_top_y - monitor_top_y) - window_height
}

#[cfg(target_os = "macos")]
fn apply_macos_island_window_style(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{
        NSColor, NSMainMenuWindowLevel, NSWindow, NSWindowAnimationBehavior,
        NSWindowCollectionBehavior,
    };

    let Ok(ns_window) = window.ns_window() else {
        return;
    };
    if ns_window.is_null() {
        return;
    }

    unsafe {
        let ns_window = &*(ns_window.cast::<NSWindow>());
        promote_to_floating_panel(ns_window);
        eprintln!("[Atoll] step: promote_to_floating_panel done");
        apply_macos_unconstrained_window_class(ns_window);
        eprintln!("[Atoll] step: unconstrained_window_class done");
        apply_accepts_first_mouse(ns_window);
        eprintln!("[Atoll] step: accepts_first_mouse done");
        let clear = NSColor::clearColor();
        ns_window.setOpaque(false);
        ns_window.setBackgroundColor(Some(&clear));
        ns_window.setHasShadow(false);
        ns_window.setMovable(false);
        ns_window.setMovableByWindowBackground(false);
        ns_window.setCanHide(false);
        ns_window.setAnimationBehavior(NSWindowAnimationBehavior::None);
        ns_window.setAllowsToolTipsWhenApplicationIsInactive(true);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns_window.setLevel(NSMainMenuWindowLevel + 3);
        eprintln!("[Atoll] step: window properties set");

        // Corner mask goes on the panel (where the WKWebView lives)
        // if it exists, otherwise on the Tauri window as fallback.
        let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            apply_content_view_corner_mask(&*(panel_ptr as *const NSWindow), 15.0);
        } else {
            apply_content_view_corner_mask(ns_window, 15.0);
        }
        eprintln!("[Atoll] step: apply_macos_island_window_style complete");
    }
}

/// Create a real NSPanel (properly initialised as a floating panel that
/// renders above the macOS menu bar), then move the WKWebView from the
/// Tauri window into this panel.  The Tauri window keeps an empty
/// contentView so tao's internal bookkeeping doesn't crash, and all
/// frame / mouse-event updates target the panel via `panel_store`.
#[cfg(target_os = "macos")]
fn promote_to_floating_panel(ns_window: &objc2_app_kit::NSWindow) {
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
    use objc2::sel;
    use objc2_app_kit::{
        NSColor, NSMainMenuWindowLevel, NSScreen, NSWindow, NSWindowCollectionBehavior,
        NSWindowStyleMask,
    };
    use objc2_foundation::NSRect;

    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| unsafe {
        let panel_cls = AnyClass::get(c"NSPanel").expect("NSPanel class");
        let frame = ns_window.frame();

        let raw: *mut AnyObject = objc2::msg_send![panel_cls, alloc];
        let style_bits: usize =
            NSWindowStyleMask::Borderless.0 as usize | (1usize << 7);
        let raw: *mut AnyObject = objc2::msg_send![
            raw,
            initWithContentRect: frame,
            styleMask: style_bits,
            backing: 2usize,
            defer: Bool::NO
        ];
        assert!(!raw.is_null(), "NSPanel init failed");

        let _: () = objc2::msg_send![raw, setFloatingPanel: Bool::YES];
        let _: () = objc2::msg_send![raw, setHidesOnDeactivate: Bool::NO];
        let _: () = objc2::msg_send![raw, setOpaque: Bool::NO];
        let clear = NSColor::clearColor();
        let _: () = objc2::msg_send![raw, setBackgroundColor: &*clear];
        let _: () = objc2::msg_send![raw, setHasShadow: Bool::NO];
        let _: () = objc2::msg_send![raw, setMovable: Bool::NO];
        let _: () = objc2::msg_send![raw, setLevel: NSMainMenuWindowLevel + 3];
        let _: () = objc2::msg_send![raw, setCollectionBehavior:
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle
        ];

        // Patch NSPanel's constrainFrameRect:toScreen: so the panel
        // is never clamped below the menu bar.
        extern "C-unwind" fn unconstrained_panel(
            _w: *mut NSWindow,
            _s: Sel,
            f: NSRect,
            _scr: *mut NSScreen,
        ) -> NSRect {
            f
        }
        let panel_class = (&*raw).class();
        let constrain_sel = sel!(constrainFrameRect:toScreen:);
        if let Some(m) = panel_class.instance_method(constrain_sel) {
            let imp: Imp = std::mem::transmute(
                unconstrained_panel
                    as extern "C-unwind" fn(*mut NSWindow, Sel, NSRect, *mut NSScreen) -> NSRect,
            );
            objc2::ffi::class_replaceMethod(
                panel_class as *const AnyClass as *mut AnyClass,
                constrain_sel,
                imp,
                objc2::ffi::method_getTypeEncoding(m),
            );
        }

        // A borderless non-activating NSPanel reports canBecomeKeyWindow == NO
        // by default, which silently swallows makeKeyAndOrderFront and leaves
        // the WKWebView unable to receive keyboard input. Force it to YES so
        // approval shortcuts work whenever we explicitly request focus.
        extern "C-unwind" fn always_yes(_w: *mut NSWindow, _s: Sel) -> Bool {
            Bool::YES
        }
        for key_sel in [sel!(canBecomeKeyWindow), sel!(canBecomeMainWindow)] {
            if let Some(m) = panel_class.instance_method(key_sel) {
                let imp: Imp = std::mem::transmute(
                    always_yes as extern "C-unwind" fn(*mut NSWindow, Sel) -> Bool,
                );
                objc2::ffi::class_replaceMethod(
                    panel_class as *const AnyClass as *mut AnyClass,
                    key_sel,
                    imp,
                    objc2::ffi::method_getTypeEncoding(m),
                );
            }
        }

        // ── Move the WKWebView from the Tauri window into the panel ──
        // We use addSubview: which automatically removes the view from
        // its old superview.  Crucially we do NOT replace the Tauri
        // window's contentView — tao keeps an internal reference to it
        // and replacing it causes a crash on mouse events.
        let content_view: *mut AnyObject = objc2::msg_send![ns_window, contentView];
        if !content_view.is_null() {
            let subviews: *mut AnyObject = objc2::msg_send![content_view, subviews];
            let count: usize = objc2::msg_send![subviews, count];
            if count > 0 {
                let wk: *mut AnyObject =
                    objc2::msg_send![subviews, objectAtIndex: 0usize];

                // addSubview: on the panel's contentView automatically
                // removes `wk` from the Tauri window's contentView.
                let pcv: *mut AnyObject = objc2::msg_send![raw, contentView];
                let _: () = objc2::msg_send![pcv, addSubview: wk];
                let bounds: NSRect = objc2::msg_send![pcv, bounds];
                let _: () = objc2::msg_send![wk, setFrame: bounds];
                // NSViewWidthSizable(2) | NSViewHeightSizable(16) = 18
                let _: () = objc2::msg_send![wk, setAutoresizingMask: 18usize];

                eprintln!("[Atoll] WKWebView moved to floating panel");
            }
        }

        // The Tauri window is now content-less; keep it permanently
        // ignoring mouse events so it never blocks the panel.
        let _: () = objc2::msg_send![ns_window, setIgnoresMouseEvents: Bool::YES];

        // Panel starts with ignoresMouseEvents=YES (compact mode).
        // The mode system will toggle this via set_island_cursor_events_ignored.
        let _: () = objc2::msg_send![raw, setIgnoresMouseEvents: Bool::YES];
        let _: () = objc2::msg_send![raw, orderFrontRegardless];

        panel_store::set(raw as *mut std::ffi::c_void);

        let is_floating: Bool = objc2::msg_send![raw, isFloatingPanel];
        eprintln!(
            "[Atoll] floating panel ready, floating={}, level={}",
            is_floating.as_bool(),
            { let lvl: isize = objc2::msg_send![raw, level]; lvl },
        );
    });
}

#[cfg(target_os = "macos")]
fn apply_accepts_first_mouse(ns_window: &objc2_app_kit::NSWindow) {
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};

    extern "C-unwind" fn always_accepts(
        _view: *mut AnyObject,
        _sel: Sel,
        _event: *mut AnyObject,
    ) -> bool {
        true
    }

    unsafe fn patch_view_class(view: *mut AnyObject) {
        if view.is_null() {
            return;
        }
        let class = (&*view).class();
        let selector = objc2::sel!(acceptsFirstMouse:);
        let Some(method) = class.instance_method(selector) else {
            return;
        };
        let implementation: Imp = std::mem::transmute(
            always_accepts as extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> bool,
        );
        objc2::ffi::class_replaceMethod(
            class as *const AnyClass as *mut AnyClass,
            selector,
            implementation,
            objc2::ffi::method_getTypeEncoding(method),
        );
    }

    static VIEW_PATCHED: OnceLock<()> = OnceLock::new();
    VIEW_PATCHED.get_or_init(|| unsafe {
        // Patch the Tauri window's contentView.
        let cv: *mut AnyObject = objc2::msg_send![ns_window, contentView];
        patch_view_class(cv);

        // Also patch the floating panel's views (contentView + WKWebView).
        let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            let pcv: *mut AnyObject =
                objc2::msg_send![panel_ptr as *mut AnyObject, contentView];
            patch_view_class(pcv);
            if !pcv.is_null() {
                let subviews: *mut AnyObject = objc2::msg_send![pcv, subviews];
                let count: usize = objc2::msg_send![subviews, count];
                for i in 0..count {
                    let sv: *mut AnyObject =
                        objc2::msg_send![subviews, objectAtIndex: i];
                    patch_view_class(sv);
                }
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn apply_macos_unconstrained_window_class(ns_window: &objc2_app_kit::NSWindow) {
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, Imp, Sel};
    use objc2::sel;
    use objc2_app_kit::{NSScreen, NSWindow};
    use objc2_foundation::NSRect;

    extern "C-unwind" fn unconstrained_frame(
        _window: *mut NSWindow,
        _selector: Sel,
        frame: NSRect,
        _screen: *mut NSScreen,
    ) -> NSRect {
        frame
    }

    static WINDOW_CLASS_PATCHED: OnceLock<()> = OnceLock::new();
    WINDOW_CLASS_PATCHED.get_or_init(|| {
        let selector = sel!(constrainFrameRect:toScreen:);
        let class = ns_window.class();
        let method = class
            .instance_method(selector)
            .expect("NSWindow constrainFrameRect:toScreen: should exist");
        unsafe {
            let implementation: Imp = std::mem::transmute(
                unconstrained_frame
                    as extern "C-unwind" fn(*mut NSWindow, Sel, NSRect, *mut NSScreen) -> NSRect,
            );
            objc2::ffi::class_replaceMethod(
                class as *const AnyClass as *mut AnyClass,
                selector,
                implementation,
                objc2::ffi::method_getTypeEncoding(method),
            );
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn apply_macos_island_window_style(_window: &tauri::WebviewWindow) {}

#[cfg(target_os = "macos")]
unsafe fn apply_content_view_corner_mask(ns_window: &objc2_app_kit::NSWindow, radius: f64) {
    use objc2::runtime::AnyObject;

    let cv: *mut AnyObject = objc2::msg_send![ns_window, contentView];
    if cv.is_null() {
        return;
    }
    let _: () = objc2::msg_send![cv, setWantsLayer: true];
    let layer: *mut AnyObject = objc2::msg_send![cv, layer];
    if layer.is_null() {
        return;
    }
    let _: () = objc2::msg_send![layer, setCornerRadius: radius];
    let _: () = objc2::msg_send![layer, setMasksToBounds: true];
    // kCALayerMinXMinYCorner(1) | kCALayerMaxXMinYCorner(2) = bottom corners in CG coords
    let _: () = objc2::msg_send![layer, setMaskedCorners: 3_usize];
}

fn snapshot_from(
    requests: &[PermissionRequest],
    session_last_seen: &HashMap<String, u64>,
    retention_secs: u64,
    session_token_usage: &HashMap<String, TokenUsage>,
    known_sessions: &HashMap<String, KnownSession>,
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
            let last_seen_ts = session_last_seen
                .get(&request.session)
                .copied()
                .unwrap_or_else(|| parse_iso_timestamp_secs(&request.requested_at));
            if now.saturating_sub(last_seen_ts) > retention_secs {
                continue;
            }
            let entry = retained_map
                .entry(&request.session)
                .or_insert_with(|| {
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
            sessions.push(SessionSummary {
                session_id: session_id.to_string(),
                agent,
                cwd,
                pending_count: 0,
                total_count: 0,
                last_activity,
                transcript_path,
            });
        }

        sessions.sort_by(|a, b| {
            b.pending_count
                .cmp(&a.pending_count)
                .then(b.last_activity.cmp(&a.last_activity))
        });
    }

    // Include known sessions (from Stop/PostToolUse events) that have no
    // permission requests – these are sessions with only text output.
    {
        let existing_ids: HashSet<String> =
            sessions.iter().map(|s| s.session_id.clone()).collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for (session_id, info) in known_sessions {
            if existing_ids.contains(session_id.as_str()) {
                continue;
            }
            if retention_secs > 0 {
                let last_seen_ts = session_last_seen
                    .get(session_id)
                    .copied()
                    .unwrap_or_else(|| parse_iso_timestamp_secs(&info.last_activity));
                if now.saturating_sub(last_seen_ts) > retention_secs {
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
            });
        }

        sessions.sort_by(|a, b| {
            b.pending_count
                .cmp(&a.pending_count)
                .then(b.last_activity.cmp(&a.last_activity))
        });
    }

    let mut daily_tokens = TokenUsage::default();
    for usage in session_token_usage.values() {
        daily_tokens.add_assign(*usage);
    }

    IslandSnapshot {
        online: true,
        pending_count,
        archived_count,
        active_request,
        recent: visible.into_iter().take(12).cloned().collect(),
        sessions,
        daily_tokens,
    }
}

fn build_session_summaries(visible: &[&PermissionRequest]) -> Vec<SessionSummary> {
    let mut session_map: HashMap<&str, (String, usize, usize, String, Option<String>, AgentKind)> =
        HashMap::new();

    for request in visible {
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
            },
        )
        .collect();

    summaries.sort_by(|a, b| b.pending_count.cmp(&a.pending_count).then(b.last_activity.cmp(&a.last_activity)));
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
    fn expanded_window_is_560_by_320() {
        let size = island_window_logical_size(
            IslandWindowMode::Expanded,
            COMPACT_WINDOW_WIDTH,
            NotchMetrics::default(),
        );

        assert_eq!(size, LogicalSize::new(560.0, 320.0));
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
    fn notched_display_reserves_top_inset_without_widening() {
        let notch = NotchMetrics {
            has_notch: true,
            width: 200.0,
            height: 38.0,
        };
        let compact = island_window_logical_size(IslandWindowMode::Compact, 132.0, notch);
        // Height grows by the notch height so content can sit below the cutout.
        assert_eq!(compact.height, COMPACT_WINDOW_HEIGHT + 38.0);
        // Width keeps following the content (the capsule stays centered under
        // the notch) instead of being inflated to the notch width.
        assert_eq!(compact.width, 132.0);
    }

    #[test]
    fn non_notched_display_keeps_original_compact_size() {
        let compact = island_window_logical_size(
            IslandWindowMode::Compact,
            132.0,
            NotchMetrics::default(),
        );
        assert_eq!(compact, LogicalSize::new(132.0, COMPACT_WINDOW_HEIGHT));
    }

    #[test]
    fn appkit_frame_places_the_window_at_the_screen_top() {
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

        let completed_id =
            crate::hook_bridge::mark_matching_pending_request_complete(&mut requests, &payload);

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
        }];

        let payload = json!({
            "session_id": "session-123",
            "hook_event_name": "PostToolUse"
        });

        let completed_id =
            crate::hook_bridge::mark_matching_pending_request_complete(&mut requests, &payload);

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
            },
        ];

        let payload = json!({
            "session_id": "session-123",
            "hook_event_name": "PostToolUse"
        });

        let completed_id =
            crate::hook_bridge::mark_matching_pending_request_complete(&mut requests, &payload);

        assert_eq!(completed_id.as_deref(), Some("request-newer"));
        assert_eq!(requests[0].status, crate::PermissionStatus::Approved);
        assert_eq!(requests[1].status, crate::PermissionStatus::Pending);
    }

    #[test]
    fn encodes_hook_decision_for_claude_hook_event() {
        let approved = crate::hook_bridge::claude_hook_response(
            "PermissionRequest",
            crate::Decision::Approved,
            "",
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

        let denied =
            crate::hook_bridge::claude_hook_response("PermissionRequest", crate::Decision::Denied, "");
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
        let denied = crate::hook_bridge::claude_hook_response(
            "PermissionRequest",
            crate::Decision::Denied,
            "Please use a safer command",
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
        let approved =
            crate::hook_bridge::claude_hook_response("PreToolUse", crate::Decision::Approved, "");
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
    fn encodes_permission_request_ask_as_empty_response() {
        let ask =
            crate::hook_bridge::claude_hook_ask_response("PermissionRequest", "Atoll unavailable");

        assert_eq!(ask, json!({}));
    }

    #[test]
    fn encodes_pre_tool_use_ask_response() {
        let ask = crate::hook_bridge::claude_hook_ask_response("PreToolUse", "Atoll unavailable");

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
