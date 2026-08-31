//! Shared application state, cross-module data types, and window/notch
//! geometry constants. Every item is re-exported from the crate root so the
//! sibling modules can keep referencing `crate::AppState` etc.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{LogicalPosition, PhysicalSize};

use crate::{
    clipboard_history, lyrics, platform, shortcuts, transcript, HookHealthSnapshot, TranscriptCache,
};

pub(crate) const COMPACT_WINDOW_WIDTH: f64 = 132.0;
pub(crate) const COMPACT_WINDOW_HEIGHT: f64 = 36.0;
/// Windows-only super-collapsed strip; macOS never selects this mode.
pub(crate) const MICRO_WINDOW_WIDTH: f64 = 72.0;
pub(crate) const MICRO_WINDOW_HEIGHT: f64 = 24.0;
pub(crate) const EXPANDED_WINDOW_WIDTH: f64 = 560.0;
pub(crate) const EXPANDED_WINDOW_HEIGHT: f64 = 320.0;
pub(crate) const EXPANDED_IDLE_WINDOW_HEIGHT: f64 = 240.0;
pub(crate) const EXPANDED_PLAN_WINDOW_WIDTH: f64 = 680.0;
pub(crate) const EXPANDED_PLAN_WINDOW_HEIGHT: f64 = 680.0;
pub(crate) const EXPANDED_SETTINGS_WINDOW_WIDTH: f64 = 680.0;
pub(crate) const EXPANDED_SETTINGS_WINDOW_HEIGHT: f64 = 680.0;
pub(crate) const MIN_COMPACT_WINDOW_WIDTH: f64 = 72.0;
// Dormant pill height (width spans the notch + side padding on notched displays).
pub(crate) const DORMANT_WINDOW_HEIGHT: f64 = 36.0;
// Extra width beyond the notch on each side so edges are visible.
pub(crate) const DORMANT_NOTCH_PADDING: f64 = 30.0;
pub(crate) const MAX_ACTIVE_SUBAGENTS: usize = 512;
pub(crate) const WINDOW_ANIMATION_DURATION: Duration = Duration::from_millis(420);

pub(crate) fn lock_state<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
pub(crate) static TOKEN_HISTORY_ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) static APPROVAL_HISTORY_ENV_LOCK: Mutex<()> = Mutex::new(());

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
pub(crate) const NOTCH_COVER_PADDING: f64 = 16.0;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PermissionRequest {
    pub(crate) id: String,
    pub(crate) tool_use_id: Option<String>,
    pub(crate) agent: AgentKind,
    pub(crate) session: String,
    pub(crate) command: String,
    pub(crate) detail: String,
    pub(crate) cwd: String,
    pub(crate) requested_at: String,
    pub(crate) status: PermissionStatus,
    #[serde(default)]
    pub(crate) archived: bool,
    #[serde(default)]
    pub(crate) supports_always: bool,
    #[serde(default)]
    pub(crate) transcript_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool_input: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IslandSnapshot {
    pub(crate) online: bool,
    pub(crate) pending_count: usize,
    pub(crate) archived_count: usize,
    pub(crate) active_request: Option<PermissionRequest>,
    pub(crate) recent: Vec<PermissionRequest>,
    pub(crate) sessions: Vec<SessionSummary>,
    pub(crate) daily_tokens: TokenUsage,
    pub(crate) active_session_tokens: TokenUsage,
    #[serde(default)]
    pub(crate) daily_tokens_by_model: HashMap<String, TokenUsage>,
    #[serde(default)]
    pub(crate) active_session_tokens_by_model: HashMap<String, TokenUsage>,
    pub(crate) hook_health: HookHealthSnapshot,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveSubagent {
    pub(crate) agent_id: String,
    pub(crate) session_id: String,
    pub(crate) agent_kind: AgentKind,
    pub(crate) agent_type: String,
    pub(crate) started_at: String,
    pub(crate) agent_transcript_path: Option<String>,
    pub(crate) completed_at: Option<String>,
    #[serde(default)]
    pub(crate) archived: bool,
    pub(crate) last_message: Option<String>,
    /// Cursor subagent's independent conversation_id (bound on first preToolUse).
    pub(crate) conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubagentSummary {
    pub(crate) agent_id: String,
    pub(crate) agent_type: String,
    pub(crate) started_at: String,
    pub(crate) agent_transcript_path: Option<String>,
    pub(crate) completed_at: Option<String>,
    #[serde(default)]
    pub(crate) archived: bool,
    pub(crate) last_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionSummary {
    pub(crate) session_id: String,
    pub(crate) agent: AgentKind,
    pub(crate) cwd: String,
    pub(crate) pending_count: usize,
    pub(crate) total_count: usize,
    pub(crate) last_activity: String,
    pub(crate) transcript_path: Option<String>,
    #[serde(default)]
    pub(crate) pinned: bool,
    #[serde(default)]
    pub(crate) session_host: platform::SessionHost,
    #[serde(default)]
    pub(crate) active_subagents: Vec<SubagentSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenUsage {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) cache_creation_tokens: u64,
}

impl TokenUsage {
    pub(crate) fn is_zero(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.cache_read_tokens == 0
            && self.cache_creation_tokens == 0
    }

    pub(crate) fn add_assign(&mut self, other: TokenUsage) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(other.cache_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
    }

    pub(crate) fn component_wise_max(self, other: TokenUsage) -> TokenUsage {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AgentKind {
    Claude,
    Codex,
    Cursor,
    Zcode,
    Gemini,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PermissionStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Decision {
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IslandWindowMode {
    Micro,
    Dormant,
    Compact,
    Expanded,
}

/// Re-exported so `get_now_playing` can return the type on all platforms.
/// All three definitions are field-for-field identical (serde camelCase).
#[cfg(target_os = "macos")]
pub(crate) use crate::media::NowPlayingTrack;

#[cfg(target_os = "windows")]
pub(crate) use media_windows::NowPlayingTrack;

/// Stub for platforms without a media source (e.g. Linux) so the command
/// signature compiles without the platform media modules.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NowPlayingTrack {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub position: Option<f64>,
    pub playing: bool,
    pub artwork_base64: Option<String>,
    pub app: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IslandHoverChanged {
    pub(crate) hovering: bool,
    pub(crate) cursor_over_window: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) client_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) client_y: Option<f64>,
}

pub(crate) struct DecisionWithNote {
    pub(crate) decision: Decision,
    pub(crate) note: String,
    pub(crate) updated_input: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KnownSession {
    pub(crate) agent: AgentKind,
    pub(crate) cwd: String,
    pub(crate) transcript_path: Option<String>,
    pub(crate) last_activity: String,
    #[serde(default)]
    pub(crate) host: platform::SessionHost,
    /// Full Cursor composer UUID when the session key is a short hook id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) conversation_id: Option<String>,
}

/// Shared application state.
///
/// Lock ordering for code that must hold more than one mutex at a time:
/// requests -> known_sessions -> pinned_sessions -> session/token maps ->
/// hook_waiters -> active_subagents -> UI/window metrics. Prefer cloning the
/// minimum data and dropping each guard before acquiring the next lock.
pub(crate) struct AppState {
    pub(crate) requests: Mutex<Vec<PermissionRequest>>,
    pub(crate) session_request_totals: Mutex<HashMap<String, usize>>,
    pub(crate) hook_waiters: Mutex<HashMap<String, SyncSender<DecisionWithNote>>>,
    pub(crate) auto_approve_sessions: Mutex<HashSet<String>>,
    pub(crate) compact_width: Mutex<f64>,
    pub(crate) compact_left_width: Mutex<f64>,
    pub(crate) presentation_generation: Arc<AtomicU64>,
    pub(crate) home_bounds: Mutex<Option<HomeWindowBounds>>,
    pub(crate) notch_metrics: Mutex<NotchMetrics>,
    pub(crate) session_last_seen: Mutex<HashMap<String, u64>>,
    pub(crate) session_retention_secs: Mutex<u64>,
    pub(crate) subagent_retention_secs: Mutex<u64>,
    pub(crate) session_token_usage: Mutex<HashMap<String, TokenUsage>>,
    pub(crate) session_token_usage_by_model: Mutex<HashMap<String, HashMap<String, TokenUsage>>>,
    /// Sticky session → agent mapping that survives session purges within a day.
    pub(crate) session_agent_map: Mutex<HashMap<String, String>>,
    pub(crate) token_usage_file_offsets: Mutex<HashMap<String, u64>>,
    pub(crate) token_usage_day: Mutex<String>,
    /// Today's persisted total loaded at process start (and after midnight rollover).
    /// Hook increments add on top until transcript full-scans produce absolute totals.
    pub(crate) startup_daily_floor: Mutex<TokenUsage>,
    /// Today's persisted per-model totals loaded at process start (cost-mode floor).
    pub(crate) startup_daily_floor_by_model: Mutex<HashMap<String, TokenUsage>>,
    /// Sessions whose in-memory totals came from a transcript full-scan (absolute).
    pub(crate) absolute_token_sessions: Mutex<HashSet<String>>,
    /// High-water mark synced to token_history.json; never regresses within a day.
    pub(crate) daily_tokens_baseline: Mutex<TokenUsage>,
    pub(crate) known_sessions: Mutex<HashMap<String, KnownSession>>,
    pub(crate) pinned_sessions: Mutex<HashSet<String>>,
    /// Platform-specific focus restore target (macOS pid / Windows HWND).
    pub(crate) previous_app_pid: Mutex<Option<i64>>,
    /// Last emitted listening-online flag; used to push snapshot updates when hook/bridge health changes.
    pub(crate) last_listening_online: Mutex<Option<bool>>,
    /// Last emitted hook-health snapshot; used to detect external config drift.
    pub(crate) last_hook_health: Mutex<Option<HookHealthSnapshot>>,
    /// Local hook bridge TCP port (0 until bound).
    pub(crate) bridge_port: AtomicU16,
    /// Per-process bearer token shared with local hook runners through bridge.json.
    pub(crate) bridge_auth_token: Mutex<String>,
    /// Last time the hook bridge accepted a TCP probe; used for offline grace during rebind.
    pub(crate) last_bridge_reachable: Mutex<Option<Instant>>,
    pub(crate) active_subagents: Mutex<Vec<ActiveSubagent>>,
    /// Maps Cursor subagent conversation_id → parent session_id.
    pub(crate) cursor_subagent_conversations: Mutex<HashMap<String, String>>,
    /// Cursor sessions that already produced token usage from lifecycle hooks.
    pub(crate) cursor_lifecycle_token_sessions: Mutex<HashSet<String>>,
    /// Rate-limiter for SubagentStart/SubagentStop snapshot emissions.
    pub(crate) last_subagent_snapshot_emit: Mutex<Instant>,
    /// Debounce generation for Cursor observer snapshot emits.
    pub(crate) snapshot_debounce_generation: AtomicU64,
    pub(crate) snapshot_debounce_worker_running: AtomicBool,
    /// Rate-limiter for subagent transcript reconciliation in build_snapshot.
    pub(crate) last_subagent_reconcile: Mutex<Instant>,
    /// Last hook HTTP activity; used to back off token refresh when idle.
    pub(crate) last_hook_activity: Mutex<Instant>,
    /// Coalesces token-history persistence so snapshot and hook hot paths never write files.
    pub(crate) token_history_dirty: AtomicBool,
    pub(crate) transcript_cache: Mutex<TranscriptCache>,
    /// Whether the Now Playing media card is shown in the idle island.
    pub(crate) media_card_enabled: Mutex<bool>,
    /// Whether the expanded island grows the now-playing artwork into a frosted backdrop.
    pub(crate) artwork_backdrop_enabled: Mutex<bool>,
    /// Clipboard history entries (pruned, newest first).
    pub(crate) clipboard_history: Mutex<Vec<clipboard_history::ClipboardEntry>>,
    /// Whether clipboard history monitoring is enabled (privacy toggle).
    pub(crate) clipboard_history_enabled: Mutex<bool>,
    /// Maximum number of clipboard entries kept (user setting).
    pub(crate) clipboard_history_limit: Mutex<usize>,
    /// Whether the scrolling-lyrics marquee is enabled in the compact island.
    pub(crate) lyrics_enabled: Mutex<bool>,
    /// Current lyrics payload (None when no track or no synced lyrics).
    pub(crate) lyrics: Mutex<Option<lyrics::LyricPayload>>,
    /// Dedup key (`artist|title`) so we don't refetch the same track.
    pub(crate) lyrics_track_key: Mutex<String>,
    /// How new permission requests grab attention: "interrupt" (expand island
    /// and steal focus) or "notify" (system notification, no focus steal).
    pub(crate) approval_notice_mode: Mutex<String>,
    /// UI language used for notification copy ("en" or "zh-CN").
    pub(crate) notification_language: Mutex<String>,
    /// Global shortcut config plus the errors from the last registration
    /// attempt (startup or settings change).
    pub(crate) global_shortcuts: Mutex<shortcuts::GlobalShortcutsState>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HomeWindowBounds {
    pub(crate) position: LogicalPosition<f64>,
    pub(crate) compact_size: PhysicalSize<u32>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) monitor_top_y: f64,
    pub(crate) monitor_center_x: f64,
    pub(crate) notch: NotchMetrics,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) screen_geometry: Option<platform::ScreenGeometry>,
}

/// Camera-housing ("notch") geometry for the display the island lives on, in
/// logical points. On non-notched displays `has_notch` is false and the island
/// keeps its original top-edge layout.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NotchMetrics {
    pub(crate) has_notch: bool,
    pub(crate) width: f64,
    pub(crate) height: f64,
    #[serde(default)]
    pub(crate) left_area_width: f64,
    #[serde(default)]
    pub(crate) right_area_width: f64,
}
