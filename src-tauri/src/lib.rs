use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
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

mod capture;
mod debug_agent;
mod hook_bridge;
mod hook_trust;
mod local_time;
mod platform;
mod pricing;
mod token_history;
mod transcript;

const COMPACT_WINDOW_WIDTH: f64 = 132.0;
pub(crate) const COMPACT_WINDOW_HEIGHT: f64 = 36.0;
/// Windows-only super-collapsed strip; macOS never selects this mode.
pub(crate) const MICRO_WINDOW_WIDTH: f64 = 72.0;
pub(crate) const MICRO_WINDOW_HEIGHT: f64 = 24.0;
const EXPANDED_WINDOW_WIDTH: f64 = 560.0;
pub(crate) const EXPANDED_WINDOW_HEIGHT: f64 = 320.0;
const EXPANDED_IDLE_WINDOW_HEIGHT: f64 = 240.0;
pub(crate) const EXPANDED_PLAN_WINDOW_WIDTH: f64 = 680.0;
pub(crate) const EXPANDED_PLAN_WINDOW_HEIGHT: f64 = 680.0;
pub(crate) const EXPANDED_SETTINGS_WINDOW_WIDTH: f64 = 680.0;
pub(crate) const EXPANDED_SETTINGS_WINDOW_HEIGHT: f64 = 680.0;
const MIN_COMPACT_WINDOW_WIDTH: f64 = 72.0;
// Dormant pill height (width spans the notch + side padding on notched displays).
const DORMANT_WINDOW_HEIGHT: f64 = 36.0;
// Extra width beyond the notch on each side so edges are visible.
const DORMANT_NOTCH_PADDING: f64 = 30.0;
const MAX_ACTIVE_SUBAGENTS: usize = 512;
const WINDOW_ANIMATION_DURATION: Duration = Duration::from_millis(420);
const WINDOW_ANIMATION_FRAME: Duration = Duration::from_micros(16_667);

pub(crate) fn lock_state<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
pub(crate) static TOKEN_HISTORY_ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) static PRICING_ENV_LOCK: Mutex<()> = Mutex::new(());
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
    #[serde(default)]
    daily_tokens_by_model: HashMap<String, TokenUsage>,
    #[serde(default)]
    active_session_tokens_by_model: HashMap<String, TokenUsage>,
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
    /// Cursor subagent's independent conversation_id (bound on first preToolUse).
    conversation_id: Option<String>,
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
    fn is_zero(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.cache_read_tokens == 0
            && self.cache_creation_tokens == 0
    }

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

/// Merge live session totals with the startup floor without double-counting.
///
/// Before any transcript full-scan, in-memory session values are incremental
/// (hooks since process start) and are added to `startup_floor`. After a
/// full-scan, that session's value is absolute for today; we take
/// `max(startup_floor, sum(absolute sessions)) + sum(incremental sessions)`.
pub(crate) fn effective_daily_tokens(
    session_token_usage: &HashMap<String, TokenUsage>,
    startup_floor: TokenUsage,
    absolute_sessions: &HashSet<String>,
) -> TokenUsage {
    let mut absolute_sum = TokenUsage::default();
    let mut incremental_sum = TokenUsage::default();

    for (session_id, usage) in session_token_usage {
        if absolute_sessions.contains(session_id) {
            absolute_sum.add_assign(*usage);
        } else {
            incremental_sum.add_assign(*usage);
        }
    }

    if absolute_sessions.is_empty() {
        let mut total = startup_floor;
        total.add_assign(incremental_sum);
        return total;
    }

    let mut total = startup_floor.component_wise_max(absolute_sum);
    total.add_assign(incremental_sum);
    total
}

/// Same restart-floor semantics as [`effective_daily_tokens`], keyed by model.
pub(crate) fn effective_daily_tokens_by_model(
    session_usage_by_model: &HashMap<String, HashMap<String, TokenUsage>>,
    startup_floor: &HashMap<String, TokenUsage>,
    absolute_sessions: &HashSet<String>,
) -> HashMap<String, TokenUsage> {
    let mut absolute_sum: HashMap<String, TokenUsage> = HashMap::new();
    let mut incremental_sum: HashMap<String, TokenUsage> = HashMap::new();

    for (session_id, usage_by_model) in session_usage_by_model {
        let target = if absolute_sessions.contains(session_id) {
            &mut absolute_sum
        } else {
            &mut incremental_sum
        };
        for (model_id, usage) in usage_by_model {
            target
                .entry(model_id.clone())
                .or_default()
                .add_assign(*usage);
        }
    }

    if absolute_sessions.is_empty() {
        let mut total = startup_floor.clone();
        for (model_id, usage) in incremental_sum {
            total.entry(model_id).or_default().add_assign(usage);
        }
        return total;
    }

    let mut total = startup_floor.clone();
    for (model_id, usage) in absolute_sum {
        let entry = total.entry(model_id).or_default();
        *entry = entry.component_wise_max(usage);
    }
    for (model_id, usage) in incremental_sum {
        total.entry(model_id).or_default().add_assign(usage);
    }
    total
}

fn merge_session_model_usage(
    target: &mut HashMap<String, TokenUsage>,
    source: &HashMap<String, TokenUsage>,
    is_full_scan: bool,
) {
    for (model_id, usage) in source {
        let entry = target.entry(model_id.clone()).or_default();
        if is_full_scan {
            *entry = entry.component_wise_max(*usage);
        } else {
            entry.add_assign(*usage);
        }
    }
}

fn token_usage_from_delta(delta: transcript::TokenUsageDelta) -> TokenUsage {
    TokenUsage {
        input_tokens: delta.input_tokens,
        output_tokens: delta.output_tokens,
        cache_read_tokens: delta.cache_read_tokens,
        cache_creation_tokens: delta.cache_creation_tokens,
    }
}

fn token_usage_map_from_delta_map(
    source: &HashMap<String, transcript::TokenUsageDelta>,
) -> HashMap<String, TokenUsage> {
    source
        .iter()
        .map(|(model_id, delta)| (model_id.clone(), token_usage_from_delta(*delta)))
        .collect()
}

fn aggregate_usage_by_model(
    session_usage_by_model: &HashMap<String, HashMap<String, TokenUsage>>,
    session_filter: Option<&HashSet<&str>>,
) -> HashMap<String, TokenUsage> {
    let mut totals = HashMap::new();
    for (session_id, usage_by_model) in session_usage_by_model {
        if let Some(filter) = session_filter {
            if !filter.contains(session_id.as_str()) {
                continue;
            }
        }
        for (model_id, usage) in usage_by_model {
            totals
                .entry(model_id.clone())
                .or_insert(TokenUsage::default())
                .add_assign(*usage);
        }
    }
    totals
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AgentKind {
    Claude,
    Codex,
    Cursor,
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
    cursor_over_window: bool,
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
    /// Full Cursor composer UUID when the session key is a short hook id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conversation_id: Option<String>,
}

/// Shared application state.
///
/// Lock ordering for code that must hold more than one mutex at a time:
/// requests -> known_sessions -> pinned_sessions -> session/token maps ->
/// hook_waiters -> active_subagents -> UI/window metrics. Prefer cloning the
/// minimum data and dropping each guard before acquiring the next lock.
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
    session_token_usage_by_model: Mutex<HashMap<String, HashMap<String, TokenUsage>>>,
    /// Sticky session → agent mapping that survives session purges within a day.
    session_agent_map: Mutex<HashMap<String, String>>,
    token_usage_file_offsets: Mutex<HashMap<String, u64>>,
    token_usage_day: Mutex<String>,
    /// Today's persisted total loaded at process start (and after midnight rollover).
    /// Hook increments add on top until transcript full-scans produce absolute totals.
    startup_daily_floor: Mutex<TokenUsage>,
    /// Today's persisted per-model totals loaded at process start (cost-mode floor).
    startup_daily_floor_by_model: Mutex<HashMap<String, TokenUsage>>,
    /// Sessions whose in-memory totals came from a transcript full-scan (absolute).
    absolute_token_sessions: Mutex<HashSet<String>>,
    /// High-water mark synced to token_history.json; never regresses within a day.
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
    /// Per-process bearer token shared with local hook runners through bridge.json.
    bridge_auth_token: Mutex<String>,
    /// Last time the hook bridge accepted a TCP probe; used for offline grace during rebind.
    last_bridge_reachable: Mutex<Option<Instant>>,
    active_subagents: Mutex<Vec<ActiveSubagent>>,
    /// Maps Cursor subagent conversation_id → parent session_id.
    cursor_subagent_conversations: Mutex<HashMap<String, String>>,
    /// Cursor sessions that already produced token usage from lifecycle hooks.
    cursor_lifecycle_token_sessions: Mutex<HashSet<String>>,
    /// Rate-limiter for SubagentStart/SubagentStop snapshot emissions.
    last_subagent_snapshot_emit: Mutex<Instant>,
    /// Debounce generation for Cursor observer snapshot emits.
    snapshot_debounce_generation: AtomicU64,
    /// Rate-limiter for subagent transcript reconciliation in build_snapshot.
    last_subagent_reconcile: Mutex<Instant>,
    /// Last hook HTTP activity; used to back off token refresh when idle.
    last_hook_activity: Mutex<Instant>,
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
    refresh_hook_health_cache(&app, &state);
    let tracked_sessions = {
        let requests = lock_state(&state.requests);
        let known_sessions = lock_state(&state.known_sessions);
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
    let cursor_ready = cursor_hook_status(app);
    let any_installed = claude_ready.installed || codex_ready.installed || cursor_ready.installed;
    let any_script_found =
        claude_ready.script_found || codex_ready.script_found || cursor_ready.script_found;
    any_installed && any_script_found && hook_bridge::is_bridge_online(app)
}

pub(crate) fn touch_hook_activity(state: &AppState) {
    if let Ok(mut last) = state.last_hook_activity.lock() {
        *last = Instant::now();
    }
}

pub(crate) fn get_stored_session_host(state: &AppState, session_id: &str) -> platform::SessionHost {
    state
        .known_sessions
        .lock()
        .ok()
        .and_then(|known| known.get(session_id).map(|entry| entry.host))
        .unwrap_or(platform::SessionHost::Unknown)
}

pub(crate) fn schedule_observer_snapshot_emit(app: &AppHandle) {
    let state = app.state::<AppState>();
    let generation = state
        .snapshot_debounce_generation
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(OBSERVER_SNAPSHOT_DEBOUNCE);
        let state = app.state::<AppState>();
        if state.snapshot_debounce_generation.load(Ordering::SeqCst) != generation {
            return;
        }
        let snapshot = build_snapshot(&app, &state);
        let _ = app.emit("snapshot-changed", &snapshot);
    });
}

pub(crate) fn refresh_hook_health_cache(app: &AppHandle, state: &AppState) {
    let health = build_hook_health(app);
    remember_hook_health(state, &health);
}

fn cached_hook_health(app: &AppHandle, state: &AppState) -> HookHealthSnapshot {
    if let Ok(last) = state.last_hook_health.lock() {
        if let Some(cached) = last.as_ref() {
            return cached.clone();
        }
    }
    let health = build_hook_health(app);
    remember_hook_health(state, &health);
    health
}

pub(crate) fn reconcile_incomplete_subagents_now(state: &AppState) {
    reconcile_incomplete_subagents(state);
    if let Ok(mut last) = state.last_subagent_reconcile.lock() {
        *last = Instant::now();
    }
}

fn reconcile_incomplete_subagents_if_due(state: &AppState) {
    let should_run = {
        let mut last = state
            .last_subagent_reconcile
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let now = Instant::now();
        if now.duration_since(*last) < SUBAGENT_RECONCILE_MIN_INTERVAL {
            false
        } else {
            *last = now;
            true
        }
    };
    if should_run {
        reconcile_incomplete_subagents(state);
    }
}

pub(crate) fn build_snapshot(app: &AppHandle, state: &AppState) -> IslandSnapshot {
    prune_active_subagents(state);
    roll_over_token_usage_if_needed(state);
    reconcile_incomplete_subagents_if_due(state);
    backfill_cursor_session_metadata(state);
    let requests = lock_state(&state.requests);
    let last_seen = lock_state(&state.session_last_seen);
    let retention = *lock_state(&state.session_retention_secs);
    let token_usage = lock_state(&state.session_token_usage);
    let token_usage_by_model = lock_state(&state.session_token_usage_by_model);
    let known_sessions = lock_state(&state.known_sessions);
    let pinned = lock_state(&state.pinned_sessions);
    let online = compute_listening_online(app);
    let hook_health = cached_hook_health(app, state);
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
    // Scope token aggregation locks: sync_today_to_history below re-acquires
    // absolute_token_sessions; holding the guard here deadlocks the main thread.
    {
        let startup_floor = *state
            .startup_daily_floor
            .lock()
            .expect("state mutex poisoned");
        let startup_floor_by_model = state
            .startup_daily_floor_by_model
            .lock()
            .expect("state mutex poisoned")
            .clone();
        let absolute_sessions = state
            .absolute_token_sessions
            .lock()
            .expect("state mutex poisoned");
        snapshot.daily_tokens =
            effective_daily_tokens(&token_usage, startup_floor, &absolute_sessions);
        snapshot.daily_tokens_by_model = effective_daily_tokens_by_model(
            &token_usage_by_model,
            &startup_floor_by_model,
            &absolute_sessions,
        );
        let active_ids: HashSet<&str> = snapshot
            .sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect();
        snapshot.active_session_tokens_by_model =
            aggregate_usage_by_model(&token_usage_by_model, Some(&active_ids));
    }
    drop(token_usage_by_model);
    drop(token_usage);
    drop(last_seen);
    drop(requests);
    let subagent_retention = *lock_state(&state.subagent_retention_secs);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let active_subagents = lock_state(&state.active_subagents);
    assign_active_subagents_to_sessions(
        &mut snapshot.sessions,
        &active_subagents,
        subagent_retention,
        now_secs,
    );
    drop(active_subagents);
    persist_session_hosts(state, &snapshot.sessions);
    snapshot.hook_health = hook_health;
    let _ = token_history::sync_today_to_history(state);

    snapshot
}

fn active_subagent_visible(
    subagent: &ActiveSubagent,
    subagent_retention: u64,
    now_secs: u64,
) -> bool {
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
}

fn prune_active_subagents(state: &AppState) {
    let subagent_retention = *lock_state(&state.subagent_retention_secs);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut removed_conversations = Vec::new();

    {
        let mut subagents = lock_state(&state.active_subagents);
        subagents.retain(|subagent| {
            let keep = active_subagent_visible(subagent, subagent_retention, now_secs)
                || subagent.completed_at.is_none();
            if !keep {
                if let Some(conversation_id) = subagent.conversation_id.clone() {
                    removed_conversations.push(conversation_id);
                }
            }
            keep
        });

        let mut overflow = subagents.len().saturating_sub(MAX_ACTIVE_SUBAGENTS);
        if overflow > 0 {
            subagents.retain(|subagent| {
                let removable = subagent.archived || subagent.completed_at.is_some();
                if overflow > 0 && removable {
                    overflow -= 1;
                    if let Some(conversation_id) = subagent.conversation_id.clone() {
                        removed_conversations.push(conversation_id);
                    }
                    return false;
                }
                true
            });
        }
    }

    if !removed_conversations.is_empty() {
        if let Ok(mut map) = state.cursor_subagent_conversations.lock() {
            for conversation_id in removed_conversations {
                map.remove(&conversation_id);
            }
        }
    }
}

fn subagent_summary_from_active(subagent: &ActiveSubagent) -> SubagentSummary {
    SubagentSummary {
        agent_id: subagent.agent_id.clone(),
        agent_type: subagent.agent_type.clone(),
        started_at: subagent.started_at.clone(),
        agent_transcript_path: subagent.agent_transcript_path.clone(),
        completed_at: subagent.completed_at.clone(),
        archived: subagent.archived,
        last_message: subagent.last_message.clone(),
    }
}

fn assign_active_subagents_to_sessions(
    sessions: &mut [SessionSummary],
    active_subagents: &[ActiveSubagent],
    subagent_retention: u64,
    now_secs: u64,
) {
    let mut subagents_by_session: HashMap<String, Vec<SubagentSummary>> = HashMap::new();
    for subagent in active_subagents.iter() {
        if !active_subagent_visible(subagent, subagent_retention, now_secs) {
            continue;
        }
        subagents_by_session
            .entry(subagent.session_id.clone())
            .or_default()
            .push(subagent_summary_from_active(subagent));
    }
    for session in sessions.iter_mut() {
        session.active_subagents = subagents_by_session
            .remove(&session.session_id)
            .unwrap_or_default();
    }
}

fn persist_session_hosts(state: &AppState, sessions: &[SessionSummary]) {
    for session in sessions {
        if matches!(
            session.session_host,
            platform::SessionHost::ClaudeDesktop
                | platform::SessionHost::ClaudeCli
                | platform::SessionHost::CodexDesktop
                | platform::SessionHost::CodexCli
                | platform::SessionHost::CursorIde
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
        let cursor_script_path =
            resolve_hook_script_path(app, "atoll-cursor-hook.mjs").unwrap_or_default();
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
                needs_retrust: false,
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
                needs_retrust: false,
            },
            cursor: HookStatus {
                installed: false,
                script_found: !cursor_script_path.is_empty()
                    && std::path::Path::new(&cursor_script_path).exists(),
                settings_path: cursor_hooks_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                script_path: cursor_script_path,
                node_path: String::new(),
                node_found: resolve_node_executable().is_ok(),
                needs_retrust: false,
            },
        };
    }

    let claude_status = claude_hook_status(app);
    let codex_status = codex_hook_status(app);
    let cursor_status = cursor_hook_status(app);

    // #region agent log (diagA)
    crate::debug_agent::log(
        "H-F",
        "lib.rs:build_hook_health",
        "hook health snapshot",
        json!({
            "online": compute_listening_online(app),
            "bridgeReachable": hook_bridge::is_bridge_reachable(app),
            "claude": {
                "installed": claude_status.installed,
                "scriptFound": claude_status.script_found,
                "scriptPath": claude_status.script_path,
                "nodeFound": claude_status.node_found,
                "nodePath": claude_status.node_path,
            },
            "codex": {
                "installed": codex_status.installed,
                "scriptFound": codex_status.script_found,
                "scriptPath": codex_status.script_path,
                "nodeFound": codex_status.node_found,
                "nodePath": codex_status.node_path,
            },
            "cursor": {
                "installed": cursor_status.installed,
                "scriptFound": cursor_status.script_found,
                "scriptPath": cursor_status.script_path,
                "nodeFound": cursor_status.node_found,
                "nodePath": cursor_status.node_path,
            },
        }),
    );
    // #endregion

    HookHealthSnapshot {
        claude: claude_status,
        codex: codex_status,
        cursor: cursor_status,
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
        "claude",
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
    if installed {
        refresh_deployed_hook_assets_if_needed(app, "atoll-codex-hook.mjs");
    }
    let (mut script_path, mut script_found) =
        resolve_hook_script_readiness(app, "atoll-codex-hook.mjs", config.as_ref());
    if installed {
        #[cfg(windows)]
        maybe_repair_hook_launcher_config(app, "atoll-codex-hook.mjs", "codex-hook-launcher.json");
        if let (Some(cfg), Ok(preferred)) = (
            config.as_ref(),
            resolve_install_hook_script_path(app, "atoll-codex-hook.mjs"),
        ) {
            if let Some(configured) = configured_atoll_hook_script_path(cfg, "atoll-codex-hook") {
                if should_flag_dev_hook_drift(&configured, &preferred)
                    && deployed_hook_script_path("atoll-codex-hook.mjs").is_none()
                {
                    script_found = false;
                }
            }
        }
    }
    script_path = canonical_hook_script_path(
        app,
        "atoll-codex-hook.mjs",
        config.as_ref(),
        "atoll-codex-hook",
        &script_path,
    );
    if !script_path.is_empty() && std::path::Path::new(&script_path).is_file() {
        script_found = true;
    }
    build_hook_status(
        installed,
        script_found,
        hooks_path,
        script_path,
        config.as_ref(),
        "atoll-codex-hook",
        "codex",
    )
}

fn cursor_hook_status(app: &AppHandle) -> HookStatus {
    let hooks_path = cursor_hooks_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut config = read_json_file(&hooks_path);
    let installed = config
        .as_ref()
        .map(|hooks| has_atoll_cursor_hooks(hooks))
        .unwrap_or(false);
    if installed {
        if let Some(repaired) = maybe_repair_cursor_hook_events(
            app,
            &hooks_path,
            config.as_ref(),
            &hook_bridge::cursor_hook_url_for_app(app),
        ) {
            config = Some(repaired);
        }
        refresh_deployed_hook_assets_if_needed(app, "atoll-cursor-hook.mjs");
    }
    let (mut script_path, mut script_found) =
        resolve_hook_script_readiness(app, "atoll-cursor-hook.mjs", config.as_ref());
    if installed {
        #[cfg(windows)]
        maybe_repair_hook_launcher_config(
            app,
            "atoll-cursor-hook.mjs",
            "cursor-hook-launcher.json",
        );
        if let (Some(cfg), Ok(preferred)) = (
            config.as_ref(),
            resolve_install_hook_script_path(app, "atoll-cursor-hook.mjs"),
        ) {
            if let Some(configured) = configured_atoll_hook_script_path(cfg, "atoll-cursor-hook") {
                if should_flag_dev_hook_drift(&configured, &preferred)
                    && deployed_hook_script_path("atoll-cursor-hook.mjs").is_none()
                {
                    script_found = false;
                }
            }
        }
    }
    script_path = canonical_hook_script_path(
        app,
        "atoll-cursor-hook.mjs",
        config.as_ref(),
        "atoll-cursor-hook",
        &script_path,
    );
    if !script_path.is_empty() && std::path::Path::new(&script_path).is_file() {
        script_found = true;
    }
    build_hook_status(
        installed,
        script_found,
        hooks_path,
        script_path,
        config.as_ref(),
        "atoll-cursor-hook",
        "cursor",
    )
}

fn is_dev_hook_script_path(path: &str) -> bool {
    path.contains("/target/debug/")
        || path.contains("/target/release/")
        || path.contains("/src-tauri/target/")
}

/// Compare two hook script paths in a separator- and prefix-agnostic way.
/// `configured` arrives from hooks.json with forward slashes (written by
/// `normalize_hook_command_path`), while `preferred` comes from
/// `dunce::simplified(PathBuf)`. On Windows `dunce::simplified` keeps the
/// `\\?\` verbatim prefix for paths containing non-ASCII characters (e.g. a user
/// home directory like `C:\Users\杨帅`), so `preferred` can look like
/// `\\?\C:\Users\杨帅\...\atoll-codex-hook.mjs` while `configured` is
/// `C:/Users/杨帅/...\atoll-codex-hook.mjs`. A naive `!=` (or even `Path` equality,
/// which treats verbatim and drive prefixes as distinct components) would always
/// report them as different and falsely flag dev-path drift, flipping
/// `script_found` to false. Normalizing both sides by stripping the verbatim
/// prefix and unifying separators makes the same file compare equal.
fn dev_hook_paths_differ(configured: &str, preferred: &str) -> bool {
    fn normalize(p: &str) -> String {
        let stripped = p.strip_prefix(r"\\?\").unwrap_or(p);
        stripped.replace('\\', "/")
    }
    normalize(configured) != normalize(preferred)
}

/// True when hooks.json still points at a stale dev build path that no longer exists,
/// while the running app bundle exposes a valid replacement script.
fn should_flag_dev_hook_drift(configured: &str, preferred: &str) -> bool {
    if !is_dev_hook_script_path(configured) {
        return false;
    }
    if !dev_hook_paths_differ(configured, preferred) {
        return false;
    }
    if !std::path::Path::new(preferred).is_file() {
        return false;
    }
    // Configured path still works for the hook host — not drift.
    if std::path::Path::new(configured).is_file() {
        return false;
    }
    true
}

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
    expanded_plan: Option<bool>,
    expanded_settings: Option<bool>,
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
    let expanded_plan = expanded_plan.unwrap_or(false);
    let expanded_settings = expanded_settings.unwrap_or(false);

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
            expanded_plan,
            expanded_settings,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn get_notch_metrics(state: State<'_, AppState>) -> NotchMetrics {
    *lock_state(&state.notch_metrics)
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
    let requests = lock_state(&state.requests);
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
const TRANSCRIPT_EXTENSIONS: &[&str] = &["jsonl", "json"];

fn has_parent_dir_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn has_transcript_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            TRANSCRIPT_EXTENSIONS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false)
}

fn canonicalize_requested_transcript_path(transcript_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(transcript_path);
    if !path.is_absolute() {
        return Err("Transcript path must be absolute".into());
    }
    if has_parent_dir_component(path) {
        return Err("Transcript path cannot contain parent directory components".into());
    }
    if !has_transcript_extension(path) {
        return Err("Transcript path must point to a transcript file".into());
    }

    let canonical = dunce::canonicalize(path)
        .map_err(|error| format!("Cannot resolve transcript path: {error}"))?;
    if !canonical.is_file() {
        return Err("Transcript path must point to a file".into());
    }
    if !has_transcript_extension(&canonical) {
        return Err("Transcript path must point to a transcript file".into());
    }
    Ok(canonical)
}

fn collect_trusted_transcript_path_strings(state: &AppState) -> Vec<String> {
    let mut paths = Vec::new();

    if let Ok(requests) = state.requests.lock() {
        paths.extend(
            requests
                .iter()
                .filter_map(|request| request.transcript_path.clone()),
        );
    }

    if let Ok(known_sessions) = state.known_sessions.lock() {
        paths.extend(
            known_sessions
                .values()
                .filter_map(|session| session.transcript_path.clone()),
        );
    }

    if let Ok(active_subagents) = state.active_subagents.lock() {
        paths.extend(
            active_subagents
                .iter()
                .filter_map(|subagent| subagent.agent_transcript_path.clone()),
        );
    }

    paths
}

fn trusted_transcript_paths(state: &AppState) -> HashSet<PathBuf> {
    collect_trusted_transcript_path_strings(state)
        .into_iter()
        .filter_map(|path| canonicalize_requested_transcript_path(&path).ok())
        .collect()
}

fn validate_trusted_transcript_path(
    state: &AppState,
    transcript_path: &str,
) -> Result<PathBuf, String> {
    let canonical = canonicalize_requested_transcript_path(transcript_path)?;
    if trusted_transcript_paths(state).contains(&canonical) {
        return Ok(canonical);
    }
    Err("Transcript path is not associated with a known session".into())
}

fn push_transcript_message(messages: &mut VecDeque<ChatMessage>, message: ChatMessage) {
    messages.push_back(message);
    if messages.len() > TRANSCRIPT_MAX_MESSAGES {
        messages.pop_front();
    }
}

fn read_transcript_messages(transcript_path: &str) -> Result<Vec<ChatMessage>, String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let format = transcript::detect_transcript_format(transcript_path);
    let file = File::open(transcript_path).map_err(|e| format!("Cannot open transcript: {e}"))?;
    let reader = BufReader::new(file);

    let mut messages: VecDeque<ChatMessage> = VecDeque::new();

    if format == transcript::TranscriptFormat::Codex {
        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read error: {e}"))?;
            if let Some(parsed) = transcript::parse_codex_message_line(&line) {
                push_transcript_message(
                    &mut messages,
                    ChatMessage {
                        role: parsed.role,
                        content: parsed.content,
                        tool_name: parsed.tool_name,
                        tool_input: None,
                    },
                );
            }
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

            if let Some(msg_type) = entry.get("type").and_then(Value::as_str) {
                match msg_type {
                    "human" | "user" => {
                        let content = extract_transcript_text(&entry);
                        if !content.is_empty() {
                            push_transcript_message(
                                &mut messages,
                                ChatMessage {
                                    role: "user".into(),
                                    content,
                                    tool_name: None,
                                    tool_input: None,
                                },
                            );
                        }
                    }
                    "assistant" => {
                        let content = extract_transcript_text(&entry);
                        let (tool_name, tool_input) = extract_tool_use_from_entry(&entry);
                        if !content.is_empty() || tool_name.is_some() {
                            push_transcript_message(
                                &mut messages,
                                ChatMessage {
                                    role: "assistant".into(),
                                    content,
                                    tool_name,
                                    tool_input,
                                },
                            );
                        }
                    }
                    _ => {}
                }
                continue;
            }

            // Cursor transcripts: top-level "role" instead of "type"
            if let Some(role) = entry.get("role").and_then(Value::as_str) {
                match role {
                    "user" => {
                        let content = extract_transcript_text(&entry);
                        if !content.is_empty() {
                            push_transcript_message(
                                &mut messages,
                                ChatMessage {
                                    role: "user".into(),
                                    content,
                                    tool_name: None,
                                    tool_input: None,
                                },
                            );
                        }
                    }
                    "assistant" => {
                        let content = extract_transcript_text(&entry);
                        let (tool_name, tool_input) = extract_tool_use_from_entry(&entry);
                        if !content.is_empty() || tool_name.is_some() {
                            push_transcript_message(
                                &mut messages,
                                ChatMessage {
                                    role: "assistant".into(),
                                    content,
                                    tool_name,
                                    tool_input,
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(messages.into_iter().collect())
}

/// Resolve a session's transcript file, checking known state, requests, and on-disk discovery.
pub(crate) fn resolve_session_transcript_path(
    state: &AppState,
    session_id: &str,
    requests: &[PermissionRequest],
) -> Option<String> {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if let Some(path) = entry.transcript_path.clone() {
                if std::path::Path::new(&path).is_file() {
                    return Some(path);
                }
            }
            if let Some(ref conv_id) = entry.conversation_id {
                if let Some((path, _)) = discover_cursor_agent_transcript(conv_id) {
                    return Some(path);
                }
            }
        }
    }

    for request in requests {
        if request.session == session_id {
            if let Some(path) = request.transcript_path.clone() {
                if std::path::Path::new(&path).is_file() {
                    return Some(path);
                }
            }
        }
    }

    discover_cursor_agent_transcript(session_id).map(|(path, _)| path)
}

fn resolve_session_transcript_path_from_snapshot(
    known_sessions: &HashMap<String, KnownSession>,
    requests: &[PermissionRequest],
    session_id: &str,
    agent: &AgentKind,
) -> Option<String> {
    if let Some(entry) = known_sessions.get(session_id) {
        if let Some(path) = entry.transcript_path.clone() {
            return Some(path);
        }
        if matches!(agent, AgentKind::Cursor) {
            if let Some(ref conv_id) = entry.conversation_id {
                if let Some((path, _)) = discover_cursor_agent_transcript(conv_id) {
                    return Some(path);
                }
            }
        }
    }

    for request in requests {
        if !request.archived && request.session == session_id {
            if let Some(path) = request.transcript_path.clone() {
                return Some(path);
            }
        }
    }

    if matches!(agent, AgentKind::Cursor) {
        discover_cursor_agent_transcript(session_id).map(|(path, _)| path)
    } else {
        None
    }
}

fn persist_session_transcript_path(state: &AppState, session_id: &str, path: &str) {
    if let Ok(mut known) = state.known_sessions.lock() {
        if let Some(entry) = known.get_mut(session_id) {
            entry.transcript_path = Some(path.to_string());
            if entry.conversation_id.is_none() {
                if let Some(stem) = std::path::Path::new(path)
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|name| name.to_str())
                {
                    entry.conversation_id = Some(stem.to_string());
                }
            }
        }
    }
}

#[tauri::command]
fn get_session_transcript(
    state: State<'_, AppState>,
    transcript_path: String,
) -> Result<Vec<ChatMessage>, String> {
    let canonical = validate_trusted_transcript_path(&state, &transcript_path)?;
    read_transcript_messages(&canonical.to_string_lossy())
}

#[tauri::command]
fn get_session_chat(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<ChatMessage>, String> {
    let requests = lock_state(&state.requests);
    let path = resolve_session_transcript_path(&state, &session_id, &requests)
        .ok_or_else(|| format!("No transcript found for session {session_id}"))?;
    drop(requests);
    persist_session_transcript_path(&state, &session_id, &path);
    read_transcript_messages(&path)
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

fn extract_tool_use_from_entry(entry: &Value) -> (Option<String>, Option<Value>) {
    entry
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
                    Some((Some(name), input))
                } else {
                    None
                }
            })
        })
        .unwrap_or((None, None))
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
    if let Ok(mut usage_by_model) = state.session_token_usage_by_model.lock() {
        usage_by_model.clear();
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
    if let Ok(mut startup_floor) = state.startup_daily_floor.lock() {
        *startup_floor = TokenUsage::default();
    }
    if let Ok(mut startup_floor_by_model) = state.startup_daily_floor_by_model.lock() {
        startup_floor_by_model.clear();
    }
    if let Ok(mut absolute_sessions) = state.absolute_token_sessions.lock() {
        absolute_sessions.clear();
    }
}

fn token_usage_and_model_from_transcript_entry(
    entry: &Value,
    local_today_key: &str,
) -> Option<(String, TokenUsage)> {
    if entry.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }

    let Some(timestamp) = entry.get("timestamp").and_then(Value::as_str) else {
        return None;
    };
    if !local_time::is_local_today(timestamp, local_today_key) {
        return None;
    }

    let message = entry.get("message")?;
    let usage = message.get("usage");
    let model = message
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| pricing::UNKNOWN_MODEL.to_string());

    Some((
        model,
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
        },
    ))
}

fn token_usage_from_transcript_entry(entry: &Value, local_today_key: &str) -> TokenUsage {
    token_usage_and_model_from_transcript_entry(entry, local_today_key)
        .map(|(_, usage)| usage)
        .unwrap_or_default()
}

fn parse_claude_token_usage_from_transcript(
    transcript_path: &str,
    offset: u64,
    today_key: &str,
) -> Result<(TokenUsage, HashMap<String, TokenUsage>, u64, bool), String> {
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
    let mut usage_by_model: HashMap<String, TokenUsage> = HashMap::new();
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
        if let Some((model, entry_usage)) =
            token_usage_and_model_from_transcript_entry(&entry, today_key)
        {
            usage.add_assign(entry_usage);
            usage_by_model
                .entry(model)
                .or_default()
                .add_assign(entry_usage);
        }
    }

    Ok((usage, usage_by_model, next_offset, is_full_scan))
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
) -> Result<(TokenUsage, HashMap<String, TokenUsage>, u64, bool), String> {
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
        token_usage_map_from_delta_map(&parsed.daily_delta_by_model),
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
        Some(AgentKind::Cursor) => transcript::TranscriptFormat::Cursor,
        _ => transcript::detect_transcript_format(transcript_path),
    };

    let (parsed_usage, parsed_usage_by_model, next_offset, is_full_scan) = match format {
        transcript::TranscriptFormat::Codex => {
            parse_codex_token_usage_from_transcript(transcript_path, last_offset, &today_key)?
        }
        transcript::TranscriptFormat::Claude => {
            parse_claude_token_usage_from_transcript(transcript_path, last_offset, &today_key)?
        }
        // Cursor transcripts carry no token-usage data; tokens arrive
        // via `ingest_cursor_token_usage_from_payload` from hook payloads.
        // Always set is_full_scan=false so we never overwrite values that
        // were already injected by the stop hook.
        transcript::TranscriptFormat::Cursor => {
            let file_len = std::fs::metadata(transcript_path)
                .map(|m| m.len())
                .unwrap_or(last_offset);
            (TokenUsage::default(), HashMap::new(), file_len, false)
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
        // Transcript may be truncated or rotated; never regress a session total
        // that was already accumulated from hooks or a prior scan.
        *usage_entry = usage_entry.component_wise_max(parsed_usage);
        if let Ok(mut absolute_sessions) = state.absolute_token_sessions.lock() {
            absolute_sessions.insert(session_id.to_string());
        }
    } else {
        usage_entry.add_assign(parsed_usage);
    }
    drop(usage_by_session);

    if !parsed_usage_by_model.is_empty() {
        let mut usage_by_model = state
            .session_token_usage_by_model
            .lock()
            .map_err(|error| error.to_string())?;
        let model_entry = usage_by_model.entry(session_id.to_string()).or_default();
        merge_session_model_usage(model_entry, &parsed_usage_by_model, is_full_scan);
    }

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

/// Ingest token usage from a Cursor hook payload (`stop`, `afterAgentResponse`, etc.).
///
/// Cursor's JSONL transcript doesn't embed usage data; hook payloads may carry
/// `input_tokens`, `output_tokens`, `cache_read_tokens`, and `cache_write_tokens`
/// for the turn that just completed.
///
/// Fields may appear at the top level or nested under a `token_usage` object
/// depending on Cursor version.  Cursor reports `input_tokens` as the total
/// (cache_read + cache_write + fresh); we store the raw values and let the
/// display layer decide whether to decompose them.
pub(crate) fn ingest_cursor_token_usage_from_payload(
    state: &AppState,
    session_id: &str,
    payload: &serde_json::Value,
    source: &str,
) -> Result<(), String> {
    let parsed_usage = parse_cursor_token_usage_from_payload(payload);

    if parsed_usage.is_zero() {
        let keys: Vec<&str> = payload
            .as_object()
            .map(|obj| obj.keys().map(String::as_str).collect())
            .unwrap_or_default();
        eprintln!(
            "Atoll Cursor {source} payload has no token fields (session={session_id}, keys={keys:?})"
        );
        return Ok(());
    }

    eprintln!(
        "Atoll Cursor {source} tokens: input={} output={} cache_read={} \
         cache_write={} session={session_id}",
        parsed_usage.input_tokens,
        parsed_usage.output_tokens,
        parsed_usage.cache_read_tokens,
        parsed_usage.cache_creation_tokens
    );

    roll_over_token_usage_if_needed(state);

    {
        let mut usage_by_session = state
            .session_token_usage
            .lock()
            .map_err(|error| error.to_string())?;
        let entry = usage_by_session.entry(session_id.to_string()).or_default();
        // sessionEnd may report cumulative session totals; per-turn hooks add.
        if source == "sessionEnd" {
            *entry = entry.component_wise_max(parsed_usage);
        } else {
            entry.add_assign(parsed_usage);
        }
    }

    let model_id = extract_cursor_model(payload);
    let model_usage = HashMap::from([(model_id, parsed_usage)]);

    {
        let mut usage_by_model = state
            .session_token_usage_by_model
            .lock()
            .map_err(|error| error.to_string())?;
        let model_entry = usage_by_model.entry(session_id.to_string()).or_default();
        if source == "sessionEnd" {
            merge_session_model_usage(model_entry, &model_usage, true);
        } else {
            merge_session_model_usage(model_entry, &model_usage, false);
        }
    }

    if let Ok(mut sticky) = state.session_agent_map.lock() {
        sticky
            .entry(session_id.to_string())
            .or_insert_with(|| token_history::agent_kind_key(&AgentKind::Cursor));
    }

    token_history::sync_today_to_history(state)?;
    Ok(())
}

fn cursor_token_source(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .get("token_usage")
        .or_else(|| payload.get("tokenUsage"))
        .or_else(|| payload.get("usage"))
        .or_else(|| payload.get("token_usage_delta"))
        .or_else(|| payload.get("tokenUsageDelta"))
        .or_else(|| payload.get("total_token_usage"))
        .or_else(|| payload.get("totalTokenUsage"))
        .or_else(|| payload.get("response").and_then(|value| value.get("usage")))
        .or_else(|| payload.get("message").and_then(|value| value.get("usage")))
        .unwrap_or(payload)
}

fn parse_cursor_token_usage_from_payload(payload: &serde_json::Value) -> TokenUsage {
    let token_source = cursor_token_source(payload);
    TokenUsage {
        input_tokens: first_json_u64(
            token_source,
            &[
                "input_tokens",
                "inputTokens",
                "prompt_tokens",
                "promptTokens",
                "total_input_tokens",
                "totalInputTokens",
            ],
        ),
        output_tokens: first_json_u64(
            token_source,
            &[
                "output_tokens",
                "outputTokens",
                "completion_tokens",
                "completionTokens",
                "total_output_tokens",
                "totalOutputTokens",
            ],
        ),
        cache_read_tokens: first_json_u64(
            token_source,
            &[
                "cache_read_tokens",
                "cacheReadTokens",
                "cache_read_input_tokens",
                "cacheReadInputTokens",
                "cached_input_tokens",
                "cachedInputTokens",
            ],
        ),
        cache_creation_tokens: first_json_u64(
            token_source,
            &[
                "cache_write_tokens",
                "cacheWriteTokens",
                "cache_creation_tokens",
                "cacheCreationTokens",
                "cache_creation_input_tokens",
                "cacheCreationInputTokens",
            ],
        ),
    }
}

pub(crate) fn cursor_payload_has_token_usage(payload: &serde_json::Value) -> bool {
    !parse_cursor_token_usage_from_payload(payload).is_zero()
}

pub(crate) fn remember_cursor_lifecycle_token_session(state: &AppState, session_id: &str) {
    if let Ok(mut sessions) = state.cursor_lifecycle_token_sessions.lock() {
        sessions.insert(session_id.to_string());
    }
}

pub(crate) fn cursor_lifecycle_token_seen(state: &AppState, session_id: &str) -> bool {
    state
        .cursor_lifecycle_token_sessions
        .lock()
        .map(|sessions| sessions.contains(session_id))
        .unwrap_or(false)
}

fn first_json_u64(source: &serde_json::Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| json_value_as_u64(source.get(*key)))
        .unwrap_or(0)
}

fn first_json_string(source: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| source.get(*key).and_then(Value::as_str).map(str::to_string))
}

fn extract_cursor_model(payload: &serde_json::Value) -> String {
    first_json_string(
        payload,
        &[
            "model",
            "modelName",
            "model_name",
            "model_id",
            "modelId",
            "model_slug",
            "modelSlug",
        ],
    )
    .or_else(|| {
        payload
            .get("response")
            .and_then(|response| {
                first_json_string(
                    response,
                    &["model", "modelName", "model_name", "model_id", "modelId"],
                )
            })
    })
    .unwrap_or_else(|| pricing::UNKNOWN_MODEL.to_string())
}

/// Parse a JSON value as u64, accepting integers, floats, and numeric strings.
fn json_value_as_u64(value: Option<&serde_json::Value>) -> Option<u64> {
    let value = value?;
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    if let Some(f) = value.as_f64() {
        if f >= 0.0 && f <= u64::MAX as f64 {
            return Some(f as u64);
        }
    }
    if let Some(s) = value.as_str() {
        return s.parse::<u64>().ok();
    }
    None
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
fn set_model_rate(request: pricing::SetModelRateRequest) -> Result<pricing::PricingResponse, String> {
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

fn archive_subagent_in_state(state: &AppState, agent_id: &str) -> Option<String> {
    if let Ok(mut subagents) = state.active_subagents.lock() {
        subagents
            .iter_mut()
            .find(|s| s.agent_id == agent_id)
            .and_then(|sub| {
                let conv_id = sub.conversation_id.clone();
                sub.archived = true;
                conv_id
            })
    } else {
        None
    }
}

fn archive_completed_subagents_in_state(state: &AppState, session_id: &str) -> Vec<String> {
    if let Ok(mut subagents) = state.active_subagents.lock() {
        let mut conv_ids = Vec::new();
        for sub in subagents.iter_mut() {
            if sub.session_id == session_id && sub.completed_at.is_some() && !sub.archived {
                if let Some(conv_id) = sub.conversation_id.clone() {
                    conv_ids.push(conv_id);
                }
                sub.archived = true;
            }
        }
        conv_ids
    } else {
        Vec::new()
    }
}

#[tauri::command]
fn archive_subagent(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<IslandSnapshot, String> {
    let conv_id = archive_subagent_in_state(state.inner(), &agent_id);
    if let Some(conv_id) = conv_id {
        unbind_cursor_subagent_conversation(state.inner(), Some(&conv_id));
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
    let conv_ids = archive_completed_subagents_in_state(state.inner(), &session_id);
    for conv_id in conv_ids {
        unbind_cursor_subagent_conversation(state.inner(), Some(&conv_id));
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
    let resolved_cwd = match agent {
        AgentKind::Codex => resolve_codex_session_cwd(cwd, transcript_path),
        _ => cwd.to_string(),
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
                conversation_id: None,
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

pub(crate) fn cursor_session_host(state: &AppState, session_id: &str) -> platform::SessionHost {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if entry.host != platform::SessionHost::Unknown {
                return entry.host;
            }
        }
    }

    let detected = platform::detect_cursor_session_host();
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
        AgentKind::Cursor => {
            if let Some(entry) = known_sessions.get(session_id) {
                if entry.host != platform::SessionHost::Unknown {
                    return entry.host;
                }
            }
            if platform::is_cursor_app_running() {
                return platform::SessionHost::CursorIde;
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

pub(crate) fn payload_subagent_id(payload: &Value) -> Option<&str> {
    payload
        .get("agent_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("subagent_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("tool_call_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

pub(crate) fn payload_subagent_parent_session_id(payload: &Value) -> Option<&str> {
    payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("sessionId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("parent_conversation_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("parentConversationId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("conversation_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("conversationId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

pub(crate) fn payload_subagent_type(payload: &Value) -> &str {
    payload
        .get("agent_type")
        .and_then(Value::as_str)
        .or_else(|| payload.get("subagent_type").and_then(Value::as_str))
        .unwrap_or("unknown")
}

pub(crate) fn payload_subagent_transcript_path(payload: &Value) -> Option<&str> {
    payload
        .get("agent_transcript_path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("agentTranscriptPath")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

pub(crate) fn payload_main_transcript_path(payload: &Value) -> Option<&str> {
    payload
        .get("transcript_path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("transcriptPath")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

pub(crate) fn payload_subagent_last_message(payload: &Value) -> Option<String> {
    payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .or_else(|| payload.get("summary").and_then(Value::as_str))
        .map(|s| s.chars().take(200).collect())
}

pub(crate) fn payload_conversation_id(payload: &Value) -> Option<&str> {
    payload
        .get("conversation_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("conversationId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

/// Cursor stores project folders under `~/.cursor/projects/{slug}/` where macOS
/// absolute paths become slugs like `Users-me-code-Atoll`.
pub(crate) fn decode_cursor_project_slug(slug: &str) -> Option<String> {
    if slug.is_empty() || slug == "empty-window" {
        return None;
    }
    if slug.starts_with("Users-") {
        let candidate = format!("/Users/{}", slug["Users-".len()..].replace('-', "/"));
        if std::path::Path::new(&candidate).is_dir() {
            return Some(candidate);
        }
    }
    #[cfg(windows)]
    if slug.len() > 2 {
        let drive = slug.as_bytes()[0] as char;
        if drive.is_ascii_alphabetic() && slug.as_bytes()[1] == b'-' {
            let candidate = format!(
                "{}:\\{}",
                drive.to_ascii_uppercase(),
                slug[2..].replace('-', "\\")
            );
            if std::path::Path::new(&candidate).is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Keep Cursor hooks.json env URLs aligned with the running bridge port.
pub(crate) fn sync_cursor_hook_bridge_urls(app: &AppHandle, port: u16) {
    let hooks_path = match cursor_hooks_path() {
        Some(path) => path,
        None => return,
    };
    let path_str = hooks_path.to_string_lossy();
    let Some(mut config) = read_json_file(&path_str) else {
        return;
    };
    if !has_atoll_cursor_hooks(&config) {
        return;
    }
    let cursor_url = hook_bridge::cursor_hook_url(port);
    let Some(hooks_obj) = config.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    let mut updated = false;
    for entries in hooks_obj.values_mut() {
        let Some(arr) = entries.as_array_mut() else {
            continue;
        };
        for entry in arr.iter_mut() {
            if !hook_entry_has_atoll_cursor(entry) {
                continue;
            }
            let Some(obj) = entry.as_object_mut() else {
                continue;
            };
            let mut env = obj
                .get("env")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let current = env
                .get("ATOLL_HOOK_URL")
                .and_then(Value::as_str)
                .unwrap_or("");
            if current != cursor_url {
                env.insert(
                    "ATOLL_HOOK_URL".to_string(),
                    Value::String(cursor_url.clone()),
                );
                obj.insert("env".to_string(), Value::Object(env));
                updated = true;
            }
            if obj.get("timeout").and_then(Value::as_u64) != Some(CURSOR_HOOK_TIMEOUT_SECONDS) {
                obj.insert("timeout".to_string(), json!(CURSOR_HOOK_TIMEOUT_SECONDS));
                updated = true;
            }
        }
    }
    if !updated {
        return;
    }
    let formatted = match serde_json::to_string_pretty(&config) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Atoll failed to serialize Cursor hooks for URL sync: {error}");
            return;
        }
    };
    if let Err(error) = std::fs::write(&hooks_path, formatted) {
        eprintln!("Atoll failed to write Cursor hooks for URL sync: {error}");
        return;
    }
    eprintln!("Atoll synced Cursor hook URLs to {cursor_url}");
    refresh_hook_health_cache(app, &app.state::<AppState>());
    // #region agent log
    crate::debug_agent::log(
        "H-C",
        "lib.rs:sync_cursor_hook_bridge_urls",
        "synced cursor hook env urls",
        json!({
            "port": port,
            "cursorUrl": cursor_url,
            "hooksPath": path_str,
        }),
    );
    // #endregion
}

pub(crate) fn payload_cursor_lookup_id(payload: &Value) -> Option<&str> {
    payload_conversation_id(payload).or_else(|| {
        payload
            .get("session_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                payload
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
            })
    })
}

/// Prefer full composer UUID for Cursor session keys (matches on-disk transcript dirs).
pub(crate) fn payload_cursor_session_id(payload: &Value) -> Option<&str> {
    payload_cursor_lookup_id(payload)
}

pub(crate) const CURSOR_TRANSCRIPT_PREFIX_MIN_LEN: usize = 6;

/// Locate a Cursor composer transcript on disk and infer its workspace cwd.
pub(crate) fn discover_cursor_agent_transcript(lookup_id: &str) -> Option<(String, String)> {
    if lookup_id.is_empty() {
        return None;
    }
    if let Some(found) = discover_cursor_agent_transcript_exact(lookup_id) {
        return Some(found);
    }
    if lookup_id.len() >= CURSOR_TRANSCRIPT_PREFIX_MIN_LEN {
        return discover_cursor_agent_transcript_by_prefix(lookup_id);
    }
    None
}

fn discover_cursor_agent_transcript_exact(conversation_id: &str) -> Option<(String, String)> {
    let home = dirs::home_dir()?;
    let projects = home.join(".cursor").join("projects");
    if !projects.is_dir() {
        return None;
    }
    let relative = std::path::PathBuf::from("agent-transcripts")
        .join(conversation_id)
        .join(format!("{conversation_id}.jsonl"));
    for entry in std::fs::read_dir(&projects).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let transcript = entry.path().join(&relative);
        if !transcript.is_file() {
            continue;
        }
        let workspace = decode_cursor_project_slug(&entry.file_name().to_string_lossy())
            .unwrap_or_else(|| ".".to_string());
        return Some((transcript.to_string_lossy().into_owned(), workspace));
    }
    None
}

fn discover_cursor_agent_transcript_by_prefix(prefix: &str) -> Option<(String, String)> {
    let home = dirs::home_dir()?;
    let projects = home.join(".cursor").join("projects");
    if !projects.is_dir() {
        return None;
    }

    let mut best: Option<(String, String, usize)> = None;
    for entry in std::fs::read_dir(&projects).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let transcripts_dir = entry.path().join("agent-transcripts");
        let Ok(conv_entries) = std::fs::read_dir(&transcripts_dir) else {
            continue;
        };
        for conv in conv_entries.flatten() {
            if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let conv_id = conv.file_name().to_string_lossy().into_owned();
            if !conv_id.starts_with(prefix) {
                continue;
            }
            let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
            if !jsonl.is_file() {
                continue;
            }
            let workspace = decode_cursor_project_slug(&entry.file_name().to_string_lossy())
                .unwrap_or_else(|| ".".to_string());
            let path = jsonl.to_string_lossy().into_owned();
            let score = conv_id.len();
            if best
                .as_ref()
                .map(|(_, _, len)| score > *len)
                .unwrap_or(true)
            {
                best = Some((path, workspace, score));
            }
        }
    }

    best.map(|(path, workspace, _)| (path, workspace))
}

pub(crate) fn is_unresolved_cursor_cwd(cwd: &str) -> bool {
    cwd.is_empty() || cwd == "."
}

/// Fill missing Cursor cwd/transcript from on-disk agent transcripts.
pub(crate) fn backfill_cursor_session_metadata(state: &AppState) {
    let sessions_to_backfill: Vec<(String, Option<String>)> = state
        .known_sessions
        .lock()
        .ok()
        .map(|known| {
            known
                .iter()
                .filter(|(_, info)| matches!(info.agent, AgentKind::Cursor))
                .filter(|(_, info)| {
                    info.transcript_path.is_none() || is_unresolved_cursor_cwd(&info.cwd)
                })
                .map(|(id, info)| (id.clone(), info.conversation_id.clone()))
                .collect()
        })
        .unwrap_or_default();

    for (session_id, conversation_id) in sessions_to_backfill {
        let lookup_id = conversation_id.as_deref().unwrap_or(session_id.as_str());
        let Some((path, workspace)) = discover_cursor_agent_transcript(lookup_id) else {
            continue;
        };
        if let Ok(mut known) = state.known_sessions.lock() {
            if let Some(entry) = known.get_mut(&session_id) {
                if entry.transcript_path.is_none() {
                    entry.transcript_path = Some(path);
                }
                if is_unresolved_cursor_cwd(&entry.cwd) && !is_unresolved_cursor_cwd(&workspace) {
                    entry.cwd = workspace;
                }
                if entry.conversation_id.is_none() {
                    if let Some(stem) = entry
                        .transcript_path
                        .as_deref()
                        .and_then(|path| std::path::Path::new(path).parent())
                        .and_then(|p| p.file_name())
                        .and_then(|name| name.to_str())
                    {
                        entry.conversation_id = Some(stem.to_string());
                    }
                }
            }
        }
    }
}

fn sanitize_subagent_id_for_filename(agent_id: &str) -> String {
    agent_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn cursor_subagents_dir(main_transcript: &str) -> Option<std::path::PathBuf> {
    std::path::Path::new(main_transcript)
        .parent()
        .map(|parent| parent.join("subagents"))
}

fn subagent_transcript_filename_candidates(
    agent_id: &str,
    conversation_id: Option<&str>,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(conv) = conversation_id.filter(|value| !value.is_empty()) {
        candidates.push(format!("{conv}.jsonl"));
    }
    let sanitized = sanitize_subagent_id_for_filename(agent_id);
    if agent_id.starts_with("agent-") {
        candidates.push(format!("{agent_id}.jsonl"));
    } else {
        candidates.push(format!("agent-{agent_id}.jsonl"));
    }
    candidates.push(format!("{agent_id}.jsonl"));
    if sanitized.starts_with("agent-") {
        candidates.push(format!("{sanitized}.jsonl"));
    } else {
        candidates.push(format!("agent-{sanitized}.jsonl"));
    }
    candidates.push(format!("{sanitized}.jsonl"));
    candidates.sort();
    candidates.dedup();
    candidates
}

fn scan_subagents_dir_for_transcript(
    subagents_dir: &std::path::Path,
    started_at: Option<&str>,
) -> Option<String> {
    if !subagents_dir.is_dir() {
        return None;
    }
    let started_ts = started_at.map(parse_iso_timestamp_secs);
    let mut matches: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(subagents_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                return None;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            if let Some(started_ts) = started_ts {
                if modified + 2 < started_ts {
                    return None;
                }
            }
            Some((modified, path))
        })
        .collect();
    if matches.is_empty() {
        return None;
    }
    matches.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    matches
        .first()
        .map(|(_, path)| path.to_string_lossy().into_owned())
}

fn derive_subagent_transcript_path(
    main_transcript: Option<&str>,
    agent_id: &str,
    conversation_id: Option<&str>,
    started_at: Option<&str>,
) -> Option<String> {
    let main = main_transcript?;
    let subagents_dir = cursor_subagents_dir(main)?;

    for filename in subagent_transcript_filename_candidates(agent_id, conversation_id) {
        let path = subagents_dir.join(&filename);
        if path.exists() {
            return Some(path.to_string_lossy().into_owned());
        }
    }

    if let Some(path) = scan_subagents_dir_for_transcript(&subagents_dir, started_at) {
        return Some(path);
    }

    if let Some(conv) = conversation_id.filter(|value| !value.is_empty()) {
        return Some(
            subagents_dir
                .join(format!("{conv}.jsonl"))
                .to_string_lossy()
                .into_owned(),
        );
    }

    None
}

fn known_session_transcript_path(state: &AppState, session_id: &str) -> Option<String> {
    state
        .known_sessions
        .lock()
        .ok()
        .and_then(|known| known_session_transcript_path_from_map(&known, session_id))
}

fn known_session_transcript_path_from_map(
    known_sessions: &HashMap<String, KnownSession>,
    session_id: &str,
) -> Option<String> {
    known_sessions
        .get(session_id)
        .and_then(|entry| entry.transcript_path.clone())
}

fn refreshed_subagent_transcript_path(
    main_transcript: Option<&str>,
    sub: &ActiveSubagent,
) -> Option<String> {
    let Some(resolved) = derive_subagent_transcript_path(
        main_transcript,
        &sub.agent_id,
        sub.conversation_id.as_deref(),
        Some(&sub.started_at),
    ) else {
        return None;
    };
    let current_missing = sub
        .agent_transcript_path
        .as_ref()
        .is_none_or(|path| !std::path::Path::new(path).exists());
    let resolved_exists = std::path::Path::new(&resolved).exists();
    if current_missing && (resolved_exists || sub.conversation_id.is_some()) {
        Some(resolved)
    } else {
        None
    }
}

fn resolve_complete_transcript_path_from_main(
    main_transcript: Option<&str>,
    sub: &ActiveSubagent,
    payload_path: Option<String>,
) -> Option<String> {
    if let Some(path) = payload_path.filter(|value| !value.is_empty()) {
        return Some(path);
    }
    derive_subagent_transcript_path(
        main_transcript,
        &sub.agent_id,
        sub.conversation_id.as_deref(),
        Some(&sub.started_at),
    )
}

fn bind_cursor_subagent_conversation(state: &AppState, conv_id: &str, parent_session_id: &str) {
    if let Ok(mut map) = state.cursor_subagent_conversations.lock() {
        map.insert(conv_id.to_string(), parent_session_id.to_string());
    }
}

fn unbind_cursor_subagent_conversation(state: &AppState, conv_id: Option<&str>) {
    if let Some(conv_id) = conv_id {
        if let Ok(mut map) = state.cursor_subagent_conversations.lock() {
            map.remove(conv_id);
        }
    }
}

/// Resolve a Cursor hook payload to its parent session when the event belongs to a subagent.
pub(crate) fn resolve_cursor_session_for_payload(
    state: &AppState,
    payload: &Value,
) -> Option<String> {
    if let Some(agent_id) = payload_subagent_id(payload) {
        if let Ok(subagents) = state.active_subagents.lock() {
            if let Some(sub) = subagents
                .iter()
                .find(|s| s.agent_id == agent_id && !s.archived && s.completed_at.is_none())
            {
                return Some(sub.session_id.clone());
            }
        }
    }

    let conv_id = payload_conversation_id(payload)?;

    if let Ok(map) = state.cursor_subagent_conversations.lock() {
        if let Some(parent) = map.get(conv_id) {
            return Some(parent.clone());
        }
    }

    let subagents = state.active_subagents.lock().ok()?;
    let is_known_parent = subagents
        .iter()
        .any(|s| s.session_id == conv_id && s.completed_at.is_none() && !s.archived);
    if is_known_parent {
        return None;
    }

    let mut running_unbound: Vec<&ActiveSubagent> = subagents
        .iter()
        .filter(|s| {
            matches!(s.agent_kind, AgentKind::Cursor)
                && s.completed_at.is_none()
                && !s.archived
                && s.conversation_id.is_none()
        })
        .collect();
    if running_unbound.is_empty() {
        return None;
    }

    if let Some(type_filter) = payload
        .get("subagent_type")
        .or_else(|| payload.get("agent_type"))
        .and_then(Value::as_str)
    {
        running_unbound.retain(|s| s.agent_type == type_filter);
        if running_unbound.is_empty() {
            return None;
        }
    }

    let parent = running_unbound
        .iter()
        .min_by_key(|s| &s.started_at)?
        .session_id
        .clone();
    drop(subagents);

    bind_cursor_subagent_conversation(state, conv_id, &parent);
    let main_transcript = known_session_transcript_path(state, &parent);
    let refresh_target = {
        let mut subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return Some(parent),
        };
        subagents
            .iter_mut()
            .find(|s| {
                s.session_id == parent
                    && s.conversation_id.is_none()
                    && s.completed_at.is_none()
                    && !s.archived
            })
            .map(|sub| {
                sub.conversation_id = Some(conv_id.to_string());
                sub.clone()
            })
    };

    if let Some(target) = refresh_target {
        if let Some(path) = refreshed_subagent_transcript_path(main_transcript.as_deref(), &target)
        {
            if let Ok(mut subagents) = state.active_subagents.lock() {
                if let Some(sub) = subagents.iter_mut().find(|s| {
                    s.agent_id == target.agent_id
                        && s.conversation_id.as_deref() == Some(conv_id)
                        && s.completed_at.is_none()
                        && !s.archived
                }) {
                    sub.agent_transcript_path = Some(path);
                }
            }
        }
    }
    Some(parent)
}

pub(crate) fn register_subagent_start(
    state: &AppState,
    payload: &serde_json::Value,
    agent_kind: AgentKind,
) {
    let agent_id = payload_subagent_id(payload).unwrap_or("").to_string();
    let session_id = payload_subagent_parent_session_id(payload)
        .unwrap_or("")
        .to_string();
    let agent_type = payload_subagent_type(payload).to_string();
    let agent_transcript_path = payload_subagent_transcript_path(payload)
        .map(str::to_string)
        .or_else(|| {
            derive_subagent_transcript_path(
                payload_main_transcript_path(payload),
                &agent_id,
                None,
                None,
            )
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
        conversation_id: None,
    };

    if let Ok(mut subagents) = state.active_subagents.lock() {
        if !subagents.iter().any(|s| s.agent_id == subagent.agent_id) {
            subagents.push(subagent);
        }
    }
}

pub(crate) fn complete_subagent(state: &AppState, payload: &serde_json::Value) {
    let payload_transcript_path = payload_subagent_transcript_path(payload).map(str::to_string);
    let last_message = payload_subagent_last_message(payload);

    if let Some(agent_id) = payload_subagent_id(payload) {
        let target = state
            .active_subagents
            .lock()
            .ok()
            .and_then(|subagents| subagents.iter().find(|s| s.agent_id == agent_id).cloned());

        if let Some(target) = target {
            let main_transcript = known_session_transcript_path(state, &target.session_id);
            let transcript_path = resolve_complete_transcript_path_from_main(
                main_transcript.as_deref(),
                &target,
                payload_transcript_path,
            );
            let conv_id = target.conversation_id.clone();
            if let Ok(mut subagents) = state.active_subagents.lock() {
                if let Some(sub) = subagents.iter_mut().find(|s| s.agent_id == target.agent_id) {
                    mark_subagent_complete(sub, transcript_path, last_message);
                }
            }
            unbind_cursor_subagent_conversation(state, conv_id.as_deref());
        }
        return;
    }

    let Some(parent_session) = payload_subagent_parent_session_id(payload) else {
        return;
    };
    let type_filter = payload
        .get("subagent_type")
        .or_else(|| payload.get("agent_type"))
        .and_then(Value::as_str);

    let target = {
        let subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        subagents
            .iter()
            .filter(|s| s.session_id == parent_session && s.completed_at.is_none() && !s.archived)
            .filter(|s| type_filter.map(|t| s.agent_type == t).unwrap_or(true))
            .min_by_key(|s| s.started_at.clone())
            .cloned()
    };

    if let Some(target) = target {
        let main_transcript = known_session_transcript_path(state, &target.session_id);
        let transcript_path = resolve_complete_transcript_path_from_main(
            main_transcript.as_deref(),
            &target,
            payload_transcript_path,
        );
        let conv_id = target.conversation_id.clone();
        if let Ok(mut subagents) = state.active_subagents.lock() {
            if let Some(sub) = subagents
                .iter_mut()
                .find(|s| s.agent_id == target.agent_id && s.completed_at.is_none() && !s.archived)
            {
                mark_subagent_complete(sub, transcript_path, last_message);
            }
        }
        unbind_cursor_subagent_conversation(state, conv_id.as_deref());
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
    let known_transcripts: HashMap<String, String> = state
        .known_sessions
        .lock()
        .ok()
        .map(|known| {
            known
                .iter()
                .filter_map(|(session_id, session)| {
                    session
                        .transcript_path
                        .as_ref()
                        .map(|path| (session_id.clone(), path.clone()))
                })
                .collect()
        })
        .unwrap_or_default();

    let refresh_candidates: Vec<ActiveSubagent> = {
        let subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        subagents
            .iter()
            .filter(|sub| sub.completed_at.is_none() && !sub.archived)
            .cloned()
            .collect()
    };

    let path_updates: Vec<(String, String)> = refresh_candidates
        .iter()
        .filter_map(|sub| {
            refreshed_subagent_transcript_path(
                known_transcripts.get(&sub.session_id).map(String::as_str),
                sub,
            )
            .map(|path| (sub.agent_id.clone(), path))
        })
        .collect();

    if !path_updates.is_empty() {
        if let Ok(mut subagents) = state.active_subagents.lock() {
            for (agent_id, path) in path_updates {
                if let Some(sub) = subagents.iter_mut().find(|sub| {
                    sub.agent_id == agent_id && sub.completed_at.is_none() && !sub.archived
                }) {
                    sub.agent_transcript_path = Some(path);
                }
            }
        }
    }

    let pending_paths: Vec<(String, String)> = {
        let subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        subagents
            .iter()
            .filter(|sub| sub.completed_at.is_none() && !sub.archived)
            .filter_map(|sub| {
                sub.agent_transcript_path
                    .as_ref()
                    .map(|path| (sub.agent_id.clone(), path.clone()))
            })
            .collect()
    };

    if pending_paths.is_empty() {
        return;
    }

    let results: Vec<(String, String)> = pending_paths
        .into_iter()
        .filter_map(|(agent_id, path)| {
            transcript::extract_subagent_terminal_message(&path).map(|msg| (agent_id, msg))
        })
        .collect();

    if results.is_empty() {
        return;
    }

    let mut subagents = match state.active_subagents.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    for (agent_id, message) in results {
        if let Some(sub) = subagents.iter_mut().find(|sub| sub.agent_id == agent_id) {
            if sub.completed_at.is_none() && !sub.archived {
                mark_subagent_complete(sub, None, Some(message));
            }
        }
    }
}

const SUBAGENT_SNAPSHOT_MIN_INTERVAL: Duration = Duration::from_millis(300);
const SUBAGENT_RECONCILE_MIN_INTERVAL: Duration = Duration::from_secs(2);
const OBSERVER_SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(400);
const TOKEN_REFRESH_INTERVAL_ACTIVE: Duration = Duration::from_millis(900);
const TOKEN_REFRESH_INTERVAL_IDLE: Duration = Duration::from_secs(5);
const HOOK_ACTIVITY_IDLE_THRESHOLD: Duration = Duration::from_secs(30);

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
    reconcile_incomplete_subagents_now(state);
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
    /// True when Atoll's hook script content changed since the host CLI last
    /// trusted it (e.g. an Atoll update overwrote the script in place). The
    /// host may be silently ignoring the hook until the user re-trusts it.
    #[serde(default)]
    needs_retrust: bool,
}

fn default_node_found() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
struct HookHealthSnapshot {
    claude: HookStatus,
    codex: HookStatus,
    cursor: HookStatus,
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
            needs_retrust: false,
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
            needs_retrust: false,
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

    let written = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot verify settings: {e}"))?;
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
    hook_trust::record_hook_installed("claude", &script_path);

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
        hook_trust::clear_hook_installed("claude");
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: settings_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
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
    hook_trust::clear_hook_installed("claude");

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

fn cursor_hooks_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".cursor").join("hooks.json"))
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
            needs_retrust: false,
        });
    }
    Ok(codex_hook_status(&app))
}

/// Stable hook install dir so hooks.json does not point at `target/debug/scripts`,
/// which disappears during rebuilds and makes Codex hooks exit with code 1.
fn atoll_local_hooks_dir() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("Atoll").join("hooks"))
}

fn deployed_hook_script_path(script_name: &str) -> Option<String> {
    let path = atoll_local_hooks_dir()?.join(script_name);
    if path.is_file() {
        Some(normalize_hook_script_path(&path.to_string_lossy()))
    } else {
        None
    }
}

fn files_equal(left: &std::path::Path, right: &std::path::Path) -> bool {
    match (std::fs::read(left), std::fs::read(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn deployed_hook_assets_current(
    source_script: &std::path::Path,
    deployed_script: &std::path::Path,
) -> bool {
    if !files_equal(source_script, deployed_script) {
        return false;
    }

    let Some(source_dir) = source_script.parent() else {
        return true;
    };
    let Some(deployed_dir) = deployed_script.parent() else {
        return true;
    };
    let source_bridge = source_dir.join("atoll-hook-bridge.mjs");
    if !source_bridge.is_file() {
        return true;
    }
    files_equal(&source_bridge, &deployed_dir.join("atoll-hook-bridge.mjs"))
}

fn refresh_deployed_hook_assets_if_needed(app: &AppHandle, script_name: &str) {
    let Some(deployed_script_path) = deployed_hook_script_path(script_name) else {
        return;
    };
    let Ok(source_script_path) = resolve_install_hook_script_path(app, script_name) else {
        return;
    };
    if source_script_path == deployed_script_path {
        return;
    }

    let source = std::path::Path::new(&source_script_path);
    let deployed = std::path::Path::new(&deployed_script_path);
    if deployed_hook_assets_current(source, deployed) {
        return;
    }

    if let Err(error) = materialize_hook_deployment(app, script_name, &source_script_path) {
        eprintln!("Atoll failed to refresh deployed {script_name}: {error}");
    }
}

fn canonical_hook_script_path(
    app: &AppHandle,
    script_name: &str,
    config: Option<&Value>,
    marker: &str,
    fallback_path: &str,
) -> String {
    if let Some(deployed) = deployed_hook_script_path(script_name) {
        return deployed;
    }
    if let Some(configured) = config.and_then(|cfg| configured_atoll_hook_script_path(cfg, marker))
    {
        if std::path::Path::new(&configured).is_file() {
            return configured;
        }
    }
    if !fallback_path.is_empty() && std::path::Path::new(fallback_path).is_file() {
        return fallback_path.to_string();
    }
    resolve_hook_script_path(app, script_name).unwrap_or_default()
}

#[cfg(windows)]
fn maybe_repair_hook_launcher_config(app: &AppHandle, script_name: &str, config_filename: &str) {
    let Some(local_dir) = dirs::data_local_dir().map(|dir| dir.join("Atoll")) else {
        return;
    };
    let config_path = local_dir.join(config_filename);
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<Value>(&content) else {
        return;
    };
    let current_script = config.get("script").and_then(Value::as_str).unwrap_or("");
    let needs_repair = current_script.is_empty()
        || is_dev_hook_script_path(current_script)
        || !std::path::Path::new(current_script).is_file();
    if !needs_repair {
        return;
    }
    let Some(stable_script) = deployed_hook_script_path(script_name) else {
        return;
    };
    let runner = atoll_local_hooks_dir()
        .map(|dir| dir.join("atoll-hook-runner.exe"))
        .filter(|path| path.is_file())
        .map(|path| normalize_hook_command_path(&path.to_string_lossy()))
        .or_else(|| hook_runner_for_command(app));
    let node = config
        .get("node")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| resolve_node_executable().ok());
    let (Some(runner), Some(node)) = (runner, node) else {
        return;
    };
    config["script"] = json!(normalize_hook_command_path(&stable_script));
    config["runner"] = json!(runner);
    config["node"] = json!(normalize_hook_command_path(&node));
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(&config_path, formatted);
    }
}

#[cfg(not(windows))]
fn maybe_repair_hook_launcher_config(_app: &AppHandle, _script_name: &str, _config_filename: &str) {
}

#[cfg(windows)]
fn is_windows_file_locked_error(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32))
}

#[cfg(not(windows))]
fn is_windows_file_locked_error(_error: &std::io::Error) -> bool {
    false
}

/// Copy hook assets into the stable deploy dir. If Windows reports that the
/// destination is locked (ERROR_SHARING_VIOLATION / os error 32) because Codex,
/// Cursor, or a live hook invocation still has the runner open, keep the existing
/// file so install can finish updating hooks.json and launcher config.
fn copy_deployed_hook_file(
    source: &std::path::Path,
    dest: &std::path::Path,
    label: &str,
) -> Result<(), String> {
    match std::fs::copy(source, dest) {
        Ok(_) => Ok(()),
        Err(error) => {
            if dest.is_file() && is_windows_file_locked_error(&error) {
                eprintln!(
                    "Atoll kept existing {label} at {} because the file is in use ({error})",
                    dest.display()
                );
                Ok(())
            } else {
                Err(format!(
                    "Cannot copy {label} to {}: {error}",
                    dest.display()
                ))
            }
        }
    }
}

fn materialize_hook_deployment(
    app: &AppHandle,
    script_name: &str,
    source_script_path: &str,
) -> Result<String, String> {
    let source = std::path::Path::new(source_script_path);
    if !source.is_file() {
        return Err(format!("Hook script not found at: {source_script_path}"));
    }

    let hooks_dir = atoll_local_hooks_dir()
        .ok_or_else(|| "Cannot determine local data directory".to_string())?;
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|error| format!("Cannot create {}: {error}", hooks_dir.display()))?;

    let dest_script = hooks_dir.join(script_name);
    copy_deployed_hook_file(source, &dest_script, "hook script")?;

    if let Some(source_dir) = source.parent() {
        let bridge_name = "atoll-hook-bridge.mjs";
        let source_bridge = source_dir.join(bridge_name);
        if source_bridge.is_file() {
            let dest_bridge = hooks_dir.join(bridge_name);
            copy_deployed_hook_file(&source_bridge, &dest_bridge, "hook bridge module")?;
        }
    }

    #[cfg(windows)]
    {
        let dest_runner = hooks_dir.join("atoll-hook-runner.exe");
        if let Some(runner_path) = hook_runner_for_command(app) {
            let runner_source = std::path::Path::new(&runner_path);
            if runner_source.is_file() {
                copy_deployed_hook_file(runner_source, &dest_runner, "hook runner")?;
            }
        }
        if !dest_runner.is_file() {
            return Err(
                "Cannot locate atoll-hook-runner.exe. Rebuild Atoll, then try installing hooks again."
                    .into(),
            );
        }
    }

    Ok(normalize_hook_script_path(&dest_script.to_string_lossy()))
}

#[tauri::command]
fn install_codex_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, "atoll-codex-hook.mjs")?;
    let script_path =
        materialize_hook_deployment(&app, "atoll-codex-hook.mjs", &source_script_path)?;

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

    #[cfg(windows)]
    let hook_command = write_codex_hook_launcher_command(&app, &node_path, &script_path)?;
    #[cfg(not(windows))]
    let hook_command = format_hook_command(None, &node_path, &script_path);
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
    hook_trust::on_codex_hooks_installed(&script_path);

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
        hook_trust::clear_hook_installed("codex");
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: hooks_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
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
    hook_trust::clear_hook_installed("codex");

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
fn get_cursor_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    if capture::force_hook_uninstalled() {
        let script_path =
            resolve_hook_script_path(&app, "atoll-cursor-hook.mjs").unwrap_or_default();
        return Ok(HookStatus {
            installed: false,
            script_found: !script_path.is_empty() && std::path::Path::new(&script_path).exists(),
            settings_path: cursor_hooks_path()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            script_path,
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
        });
    }
    Ok(cursor_hook_status(&app))
}

#[tauri::command]
fn install_cursor_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, "atoll-cursor-hook.mjs")?;
    let script_path =
        materialize_hook_deployment(&app, "atoll-cursor-hook.mjs", &source_script_path)?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = resolve_node_executable()?;

    let hooks_path =
        cursor_hooks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = hooks_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create ~/.cursor directory: {e}"))?;
    }

    let mut config: Value = if hooks_path.exists() {
        let content =
            std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot read hooks: {e}"))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    if config.get("version").is_none() {
        if let Some(obj) = config.as_object_mut() {
            obj.insert("version".to_string(), json!(1));
        }
    }

    let hook_command = write_cursor_hook_launcher_command(&app, &node_path, &script_path)?;

    let config_obj = config
        .as_object_mut()
        .ok_or_else(|| "hooks.json is not a JSON object".to_string())?;
    let hooks_obj = config_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    upsert_cursor_hook_events(
        hooks_obj,
        &hook_command,
        &hook_bridge::cursor_hook_url_for_app(&app),
    );

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize hooks: {e}"))?;
    std::fs::write(&hooks_path, formatted).map_err(|e| format!("Cannot write hooks: {e}"))?;

    let written =
        std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot verify hooks: {e}"))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse hooks after write: {e}"))?;
    if !has_atoll_cursor_hooks(&verify) {
        return Err(
            "Cursor hooks were not saved correctly. Check permissions on ~/.cursor/hooks.json."
                .into(),
        );
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after Cursor hook install: {error}");
    }
    hook_trust::record_hook_installed("cursor", &script_path);

    let state = app.state::<AppState>();
    refresh_hook_health_cache(&app, &state);
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(cursor_hook_status(&app))
}

#[tauri::command]
fn uninstall_cursor_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let hooks_path =
        cursor_hooks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !hooks_path.exists() {
        hook_trust::clear_hook_installed("cursor");
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: hooks_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
        });
    }

    let content =
        std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot read hooks: {e}"))?;
    let mut config: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    if let Some(hooks) = config.get_mut("hooks") {
        remove_atoll_cursor_hooks(hooks);
    }

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize hooks: {e}"))?;
    std::fs::write(&hooks_path, formatted).map_err(|e| format!("Cannot write hooks: {e}"))?;
    hook_trust::clear_hook_installed("cursor");

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(cursor_hook_status(&app))
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

fn normalize_hook_command_path(path: &str) -> String {
    let path = normalize_hook_script_path(path);
    #[cfg(windows)]
    {
        path.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        path
    }
}

fn format_hook_command(_runner_path: Option<&str>, node_path: &str, script_path: &str) -> String {
    let node_path = normalize_hook_command_path(node_path);
    let script_path = normalize_hook_command_path(script_path);

    #[cfg(windows)]
    if let Some(runner_path) = _runner_path {
        let runner_path = normalize_hook_command_path(runner_path);
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

/// Windows hook hosts (Cursor, Codex) often spawn hook commands through `cmd /c`.
/// A single quoted string like `"runner.exe" "node.exe" "script.mjs"` fails on paths
/// with spaces or non-ASCII profile dirs. Write a PowerShell launcher that forwards
/// stdin to `atoll-hook-runner.exe`; paths live in a UTF-8 JSON config file.
#[cfg(windows)]
fn write_windows_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
    config_filename: &str,
    ps1_filename: &str,
    fallback_stdout: &str,
) -> Result<String, String> {
    let stable_runner = atoll_local_hooks_dir()
        .map(|dir| dir.join("atoll-hook-runner.exe"))
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned());
    let runner_path = stable_runner
        .or_else(|| hook_runner_for_command(app))
        .ok_or_else(|| "Cannot locate atoll-hook-runner.exe".to_string())?;
    let local_dir = dirs::data_local_dir()
        .ok_or_else(|| "Cannot determine local data directory".to_string())?
        .join("Atoll");
    std::fs::create_dir_all(&local_dir)
        .map_err(|error| format!("Cannot create {}: {error}", local_dir.display()))?;

    let runner = normalize_hook_command_path(&runner_path);
    let node = normalize_hook_command_path(node_path);
    let script = normalize_hook_command_path(script_path);
    let config_path = local_dir.join(config_filename);
    let config = json!({
        "runner": runner,
        "node": node,
        "script": script,
    });
    let config_json = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("Cannot serialize hook launcher config: {error}"))?;
    std::fs::write(&config_path, config_json.as_bytes())
        .map_err(|error| format!("Cannot write {}: {error}", config_path.display()))?;

    let ps1_path = local_dir.join(ps1_filename);
    let ps1_body = format!(
        r#"$ErrorActionPreference = 'Stop'
$configPath = Join-Path $env:LOCALAPPDATA 'Atoll\{config_filename}'
try {{
  $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
  $psi = New-Object System.Diagnostics.ProcessStartInfo($config.runner, ('"' + $config.node + '" "' + $config.script + '"'))
  $psi.UseShellExecute = $false
  $psi.RedirectStandardInput = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.CreateNoWindow = $true
  $p = [System.Diagnostics.Process]::Start($psi)
  [Console]::OpenStandardInput().CopyTo($p.StandardInput.BaseStream)
  $p.StandardInput.Close()
  [Console]::Out.Write($p.StandardOutput.ReadToEnd())
  $p.WaitForExit() | Out-Null
  exit $p.ExitCode
}} catch {{
  [Console]::Out.Write('{fallback_stdout}')
  exit 0
}}
"#
    );
    // UTF-8 BOM so Windows PowerShell reads non-ASCII paths from the JSON config reliably.
    let mut ps1_bytes = vec![0xEF, 0xBB, 0xBF];
    ps1_bytes.extend_from_slice(ps1_body.as_bytes());
    std::fs::write(&ps1_path, ps1_bytes)
        .map_err(|error| format!("Cannot write {}: {error}", ps1_path.display()))?;

    Ok(format!(
        "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\"",
        normalize_hook_command_path(&ps1_path.to_string_lossy()).replace('"', "\\\"")
    ))
}

#[cfg(windows)]
fn write_cursor_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    write_windows_hook_launcher_command(
        app,
        node_path,
        script_path,
        "cursor-hook-launcher.json",
        "atoll-cursor-hook.ps1",
        r#"{"permission":"allow"}"#,
    )
}

#[cfg(windows)]
fn write_codex_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    write_windows_hook_launcher_command(
        app,
        node_path,
        script_path,
        "codex-hook-launcher.json",
        "atoll-codex-hook.ps1",
        "{}",
    )
}

#[cfg(not(windows))]
fn write_codex_hook_launcher_command(
    _app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(None, node_path, script_path))
}

#[cfg(not(windows))]
fn write_cursor_hook_launcher_command(
    _app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(None, node_path, script_path))
}

/// Legacy helper kept for tests; production Cursor installs use [`write_cursor_hook_launcher_command`].
#[cfg(windows)]
fn format_cursor_hook_command(
    runner_path: Option<&str>,
    node_path: &str,
    script_path: &str,
) -> String {
    format!(
        "cmd /c {}",
        format_hook_command(runner_path, node_path, script_path)
    )
}

#[cfg(not(windows))]
fn format_cursor_hook_command(
    runner_path: Option<&str>,
    node_path: &str,
    script_path: &str,
) -> String {
    format_hook_command(runner_path, node_path, script_path)
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

fn expand_windows_hook_env(command: &str) -> String {
    let mut result = command.to_string();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        result = result.replace("%LOCALAPPDATA%", &local);
    }
    if let Ok(user) = std::env::var("USERPROFILE") {
        result = result.replace("%USERPROFILE%", &user);
    }
    result
}

fn extract_hook_command_parts(command: &str) -> Option<(String, String)> {
    let mut trimmed = expand_windows_hook_env(command).trim().to_string();
    if let Some(rest) = trimmed
        .strip_prefix("cmd /c ")
        .or_else(|| trimmed.strip_prefix("cmd /C "))
    {
        trimmed = rest.trim().to_string();
    }

    let normalized = normalize_hook_script_path(&trimmed);
    if normalized
        .to_ascii_lowercase()
        .ends_with("atoll-cursor-hook.ps1")
        || normalized
            .to_ascii_lowercase()
            .ends_with("atoll-codex-hook.ps1")
    {
        return parse_hook_launcher_script(&normalized);
    }
    if normalized
        .to_ascii_lowercase()
        .ends_with("atoll-cursor-hook.cmd")
    {
        return parse_cursor_launcher_cmd(&normalized);
    }

    if trimmed.starts_with("powershell ") {
        if let Some(start) = trimmed.find("-File \"") {
            let rest = &trimmed[start + 7..];
            if let Some(end) = rest.find('"') {
                let ps1 = normalize_hook_script_path(&expand_windows_hook_env(&rest[..end]));
                return parse_hook_launcher_script(&ps1);
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix("node ") {
        let script = if rest.starts_with('"') {
            extract_first_quoted_value(rest)?.0
        } else {
            rest.split_whitespace().next()?.to_string()
        };
        return Some(("node".to_string(), normalize_hook_script_path(&script)));
    }

    let (first, rest) = extract_first_quoted_value(trimmed.as_str())?;
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

fn parse_cursor_launcher_cmd(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("@echo off") {
            continue;
        }
        return extract_hook_command_parts(line);
    }
    None
}

fn parse_hook_launcher_config(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let config: Value = serde_json::from_str(&content).ok()?;
    let node = config.get("node").and_then(Value::as_str)?;
    let script = config.get("script").and_then(Value::as_str)?;
    Some((
        normalize_hook_script_path(node),
        normalize_hook_script_path(script),
    ))
}

fn parse_hook_launcher_script(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    for (marker, filename) in [
        ("cursor-hook-launcher.json", "cursor-hook-launcher.json"),
        ("codex-hook-launcher.json", "codex-hook-launcher.json"),
    ] {
        if content.contains(marker) {
            let config_path = std::path::Path::new(path)
                .parent()
                .map(|dir| dir.join(filename))
                .filter(|candidate| candidate.is_file())
                .or_else(|| dirs::data_local_dir().map(|dir| dir.join("Atoll").join(filename)))?;
            return parse_hook_launcher_config(&config_path.to_string_lossy());
        }
    }
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.contains("atoll-hook-runner") {
            continue;
        }
        let line = line
            .strip_prefix("$Input | & ")
            .or_else(|| line.strip_prefix("$input | & "))
            .unwrap_or(line);
        return extract_hook_command_parts(line);
    }
    None
}

fn extract_node_script_path(command: &str) -> Option<String> {
    extract_hook_command_parts(command).map(|(_, script)| script)
}

fn configured_atoll_hook_command(config: &Value, marker: &str) -> Option<String> {
    let hooks = config.get("hooks")?.as_object()?;
    for matchers in hooks.values() {
        let arr = matchers.as_array()?;
        for matcher in arr {
            if let Some(hook_arr) = matcher.get("hooks").and_then(Value::as_array) {
                for hook in hook_arr {
                    let cmd = hook.get("command")?.as_str()?;
                    if cmd.contains(marker) {
                        return Some(cmd.to_string());
                    }
                }
            }
            if let Some(cmd) = matcher.get("command").and_then(Value::as_str) {
                if cmd.contains(marker) {
                    return Some(cmd.to_string());
                }
            }
        }
    }
    None
}

fn configured_atoll_hook_node_path(config: &Value, marker: &str) -> Option<String> {
    configured_atoll_hook_command(config, marker)
        .and_then(|cmd| extract_hook_command_parts(&cmd).map(|(node, _)| node))
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
    configured_atoll_hook_command(config, marker).and_then(|cmd| extract_node_script_path(&cmd))
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

    if let Some(hooks_dir) = atoll_local_hooks_dir() {
        candidates.push(hooks_dir.join(script_name));
    }

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
            if ancestor.file_name().is_some_and(|name| name == "src-tauri") {
                if let Some(repo_root) = ancestor.parent() {
                    candidates.push(repo_root.join("scripts").join(script_name));
                }
            }
            if ancestor.join("src-tauri").exists() {
                candidates.push(ancestor.join("scripts").join(script_name));
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("target")
                        .join("debug")
                        .join("scripts")
                        .join(script_name),
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

#[cfg(windows)]
fn resolve_hook_runner_path(app: &AppHandle) -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Some(hooks_dir) = atoll_local_hooks_dir() {
        candidates.push(hooks_dir.join("atoll-hook-runner.exe"));
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join("atoll-hook-runner.exe"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("atoll-hook-runner.exe"));
            candidates.push(
                exe_dir
                    .join("resources")
                    .join("scripts")
                    .join("atoll-hook-runner.exe"),
            );
            candidates.push(exe_dir.join("scripts").join("atoll-hook-runner.exe"));
        }
        for ancestor in exe.ancestors().skip(1) {
            if ancestor.file_name().is_some_and(|name| name == "src-tauri") {
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
            if ancestor.join("src-tauri").exists() {
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("generated")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("target")
                        .join("debug")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("src-tauri")
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

fn hook_entry_has_atoll_cursor(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(Value::as_str)
        .map(|cmd| {
            cmd.contains("atoll-cursor-hook")
                || cmd.contains("atoll-cursor-hook.ps1")
                || cmd.contains("atoll-cursor-hook.cmd")
        })
        .unwrap_or(false)
}

const CURSOR_HOOK_TIMEOUT_SECONDS: u64 = 5;
const CURSOR_HOOK_EVENTS: [(&str, u64); 10] = [
    ("sessionStart", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("beforeSubmitPrompt", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("afterAgentResponse", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("afterAgentThought", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("sessionEnd", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("preToolUse", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("postToolUse", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("stop", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("subagentStart", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("subagentStop", CURSOR_HOOK_TIMEOUT_SECONDS),
];

const CURSOR_CORE_HOOK_EVENTS: [&str; 5] = [
    "preToolUse",
    "postToolUse",
    "stop",
    "subagentStart",
    "subagentStop",
];

const CURSOR_LIFECYCLE_HOOK_EVENTS: [&str; 5] = [
    "sessionStart",
    "beforeSubmitPrompt",
    "afterAgentResponse",
    "afterAgentThought",
    "sessionEnd",
];

fn upsert_cursor_hook_events(hooks: &mut Value, hook_command: &str, hook_url: &str) {
    let Some(hooks_obj) = hooks.as_object_mut() else {
        return;
    };

    // Composer / Agent Chat hooks only. Tab inline-completion hooks (`beforeTabFileRead`,
    // `afterTabFileEdit`) are intentionally excluded: Tab does not create a Composer
    // session or emit sessionStart, so Atoll cannot attribute usage to a session.
    for (event, timeout) in CURSOR_HOOK_EVENTS {
        let mut merged: Vec<Value> = hooks_obj
            .get(event)
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter(|entry| !hook_entry_has_atoll_cursor(entry))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        merged.push(json!({
            "command": hook_command,
            "timeout": timeout,
            "env": {
                "ATOLL_HOOK_URL": hook_url
            }
        }));
        hooks_obj.insert(event.to_string(), Value::Array(merged));
    }
}

fn remove_atoll_cursor_hooks(hooks: &mut Value) {
    let Some(hooks_obj) = hooks.as_object_mut() else {
        return;
    };

    for entries in hooks_obj.values_mut() {
        if let Some(arr) = entries.as_array_mut() {
            arr.retain(|entry| !hook_entry_has_atoll_cursor(entry));
        }
    }

    hooks_obj.retain(|_, entries| {
        entries
            .as_array()
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    });
}

fn cursor_hooks_have_events(config: &Value, events: &[&str]) -> bool {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    events.iter().all(|event| {
        hooks
            .get(*event)
            .and_then(Value::as_array)
            .map(|arr| arr.iter().any(hook_entry_has_atoll_cursor))
            .unwrap_or(false)
    })
}

fn cursor_hooks_need_lifecycle_upgrade(config: &Value) -> bool {
    has_atoll_cursor_hooks(config)
        && !cursor_hooks_have_events(config, &CURSOR_LIFECYCLE_HOOK_EVENTS)
}

fn cursor_hooks_need_timeout_repair(config: &Value) -> bool {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    hooks.values().any(|entries| {
        entries
            .as_array()
            .map(|arr| {
                arr.iter().any(|entry| {
                    hook_entry_has_atoll_cursor(entry)
                        && entry.get("timeout").and_then(Value::as_u64)
                            != Some(CURSOR_HOOK_TIMEOUT_SECONDS)
                })
            })
            .unwrap_or(false)
    })
}

fn cursor_hook_command_needs_repair(
    command: &str,
    preferred_script_path: Option<&str>,
    require_powershell_launcher: bool,
) -> bool {
    let lower = command.to_ascii_lowercase();
    if require_powershell_launcher
        && !(lower.starts_with("powershell ")
            && lower.contains("atoll-cursor-hook.ps1")
            && lower.contains("-file "))
    {
        return true;
    }

    let Some((_node, script)) = extract_hook_command_parts(command) else {
        return true;
    };

    if let Some(preferred) = preferred_script_path {
        if should_flag_dev_hook_drift(&script, preferred) {
            return true;
        }
    }

    !std::path::Path::new(&script).is_file()
}

fn cursor_hooks_need_command_repair(
    config: &Value,
    preferred_script_path: Option<&str>,
    require_powershell_launcher: bool,
) -> bool {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    hooks.values().any(|entries| {
        entries
            .as_array()
            .map(|arr| {
                arr.iter().any(|entry| {
                    entry
                        .get("command")
                        .and_then(Value::as_str)
                        .filter(|command| {
                            hook_entry_has_atoll_cursor(entry)
                                && cursor_hook_command_needs_repair(
                                    command,
                                    preferred_script_path,
                                    require_powershell_launcher,
                                )
                        })
                        .is_some()
                })
            })
            .unwrap_or(false)
    })
}

fn repair_cursor_hook_events_with_command(
    config: &Value,
    hook_command: &str,
    hook_url: &str,
) -> Option<Value> {
    let mut repaired = config.clone();
    if repaired.get("version").is_none() {
        if let Some(obj) = repaired.as_object_mut() {
            obj.insert("version".to_string(), json!(1));
        }
    }
    let hooks_obj = repaired
        .as_object_mut()?
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    upsert_cursor_hook_events(hooks_obj, hook_command, hook_url);
    Some(repaired)
}

fn preferred_cursor_hook_command(
    app: &AppHandle,
    source_script_path: &str,
) -> Result<(String, String), String> {
    let script_path =
        materialize_hook_deployment(app, "atoll-cursor-hook.mjs", source_script_path)?;
    let node_path = resolve_node_executable()?;
    let hook_command = write_cursor_hook_launcher_command(app, &node_path, &script_path)?;
    Ok((hook_command, script_path))
}

fn maybe_repair_cursor_hook_events(
    app: &AppHandle,
    hooks_path: &str,
    config: Option<&Value>,
    hook_url: &str,
) -> Option<Value> {
    let config = config?;
    if !has_atoll_cursor_hooks(config) {
        return None;
    }

    let source_script_path = resolve_install_hook_script_path(app, "atoll-cursor-hook.mjs").ok()?;
    let preferred_script_path = deployed_hook_script_path("atoll-cursor-hook.mjs")
        .unwrap_or_else(|| source_script_path.clone());
    let needs_repair = cursor_hooks_need_lifecycle_upgrade(config)
        || cursor_hooks_need_timeout_repair(config)
        || cursor_hooks_need_command_repair(config, Some(&preferred_script_path), cfg!(windows));
    if !needs_repair {
        return None;
    }

    let (hook_command, _script_path) =
        preferred_cursor_hook_command(app, &source_script_path).ok()?;
    let repaired = repair_cursor_hook_events_with_command(config, &hook_command, hook_url)?;
    let formatted = serde_json::to_string_pretty(&repaired).ok()?;
    if std::fs::write(hooks_path, formatted).is_err() {
        return None;
    }
    eprintln!("Atoll repaired Cursor hooks with current launcher command");
    Some(repaired)
}

/// Returns true when Atoll's Cursor hooks are installed.
///
/// Only the core Composer/Agent events that shipped with v0.1.31
/// (`preToolUse`, `postToolUse`, `stop`, `subagentStart`, `subagentStop`) are
/// required. The v0.1.32 lifecycle hooks (`sessionStart`, `beforeSubmitPrompt`,
/// `afterAgentResponse`, `afterAgentThought`, `sessionEnd`) are an optional
/// enhancement for Ask/Composer-mode session tracking: users who installed
/// hooks with v0.1.31 only have the core five, and treating them as
/// "not installed" regresses session display and the online indicator. Those
/// users keep working; hook status repair can add the new events in place.
fn has_atoll_cursor_hooks(config: &Value) -> bool {
    cursor_hooks_have_events(config, &CURSOR_CORE_HOOK_EVENTS)
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
            last_subagent_reconcile: Mutex::new(Instant::now() - Duration::from_secs(10)),
            last_hook_activity: Mutex::new(Instant::now()),
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
            uses_micro_island,
            get_claude_hook_status,
            install_claude_hooks,
            uninstall_claude_hooks,
            get_codex_hook_status,
            install_codex_hooks,
            uninstall_codex_hooks,
            get_cursor_hook_status,
            install_cursor_hooks,
            uninstall_cursor_hooks,
            get_session_retention,
            set_session_retention,
            get_subagent_retention,
            set_subagent_retention,
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
                *lock_state(&state.session_retention_secs) = retention;
                let sub_retention = load_persisted_subagent_retention_secs();
                *lock_state(&state.subagent_retention_secs) = sub_retention;
            }
            start_auto_archive_timer(app.handle().clone());
            start_token_refresh_timer(app.handle().clone());
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
                #[cfg(target_os = "windows")]
                platform::show_island_on_top(&window);
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
        #[cfg(target_os = "windows")]
        maybe_reassert_island_on_top(&app);
    });
}

#[cfg(target_os = "windows")]
fn maybe_reassert_island_on_top(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let window_for_thread = window.clone();
    let window_for_closure = window.clone();
    let _ = window_for_thread.run_on_main_thread(move || {
        platform::ensure_island_on_top(&window_for_closure);
    });
}

fn start_token_refresh_timer(app: AppHandle) {
    thread::spawn(move || {
        let mut last_snapshot_emit = Instant::now() - TOKEN_SNAPSHOT_MIN_INTERVAL;

        loop {
            let state = app.state::<AppState>();
            let tracked_sessions = {
                let requests = state.requests.lock().unwrap_or_else(|e| e.into_inner());
                let known_sessions = state
                    .known_sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                collect_session_transcript_paths(&requests, &known_sessions)
            };

            let sleep_duration = if tracked_sessions.is_empty() {
                TOKEN_REFRESH_INTERVAL_IDLE
            } else {
                let recently_active = state
                    .last_hook_activity
                    .lock()
                    .map(|last| last.elapsed() < HOOK_ACTIVITY_IDLE_THRESHOLD)
                    .unwrap_or(true);
                if recently_active {
                    TOKEN_REFRESH_INTERVAL_ACTIVE
                } else {
                    TOKEN_REFRESH_INTERVAL_IDLE
                }
            };
            thread::sleep(sleep_duration);

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
        let mut last_cursor_over = false;
        let mut last_client: Option<(f64, f64)> = None;
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

            let cursor_over_changed = cursor_over_window != last_cursor_over;
            let hover_changed = hovering != last_hovering;
            let client_changed = hovering && client != last_client;
            if cursor_over_changed || hover_changed || client_changed {
                let _ = app.emit(
                    "island-hover-changed",
                    IslandHoverChanged {
                        hovering,
                        cursor_over_window,
                        client_x: if hovering {
                            client.map(|(x, _)| x)
                        } else {
                            None
                        },
                        client_y: if hovering {
                            client.map(|(_, y)| y)
                        } else {
                            None
                        },
                    },
                );
                last_cursor_over = cursor_over_window;
                last_hovering = hovering;
                last_client = if hovering { client } else { None };
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
        false,
        false,
    ))?;
    platform::set_island_cursor_events_ignored(window, is_collapsed_pass_through_mode(mode));

    let window_size = island_window_physical_size(
        mode,
        scale_factor,
        compact_width,
        notch,
        false,
        false,
        false,
    );
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
    platform::ensure_island_on_top(window);
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
    expanded_plan: bool,
    expanded_settings: bool,
) -> tauri::Result<()> {
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    platform::ensure_island_on_top(window);
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
    let target_size = island_window_physical_size(
        mode,
        scale_factor,
        compact_width,
        notch,
        expanded_idle,
        expanded_plan,
        expanded_settings,
    );
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
            // #region agent log
            #[cfg(target_os = "windows")]
            if matches!(mode, IslandWindowMode::Expanded) {
                crate::debug_agent::log(
                    "H-B",
                    "lib.rs:animate_island_window_mode",
                    "expand animation finished",
                    serde_json::json!({
                        "mode": format!("{:?}", mode),
                        "targetW": target_size.width,
                        "targetH": target_size.height,
                        "alwaysOnTopAtEnd": window.is_always_on_top().unwrap_or(false),
                    }),
                );
            }
            // #endregion
            platform::ensure_island_on_top(window);
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

fn expanded_window_width(expanded_plan: bool, expanded_settings: bool) -> f64 {
    if expanded_plan {
        EXPANDED_PLAN_WINDOW_WIDTH
    } else if expanded_settings {
        EXPANDED_SETTINGS_WINDOW_WIDTH
    } else {
        EXPANDED_WINDOW_WIDTH
    }
}

fn expanded_window_height(
    expanded_idle: bool,
    expanded_plan: bool,
    expanded_settings: bool,
) -> f64 {
    if expanded_plan {
        EXPANDED_PLAN_WINDOW_HEIGHT
    } else if expanded_settings {
        EXPANDED_SETTINGS_WINDOW_HEIGHT
    } else if expanded_idle {
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
    expanded_plan: bool,
    expanded_settings: bool,
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
            let w = expanded_window_width(expanded_plan, expanded_settings).max(min_notch_width);
            LogicalSize::new(
                w,
                expanded_window_height(expanded_idle, expanded_plan, expanded_settings) + extra_top,
            )
        }
    }
}

fn island_window_physical_size(
    mode: IslandWindowMode,
    scale_factor: f64,
    compact_width: f64,
    notch: NotchMetrics,
    expanded_idle: bool,
    expanded_plan: bool,
    expanded_settings: bool,
) -> PhysicalSize<u32> {
    let logical_size = island_window_logical_size(
        mode,
        compact_width,
        notch,
        expanded_idle,
        expanded_plan,
        expanded_settings,
    );

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

        let messages =
            read_transcript_messages(&transcript_path.to_string_lossy()).expect("read messages");

        assert_eq!(messages.len(), TRANSCRIPT_MAX_MESSAGES);
        assert_eq!(messages[0].content, "message 25");
        assert_eq!(messages[TRANSCRIPT_MAX_MESSAGES - 1].content, "message 74");

        let _ = std::fs::remove_dir_all(dir);
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
            last_subagent_reconcile: Mutex::new(Instant::now() - Duration::from_secs(10)),
            last_hook_activity: Mutex::new(Instant::now()),
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
        let compact = island_window_logical_size(IslandWindowMode::Compact, 132.0, notch, false, false, false);
        // Compact sits in the menu-bar band (like dormant) — no extra_top.
        assert_eq!(compact.height, COMPACT_WINDOW_HEIGHT);
        // Width is clamped up to the notch width so the capsule visually
        // fuses with the camera housing (Dynamic-Island style).
        assert_eq!(compact.width, 200.0);

        // Content wider than the notch keeps its own width.
        let wide = island_window_logical_size(IslandWindowMode::Compact, 300.0, notch, false, false, false);
        assert_eq!(wide.width, 300.0);

        // Dormant is slightly wider than the notch (padding on each side).
        let dormant = island_window_logical_size(IslandWindowMode::Dormant, 132.0, notch, false, false, false);
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
        let compact = island_window_logical_size(IslandWindowMode::Compact, 132.0, no_notch, false, false, false);
        assert_eq!(compact.width, 132.0);
        assert_eq!(compact.height, COMPACT_WINDOW_HEIGHT);

        // A compact_width that already exceeds the floor is kept as-is.
        let wide = island_window_logical_size(IslandWindowMode::Compact, 250.0, no_notch, false, false, false);
        assert_eq!(wide.width, 250.0);

        // Dormant: uses the same FALLBACK_NOTCH_WIDTH reference + padding.
        let dormant = island_window_logical_size(IslandWindowMode::Dormant, 132.0, no_notch, false, false, false);
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
            false,
            false,
        );
        assert_eq!(micro.width, 132.0);
        assert_eq!(micro.height, MICRO_WINDOW_HEIGHT);
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
            last_subagent_reconcile: Mutex::new(Instant::now() - Duration::from_secs(10)),
            last_hook_activity: Mutex::new(Instant::now()),
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
