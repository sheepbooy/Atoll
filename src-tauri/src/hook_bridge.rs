use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use socket2::{Domain, Socket, Type};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    approval_history, approval_notice_is_notify, build_snapshot, complete_subagent,
    cursor_lifecycle_token_seen, cursor_payload_has_token_usage, emit_subagent_snapshot,
    get_stored_session_host, ingest_cursor_token_usage_from_payload, is_codex_internal_session,
    iso_timestamp_now, payload_subagent_id, payload_subagent_parent_session_id, platform,
    purge_tracked_session, refresh_session_token_usage, register_known_session,
    register_subagent_start, remember_cursor_lifecycle_token_session, resolve_codex_session_cwd,
    resolve_cursor_session_for_payload, roll_over_token_usage_if_needed,
    schedule_observer_snapshot_emit, send_approval_notification, show_island_quietly,
    show_main_window_for_approval, touch_hook_activity, touch_session_activity, AgentKind,
    AppState, Decision, DecisionWithNote, PermissionRequest, PermissionStatus,
};

pub(crate) const DEFAULT_HOOK_PORT: u16 = 47_777;
mod host_detect;
mod http;
mod payloads;
mod pending;
mod responses;
mod routes;
mod server;

pub(crate) use host_detect::*;
pub(crate) use http::*;
pub(crate) use payloads::*;
pub(crate) use pending::*;
pub(crate) use responses::*;
pub(crate) use routes::*;
pub(crate) use server::*;

const HOOK_BIND_HOST: &str = "127.0.0.1";
const HOOK_BIND_RETRY_COUNT: u32 = 5;
const HOOK_BIND_RETRY_DELAY: Duration = Duration::from_millis(500);
const HOOK_FALLBACK_PORT_START: u16 = 47_778;
const HOOK_FALLBACK_PORT_END: u16 = 47_827;
/// WSL/Hyper-V often reserves ~47000–48789 on Windows; try outside that block next.
const HOOK_SECONDARY_FALLBACK_START: u16 = 48_800;
const HOOK_SECONDARY_FALLBACK_END: u16 = 48_850;
const HOOK_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const HOOK_REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);
const HOOK_RESPONSE_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const BRIDGE_PROBE_TIMEOUT: Duration = Duration::from_millis(200);
const BRIDGE_ONLINE_GRACE: Duration = Duration::from_secs(3);
const HOOK_POLL_INTERVAL: Duration = Duration::from_millis(180);
const HOOK_AUTH_HEADER: &str = "x-atoll-hook-token";
const MAX_HOOK_WAITERS: usize = 96;
const MAX_INFLIGHT_CONNECTIONS: usize = 128;
const MAX_HOOK_REQUEST_LINE_BYTES: usize = 8 * 1024;
const MAX_HOOK_HEADER_BYTES: usize = 32 * 1024;
const MAX_HOOK_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_PERMISSION_TOOL_INPUT_BYTES: usize = 512 * 1024;
const MAX_PERMISSION_LABEL_CHARS: usize = 32 * 1024;
const OBSERVER_QUEUE_CAPACITY: usize = 256;
const MAX_RESOLVED_REQUESTS: usize = 4096;
const MAX_RESOLVED_REQUESTS_PER_SESSION: usize = 256;
