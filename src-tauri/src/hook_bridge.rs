use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use socket2::{Domain, Socket, Type};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    build_snapshot, complete_subagent, emit_subagent_snapshot,
    ingest_cursor_stop_token_usage, is_codex_internal_session, iso_timestamp_now, platform,
    purge_tracked_session, refresh_session_token_usage, register_known_session,
    register_subagent_start, resolve_codex_session_cwd, roll_over_token_usage_if_needed,
    show_main_window_for_approval, touch_session_activity, AgentKind, AppState, Decision,
    DecisionWithNote, PermissionRequest, PermissionStatus,
};

pub(crate) const DEFAULT_HOOK_PORT: u16 = 47_777;
const HOOK_BIND_HOST: &str = "127.0.0.1";
const HOOK_BIND_RETRY_COUNT: u32 = 5;
const HOOK_BIND_RETRY_DELAY: Duration = Duration::from_millis(500);
const HOOK_FALLBACK_PORT_START: u16 = 47_778;
const HOOK_FALLBACK_PORT_END: u16 = 47_827;
const HOOK_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const BRIDGE_PROBE_TIMEOUT: Duration = Duration::from_millis(200);
const HOOK_POLL_INTERVAL: Duration = Duration::from_millis(180);

pub(crate) fn bridge_config_path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return dirs::data_dir().map(|dir| dir.join("Atoll").join("bridge.json"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        return dirs::data_local_dir().map(|dir| dir.join("Atoll").join("bridge.json"));
    }
}

pub(crate) fn write_bridge_config(port: u16) -> std::io::Result<()> {
    let path = bridge_config_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "bridge config path"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let config = json!({
        "port": port,
        "claudeUrl": format!("http://{HOOK_BIND_HOST}:{port}/claude/pre-tool-use"),
        "codexUrl": format!("http://{HOOK_BIND_HOST}:{port}/codex/hook"),
        "cursorUrl": format!("http://{HOOK_BIND_HOST}:{port}/cursor/hook"),
    });
    std::fs::write(path, serde_json::to_string_pretty(&config)?)
}

pub(crate) fn refresh_bridge_config_file(app: &AppHandle) -> std::io::Result<()> {
    let port = app.state::<AppState>().bridge_port.load(Ordering::SeqCst);
    if port == 0 {
        return Ok(());
    }
    write_bridge_config(port)
}

fn bind_listener_on_port(port: u16) -> std::io::Result<TcpListener> {
    let addr: SocketAddr = format!("{HOOK_BIND_HOST}:{port}")
        .parse()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    socket.listen(128)?;
    Ok(socket.into())
}

fn bind_hook_listener() -> std::io::Result<(TcpListener, u16)> {
    for attempt in 0..HOOK_BIND_RETRY_COUNT {
        match bind_listener_on_port(DEFAULT_HOOK_PORT) {
            Ok(listener) => return Ok((listener, DEFAULT_HOOK_PORT)),
            Err(error) if attempt + 1 < HOOK_BIND_RETRY_COUNT => {
                eprintln!(
                    "Atoll hook bridge bind attempt {} on {DEFAULT_HOOK_PORT} failed: {error}",
                    attempt + 1
                );
                thread::sleep(HOOK_BIND_RETRY_DELAY);
            }
            Err(error) => {
                eprintln!(
                    "Atoll hook bridge failed to bind {HOOK_BIND_HOST}:{DEFAULT_HOOK_PORT} after {HOOK_BIND_RETRY_COUNT} attempts: {error}"
                );
            }
        }
    }

    for port in HOOK_FALLBACK_PORT_START..=HOOK_FALLBACK_PORT_END {
        if let Ok(listener) = bind_listener_on_port(port) {
            eprintln!(
                "Atoll hook bridge using fallback port {port} ({DEFAULT_HOOK_PORT} unavailable)"
            );
            return Ok((listener, port));
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AddrInUse,
        format!("no available hook bridge port in {DEFAULT_HOOK_PORT}..{HOOK_FALLBACK_PORT_END}"),
    ))
}

fn bridge_socket_addr(port: u16) -> Option<SocketAddr> {
    format!("{HOOK_BIND_HOST}:{port}").parse().ok()
}

fn bridge_port_from_config_file() -> Option<u16> {
    let path = bridge_config_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;
    value
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
}

fn refresh_listening_snapshot(app: &AppHandle) {
    let state = app.state::<AppState>();
    let snapshot = build_snapshot(app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    let _ = app.emit("snapshot-changed", &snapshot);
}

/// True when the local hook bridge accepts TCP connections on its bind address.
pub(crate) fn is_bridge_reachable(app: &AppHandle) -> bool {
    let stored_port = app.state::<AppState>().bridge_port.load(Ordering::SeqCst);
    let mut ports = Vec::new();
    if stored_port != 0 {
        ports.push(stored_port);
    }
    if !ports.contains(&DEFAULT_HOOK_PORT) {
        ports.push(DEFAULT_HOOK_PORT);
    }
    if let Some(config_port) = bridge_port_from_config_file() {
        if !ports.contains(&config_port) {
            ports.push(config_port);
        }
    }

    ports.into_iter().any(|port| {
        bridge_socket_addr(port)
            .and_then(|addr| TcpStream::connect_timeout(&addr, BRIDGE_PROBE_TIMEOUT).ok())
            .is_some()
    })
}

pub(crate) fn start_server(app: AppHandle) {
    thread::spawn(move || {
        let listener = match bind_hook_listener() {
            Ok((listener, port)) => {
                app.state::<AppState>()
                    .bridge_port
                    .store(port, Ordering::SeqCst);
                if let Err(error) = write_bridge_config(port) {
                    eprintln!("Atoll hook bridge failed to write bridge.json: {error}");
                } else {
                    eprintln!("Atoll hook bridge listening on {HOOK_BIND_HOST}:{port}");
                }
                refresh_listening_snapshot(&app);
                listener
            }
            Err(error) => {
                eprintln!("Atoll hook bridge failed to bind any port: {error}");
                refresh_listening_snapshot(&app);
                return;
            }
        };

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app = app.clone();
                    thread::spawn(move || handle_connection(app, stream));
                }
                Err(error) => eprintln!("Atoll hook bridge connection failed: {error}"),
            }
        }
    });
}

pub(crate) fn permission_request_from_claude_payload(
    id: String,
    payload: Value,
    requested_at: String,
) -> Option<PermissionRequest> {
    let event_name = payload.get("hook_event_name")?.as_str()?;
    if !matches!(event_name, "PreToolUse" | "PermissionRequest") {
        return None;
    }

    permission_request_from_tool_payload(id, payload, requested_at, AgentKind::Claude, true)
}

pub(crate) fn permission_request_from_codex_payload(
    id: String,
    payload: Value,
    requested_at: String,
) -> Option<PermissionRequest> {
    let event_name = payload.get("hook_event_name")?.as_str()?;
    if event_name != "PermissionRequest" {
        return None;
    }

    let transcript_path = payload_transcript_path(&payload);
    let cwd = resolve_codex_session_cwd(
        payload.get("cwd").and_then(Value::as_str).unwrap_or("."),
        transcript_path,
    );
    if is_codex_internal_session(&AgentKind::Codex, &cwd, None) {
        return None;
    }

    let mut request =
        permission_request_from_tool_payload(id, payload, requested_at, AgentKind::Codex, false)?;
    request.cwd = cwd;
    Some(request)
}

#[allow(dead_code)]
pub(crate) fn permission_request_from_cursor_payload(
    id: String,
    payload: Value,
    requested_at: String,
) -> Option<PermissionRequest> {
    let event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("preToolUse");
    if event_name != "preToolUse" {
        return None;
    }

    let resolved_cwd = resolve_cursor_cwd(&payload);
    let mut request =
        permission_request_from_tool_payload(id, payload, requested_at, AgentKind::Cursor, false)?;
    request.cwd = resolved_cwd;
    Some(request)
}

fn permission_request_from_tool_payload(
    id: String,
    payload: Value,
    requested_at: String,
    agent: AgentKind,
    supports_always_from_suggestions: bool,
) -> Option<PermissionRequest> {
    let tool_name = payload.get("tool_name")?.as_str()?.to_string();
    let tool_input = payload.get("tool_input").cloned().unwrap_or(Value::Null);
    let command = command_label(&tool_name, &tool_input);
    let detail = detail_label(&tool_name, &tool_input);
    let default_session = match agent {
        AgentKind::Codex => "codex",
        AgentKind::Cursor => "cursor",
        _ => "claude-code",
    };

    let supports_always = if supports_always_from_suggestions {
        payload
            .get("permission_suggestions")
            .and_then(Value::as_array)
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    } else {
        false
    };

    Some(PermissionRequest {
        id,
        tool_use_id: payload
            .get("tool_use_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        agent,
        session: payload_session_id(&payload)
            .unwrap_or(default_session)
            .to_string(),
        command,
        detail,
        cwd: payload
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_string(),
        requested_at,
        status: PermissionStatus::Pending,
        archived: false,
        supports_always,
        transcript_path: payload
            .get("transcript_path")
            .and_then(Value::as_str)
            .map(str::to_string),
        tool_input: payload.get("tool_input").and_then(|value| {
            if value.is_null() {
                None
            } else {
                Some(value.clone())
            }
        }),
    })
}

pub(crate) fn permission_hook_response(
    hook_event_name: &str,
    decision: Decision,
    note: &str,
    updated_input: Option<Value>,
) -> Value {
    if hook_event_name == "PermissionRequest" {
        let decision = match decision {
            Decision::Approved => {
                if let Some(input) = updated_input {
                    json!({ "behavior": "allow", "updatedInput": input })
                } else {
                    json!({ "behavior": "allow" })
                }
            }
            Decision::Denied => {
                let message = if note.is_empty() {
                    "Denied from Atoll".to_string()
                } else {
                    format!("Denied from Atoll: {note}")
                };
                json!({
                    "behavior": "deny",
                    "message": message
                })
            }
        };

        return json!({
            "hookSpecificOutput": {
                "hookEventName": hook_event_name,
                "decision": decision
            }
        });
    }

    let (permission_decision, reason) = match decision {
        Decision::Approved => ("allow", "Approved from Atoll".to_string()),
        Decision::Denied => {
            let reason = if note.is_empty() {
                "Denied from Atoll".to_string()
            } else {
                format!("Denied from Atoll: {note}")
            };
            ("deny", reason)
        }
    };
    let mut output = json!({
        "hookEventName": hook_event_name,
        "permissionDecision": permission_decision,
        "permissionDecisionReason": reason
    });
    if matches!(decision, Decision::Approved) {
        if let Some(input) = updated_input {
            output
                .as_object_mut()
                .unwrap()
                .insert("updatedInput".to_string(), input);
        }
    }
    json!({ "hookSpecificOutput": output })
}

#[derive(Debug, Clone, Copy)]
enum PermissionResponseStyle {
    ClaudeCodex,
    #[allow(dead_code)]
    Cursor,
}

pub(crate) fn cursor_permission_hook_response(
    decision: Decision,
    note: &str,
    updated_input: Option<Value>,
) -> Value {
    match decision {
        Decision::Approved => {
            if let Some(input) = updated_input {
                json!({ "permission": "allow", "updated_input": input })
            } else {
                json!({ "permission": "allow" })
            }
        }
        Decision::Denied => {
            let message = if note.is_empty() {
                "Denied from Atoll".to_string()
            } else {
                format!("Denied from Atoll: {note}")
            };
            json!({
                "permission": "deny",
                "user_message": message,
                "agent_message": message
            })
        }
    }
}

fn cursor_hook_defer_response(hook_event_name: &str, reason: &str) -> Value {
    if matches!(
        hook_event_name,
        "postToolUse"
            | "postToolUseFailure"
            | "stop"
            | "subagentStart"
            | "subagentStop"
    ) {
        return json!({});
    }

    json!({
        "permission": "deny",
        "user_message": reason,
        "agent_message": reason
    })
}

fn build_permission_response(
    style: PermissionResponseStyle,
    hook_event_name: &str,
    decision: Decision,
    note: &str,
    updated_input: Option<Value>,
) -> Value {
    match style {
        PermissionResponseStyle::ClaudeCodex => {
            permission_hook_response(hook_event_name, decision, note, updated_input)
        }
        PermissionResponseStyle::Cursor => {
            cursor_permission_hook_response(decision, note, updated_input)
        }
    }
}

fn build_hook_defer_response(
    style: PermissionResponseStyle,
    hook_event_name: &str,
    reason: &str,
) -> Value {
    match style {
        PermissionResponseStyle::ClaudeCodex => hook_defer_response(hook_event_name, reason),
        PermissionResponseStyle::Cursor => cursor_hook_defer_response(hook_event_name, reason),
    }
}

pub(crate) fn hook_defer_response(hook_event_name: &str, reason: &str) -> Value {
    if matches!(
        hook_event_name,
        "PermissionRequest"
            | "PostToolUse"
            | "PostToolUseFailure"
            | "Stop"
            | "StopFailure"
            | "SubagentStart"
            | "SubagentStop"
    ) {
        return json!({});
    }

    json!({
        "hookSpecificOutput": {
            "hookEventName": hook_event_name,
            "permissionDecision": "ask",
            "permissionDecisionReason": reason
        }
    })
}

fn handle_connection(app: AppHandle, mut stream: TcpStream) {
    let result = read_http_request(&mut stream)
        .and_then(|request| route_request(app, request, &stream))
        .unwrap_or_else(|error| fallback_hook_response("PreToolUse", &error));

    let _ = write_json_response(&mut stream, result);
}

fn route_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    if let Some(response) = crate::capture::route_http(&app, &request.path) {
        return Ok(response);
    }

    if request.method != "POST" {
        return Err("Unsupported Atoll hook endpoint".into());
    }

    match request.path.as_str() {
        "/claude/pre-tool-use" => route_claude_request(app, request, stream),
        "/codex/hook" => route_codex_request(app, request, stream),
        "/cursor/hook" => route_cursor_request(app, request, stream),
        _ => Err("Unsupported Atoll hook endpoint".into()),
    }
}

fn route_claude_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    let payload: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("Invalid Claude hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("PreToolUse")
        .to_string();

    match hook_event_name.as_str() {
        "PreToolUse" | "PermissionRequest" => submit_blocking_permission_request(
            app,
            payload,
            stream,
            |id, payload, at| permission_request_from_claude_payload(id, payload, at),
            &hook_event_name,
            PermissionResponseStyle::ClaudeCodex,
        )
        .or_else(|error| {
            Ok(build_hook_defer_response(
                PermissionResponseStyle::ClaudeCodex,
                &hook_event_name,
                &error,
            ))
        }),
        "PostToolUse" | "PostToolUseFailure" => {
            sync_tool_completion(app, payload, AgentKind::Claude, Some(stream))?;
            Ok(json!({}))
        }
        "Stop" | "StopFailure" => {
            if payload
                .get("agent_id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty())
            {
                let state = app.state::<AppState>();
                complete_subagent(&state, &payload);
            }
            sync_turn_completion(app, payload, AgentKind::Claude, true, Some(stream))?;
            Ok(json!({}))
        }
        "SubagentStart" => {
            let state = app.state::<AppState>();
            register_subagent_start(&state, &payload, AgentKind::Claude);
            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("claude-code");
            let cwd = payload.get("cwd").and_then(Value::as_str).unwrap_or(".");
            register_known_session(&state, session_id, AgentKind::Claude, cwd, None);
            touch_session_activity(&state, session_id);
            emit_subagent_snapshot(&app, &state);
            Ok(json!({}))
        }
        "SubagentStop" => {
            let state = app.state::<AppState>();
            complete_subagent(&state, &payload);
            sync_turn_completion(app, payload, AgentKind::Claude, false, Some(stream))?;
            Ok(json!({}))
        }
        _ => Ok(json!({})),
    }
}

fn route_codex_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    let payload: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("Invalid Codex hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("PermissionRequest")
        .to_string();

    match hook_event_name.as_str() {
        "PermissionRequest" => submit_blocking_permission_request(
            app,
            payload,
            stream,
            |id, payload, at| permission_request_from_codex_payload(id, payload, at),
            &hook_event_name,
            PermissionResponseStyle::ClaudeCodex,
        )
        .or_else(|error| {
            Ok(build_hook_defer_response(
                PermissionResponseStyle::ClaudeCodex,
                &hook_event_name,
                &error,
            ))
        }),
        "PostToolUse" => {
            sync_tool_completion(app, payload, AgentKind::Codex, Some(stream))?;
            Ok(json!({}))
        }
        "Stop" => {
            if payload
                .get("agent_id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty())
            {
                let state = app.state::<AppState>();
                complete_subagent(&state, &payload);
            }
            sync_turn_completion(app, payload, AgentKind::Codex, true, Some(stream))?;
            Ok(json!({}))
        }
        "SubagentStart" => {
            let state = app.state::<AppState>();
            register_subagent_start(&state, &payload, AgentKind::Codex);
            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("codex");
            let cwd = payload.get("cwd").and_then(Value::as_str).unwrap_or(".");
            register_known_session(&state, session_id, AgentKind::Codex, cwd, None);
            touch_session_activity(&state, session_id);
            emit_subagent_snapshot(&app, &state);
            Ok(json!({}))
        }
        "SubagentStop" => {
            let state = app.state::<AppState>();
            complete_subagent(&state, &payload);
            sync_turn_completion(app, payload, AgentKind::Codex, false, Some(stream))?;
            Ok(json!({}))
        }
        _ => Ok(json!({})),
    }
}

/// Cursor fires `preToolUse` for *every* tool call (unlike Claude Code which
/// only fires `PermissionRequest` for dangerous ones).  Since Cursor already
/// has its own permission management UI (auto-approve / ask settings), Atoll
/// should not duplicate that gating.  All Cursor preToolUse events are
/// auto-approved so Atoll acts as an observer (session tracking, token usage)
/// rather than a secondary permission gate.

/// Check if the payload's `agent_id` matches a known active subagent and return
/// the parent session_id.  This prevents subagent tool-use events from being
/// registered as independent sessions.
fn resolve_cursor_subagent_parent(state: &AppState, payload: &Value) -> Option<String> {
    let agent_id = payload.get("agent_id").and_then(Value::as_str)?;
    if agent_id.is_empty() {
        return None;
    }
    let subagents = state.active_subagents.lock().ok()?;
    subagents
        .iter()
        .find(|s| s.agent_id == agent_id)
        .map(|s| s.session_id.clone())
}

fn route_cursor_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    let payload: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("Invalid Cursor hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("preToolUse")
        .to_string();

    match hook_event_name.as_str() {
        "preToolUse" => {
            let state = app.state::<AppState>();
            let parent_session = resolve_cursor_subagent_parent(&state, &payload);
            let session_id = parent_session
                .as_deref()
                .or_else(|| payload_session_id(&payload))
                .unwrap_or("cursor");
            let cwd = resolve_cursor_cwd(&payload);
            let transcript_path = payload_transcript_path(&payload);
            if parent_session.is_none() {
                register_known_session(
                    &state,
                    session_id,
                    AgentKind::Cursor,
                    &cwd,
                    transcript_path,
                );
            }
            touch_session_activity(&state, session_id);
            Ok(json!({ "permission": "allow" }))
        }
        "postToolUse" | "postToolUseFailure" => {
            let state = app.state::<AppState>();
            let parent_session = resolve_cursor_subagent_parent(&state, &payload);
            drop(state);
            let payload = if let Some(parent_id) = parent_session {
                let mut p = payload;
                p.as_object_mut()
                    .unwrap()
                    .insert("session_id".to_string(), Value::String(parent_id));
                p
            } else {
                payload
            };
            sync_tool_completion(app, payload, AgentKind::Cursor, Some(stream))?;
            Ok(json!({}))
        }
        "stop" => {
            let state = app.state::<AppState>();
            let parent_session = resolve_cursor_subagent_parent(&state, &payload);
            if payload
                .get("agent_id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty())
            {
                complete_subagent(&state, &payload);
            }
            drop(state);
            let payload = if let Some(parent_id) = parent_session {
                let mut p = payload;
                p.as_object_mut()
                    .unwrap()
                    .insert("session_id".to_string(), Value::String(parent_id));
                p
            } else {
                payload
            };
            sync_turn_completion(app, payload, AgentKind::Cursor, true, Some(stream))?;
            Ok(json!({}))
        }
        "subagentStart" => {
            let state = app.state::<AppState>();
            register_subagent_start(&state, &payload, AgentKind::Cursor);
            let session_id = payload_session_id(&payload).unwrap_or("cursor");
            let cwd = resolve_cursor_cwd(&payload);
            let transcript_path = payload_transcript_path(&payload);
            register_known_session(&state, session_id, AgentKind::Cursor, &cwd, transcript_path);
            touch_session_activity(&state, session_id);
            emit_subagent_snapshot(&app, &state);
            Ok(json!({}))
        }
        "subagentStop" => {
            let state = app.state::<AppState>();
            complete_subagent(&state, &payload);
            drop(state);
            sync_turn_completion(app, payload, AgentKind::Cursor, false, Some(stream))?;
            Ok(json!({}))
        }
        _ => Ok(json!({})),
    }
}

fn submit_blocking_permission_request(
    app: AppHandle,
    payload: Value,
    stream: &TcpStream,
    build_request: impl FnOnce(String, Value, String) -> Option<PermissionRequest>,
    hook_event_name: &str,
    response_style: PermissionResponseStyle,
) -> Result<Value, String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let request = build_request(request_id.clone(), payload, iso_timestamp_now())
        .ok_or_else(|| "Unsupported hook event".to_string())?;
    let state = app.state::<AppState>();
    let agent_label = agent_resolved_label(&request.agent);

    let is_auto_approved = state
        .auto_approve_sessions
        .lock()
        .map(|sessions| sessions.contains(&request.session))
        .unwrap_or(false);

    if is_auto_approved {
        {
            let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
            let mut auto_request = request;
            auto_request.status = PermissionStatus::Approved;
            auto_request.detail = format!("{} Auto-approved.", auto_request.detail);
            touch_session_activity(&state, &auto_request.session);
            requests.insert(0, auto_request);
            roll_over_token_usage_if_needed(&state);
        }
        let snapshot = build_snapshot(&app, &state);
        let _ = app.emit("snapshot-changed", &snapshot);
        return Ok(build_permission_response(
            response_style,
            hook_event_name,
            Decision::Approved,
            "",
            None,
        ));
    }

    let (sender, receiver) = mpsc::sync_channel(1);

    {
        let mut waiters = state
            .hook_waiters
            .lock()
            .map_err(|error| error.to_string())?;
        waiters.insert(request_id.clone(), sender);
    }

    let session_id = request.session.clone();
    let session_cwd = request.cwd.clone();
    let session_agent = request.agent.clone();
    let request_transcript_path = request.transcript_path.clone();

    {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        touch_session_activity(&state, &request.session);
        requests.insert(0, request);
        roll_over_token_usage_if_needed(&state);
    }
    if matches!(session_agent, AgentKind::Claude) {
        register_known_session(
            &state,
            &session_id,
            session_agent.clone(),
            &session_cwd,
            None,
        );
        let host =
            detect_host_for_claude_hook(&state, stream, &session_cwd, &request_transcript_path);
        if host != platform::SessionHost::Unknown {
            crate::store_session_host(&state, &session_id, host);
        }
    }
    if matches!(session_agent, AgentKind::Codex) {
        register_known_session(
            &state,
            &session_id,
            session_agent.clone(),
            &session_cwd,
            request_transcript_path.as_deref(),
        );
        let host =
            detect_host_for_codex_hook(&state, stream, &session_cwd, &request_transcript_path);
        if host != platform::SessionHost::Unknown {
            crate::store_session_host(&state, &session_id, host);
        }
    }
    if matches!(session_agent, AgentKind::Cursor) {
        register_known_session(
            &state,
            &session_id,
            session_agent.clone(),
            &session_cwd,
            request_transcript_path.as_deref(),
        );
        let host = detect_host_for_cursor_hook(stream);
        if host != platform::SessionHost::Unknown {
            crate::store_session_host(&state, &session_id, host);
        }
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    show_main_window_for_approval(&app);

    let deadline = Instant::now() + HOOK_RESPONSE_TIMEOUT;
    loop {
        match receiver.recv_timeout(HOOK_POLL_INTERVAL) {
            Ok(DecisionWithNote {
                decision,
                note,
                updated_input,
            }) => {
                return Ok(build_permission_response(
                    response_style,
                    hook_event_name,
                    decision,
                    &note,
                    updated_input,
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                remove_pending_waiter(&state, &request_id);
                return Ok(build_hook_defer_response(
                    response_style,
                    hook_event_name,
                    "Atoll internal error",
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if is_peer_disconnected(stream) {
                    remove_pending_waiter(&state, &request_id);
                    mark_request_completed_externally(&state, &app, &request_id, agent_label);
                    return Ok(json!({}));
                }
                if Instant::now() >= deadline {
                    remove_pending_waiter(&state, &request_id);
                    mark_request_denied(
                        &state,
                        &app,
                        &request_id,
                        "Timed out waiting for Atoll approval.",
                    );
                    return Ok(build_hook_defer_response(
                        response_style,
                        hook_event_name,
                        "Atoll approval timed out",
                    ));
                }
            }
        }
    }
}

fn agent_resolved_label(agent: &AgentKind) -> &'static str {
    match agent {
        AgentKind::Codex => "Codex",
        AgentKind::Claude => "Claude",
        AgentKind::Cursor => "Cursor",
        _ => "Agent",
    }
}

fn remove_pending_waiter(state: &AppState, request_id: &str) {
    if let Ok(mut waiters) = state.hook_waiters.lock() {
        waiters.remove(request_id);
    }
}

fn is_peer_disconnected(stream: &TcpStream) -> bool {
    let _ = stream.set_nonblocking(true);
    let mut buf = [0u8; 1];
    let disconnected = match stream.peek(&mut buf) {
        Ok(0) => true,
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(_) => true,
        Ok(_) => false,
    };
    let _ = stream.set_nonblocking(false);
    disconnected
}

fn mark_request_completed_externally(
    state: &AppState,
    app: &AppHandle,
    request_id: &str,
    agent_label: &str,
) {
    let resolved_suffix = format!("Resolved in {agent_label}.");
    let (resolved_session_id, resolved_transcript_path, resolved_agent) = {
        let Ok(mut requests) = state.requests.lock() else {
            return;
        };

        let mut resolved_session_id: Option<String> = None;
        let mut resolved_transcript_path: Option<String> = None;
        let mut resolved_agent: Option<AgentKind> = None;
        if let Some(request) = requests.iter_mut().find(|r| r.id == request_id) {
            if request.status == PermissionStatus::Pending {
                request.status = PermissionStatus::Approved;
                if !request.detail.contains(&resolved_suffix) {
                    request.detail = format!("{} {resolved_suffix}", request.detail);
                }
                touch_session_activity(state, &request.session);
                resolved_session_id = Some(request.session.clone());
                resolved_transcript_path = request.transcript_path.clone();
                resolved_agent = Some(request.agent.clone());
            }
        }
        roll_over_token_usage_if_needed(state);
        (
            resolved_session_id,
            resolved_transcript_path,
            resolved_agent,
        )
    };

    if let Some(session_id) = resolved_session_id.as_deref() {
        if let Err(error) = refresh_session_token_usage(
            state,
            session_id,
            resolved_transcript_path.as_deref(),
            resolved_agent.as_ref(),
        ) {
            eprintln!("Atoll token usage refresh failed: {error}");
        }
    }

    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
}

fn mark_request_denied(state: &AppState, app: &AppHandle, request_id: &str, note: &str) {
    {
        let Ok(mut requests) = state.requests.lock() else {
            return;
        };

        if let Some(request) = requests.iter_mut().find(|request| request.id == request_id) {
            request.status = PermissionStatus::Denied;
            request.detail = format!("{} {note}", request.detail);
            touch_session_activity(state, &request.session);
        }

        roll_over_token_usage_if_needed(state);
    }

    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
}

fn sync_tool_completion(
    app: AppHandle,
    payload: Value,
    agent: AgentKind,
    stream: Option<&TcpStream>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let agent_label = agent_resolved_label(&agent);
    let completed_suffix = format!("Completed in {agent_label}.");
    let mut completed_session_id = payload_session_id(&payload).map(str::to_string);
    let mut completed_transcript_path = payload_transcript_path(&payload).map(str::to_string);
    let transcript_path = payload_transcript_path(&payload).map(str::to_string);
    let cwd = match agent {
        AgentKind::Codex => resolve_codex_session_cwd(
            payload.get("cwd").and_then(Value::as_str).unwrap_or("."),
            transcript_path.as_deref(),
        ),
        AgentKind::Cursor => resolve_cursor_cwd(&payload),
        _ => payload
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_string(),
    };
    let codex_internal =
        matches!(agent, AgentKind::Codex) && is_codex_internal_session(&agent, &cwd, None);

    if let Some(session_id) = completed_session_id.as_deref() {
        if codex_internal {
            purge_tracked_session(&state, session_id, completed_transcript_path.as_deref());
        } else {
            register_known_session(
                &state,
                session_id,
                agent.clone(),
                &cwd,
                completed_transcript_path.as_deref(),
            );
            if matches!(agent, AgentKind::Claude) {
                let host = detect_host_for_claude_non_permission_hook(
                    stream,
                    &cwd,
                    completed_transcript_path.as_deref(),
                );
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Codex) {
                let host = detect_host_for_codex_non_permission_hook(
                    stream,
                    &cwd,
                    completed_transcript_path.as_deref(),
                );
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Cursor) {
                let host = detect_host_for_cursor_non_permission_hook(stream);
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
        }
    }

    let completed_request_id = {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        let completed_request_id =
            mark_matching_pending_request_complete(&mut requests, &payload, &completed_suffix);

        if let Some(request_id) = completed_request_id.as_deref() {
            if let Some(completed_request) =
                requests.iter().find(|request| request.id == request_id)
            {
                if completed_session_id.is_none() {
                    completed_session_id = Some(completed_request.session.clone());
                }
                if completed_transcript_path.is_none() {
                    completed_transcript_path = completed_request.transcript_path.clone();
                }
            }
        }

        if let Some(session_id) = completed_session_id.as_deref() {
            if completed_transcript_path.is_none() {
                completed_transcript_path = requests
                    .iter()
                    .filter(|request| request.session == session_id)
                    .find_map(|request| request.transcript_path.clone());
            }
        }
        completed_request_id
    };

    if let Some(session_id) = completed_session_id.as_deref() {
        if !codex_internal {
            if let Err(error) = refresh_session_token_usage(
                &state,
                session_id,
                completed_transcript_path.as_deref(),
                Some(&agent),
            ) {
                eprintln!("Atoll token usage refresh failed: {error}");
            }
        }
    }

    if let Some(request_id) = completed_request_id.as_deref() {
        if let Ok(mut waiters) = state.hook_waiters.lock() {
            if let Some(waiter) = waiters.remove(request_id) {
                let _ = waiter.send(DecisionWithNote {
                    decision: Decision::Approved,
                    note: String::new(),
                    updated_input: None,
                });
            }
        }
    }

    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);

    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn sync_turn_completion(
    app: AppHandle,
    payload: Value,
    agent: AgentKind,
    touch_activity: bool,
    stream: Option<&TcpStream>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let agent_label = agent_resolved_label(&agent);
    let completed_suffix = format!("Completed in {agent_label}.");
    let session_id = payload_session_id(&payload).map(str::to_string);
    let transcript_path = payload_transcript_path(&payload).map(str::to_string);
    let cwd = match agent {
        AgentKind::Codex => resolve_codex_session_cwd(
            payload.get("cwd").and_then(Value::as_str).unwrap_or("."),
            transcript_path.as_deref(),
        ),
        AgentKind::Cursor => resolve_cursor_cwd(&payload),
        _ => payload
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_string(),
    };
    let mut completed_request_id: Option<String> = None;
    let codex_internal =
        matches!(agent, AgentKind::Codex) && is_codex_internal_session(&agent, &cwd, None);

    if let Some(session_id) = session_id.as_deref() {
        {
            let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
            if let Some(index) = latest_pending_request_index(&requests, Some(session_id)) {
                let request = requests
                    .get_mut(index)
                    .expect("index from latest_pending_request_index should be valid");
                request.status = PermissionStatus::Approved;
                if !request.detail.contains(&completed_suffix) {
                    request.detail = format!("{} {completed_suffix}", request.detail);
                }
                completed_request_id = Some(request.id.clone());
            }
        }

        if codex_internal {
            purge_tracked_session(&state, session_id, transcript_path.as_deref());
        } else {
            if touch_activity {
                touch_session_activity(&state, session_id);
            }
            register_known_session(
                &state,
                session_id,
                agent.clone(),
                &cwd,
                transcript_path.as_deref(),
            );
            if matches!(agent, AgentKind::Claude) {
                let host = detect_host_for_claude_non_permission_hook(
                    stream,
                    &cwd,
                    transcript_path.as_deref(),
                );
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Codex) {
                let host = detect_host_for_codex_non_permission_hook(
                    stream,
                    &cwd,
                    transcript_path.as_deref(),
                );
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Cursor) {
                let host = detect_host_for_cursor_non_permission_hook(stream);
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Cursor) {
                if let Err(error) =
                    ingest_cursor_stop_token_usage(&state, session_id, &payload)
                {
                    eprintln!("Atoll Cursor token usage ingest failed: {error}");
                }
            } else if let Err(error) = refresh_session_token_usage(
                &state,
                session_id,
                transcript_path.as_deref(),
                Some(&agent),
            ) {
                eprintln!("Atoll token usage refresh failed: {error}");
            }
        }
    }

    if let Some(request_id) = completed_request_id.as_deref() {
        if let Ok(mut waiters) = state.hook_waiters.lock() {
            if let Some(waiter) = waiters.remove(request_id) {
                let _ = waiter.send(DecisionWithNote {
                    decision: Decision::Approved,
                    note: String::new(),
                    updated_input: None,
                });
            }
        }
    }

    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);

    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn payload_session_id(payload: &Value) -> Option<&str> {
    payload
        .get("session_id")
        .and_then(Value::as_str)
        .or_else(|| payload.get("sessionId").and_then(Value::as_str))
        .or_else(|| payload.get("conversation_id").and_then(Value::as_str))
        .or_else(|| payload.get("conversationId").and_then(Value::as_str))
}

fn payload_transcript_path(payload: &Value) -> Option<&str> {
    payload
        .get("transcript_path")
        .and_then(Value::as_str)
        .or_else(|| payload.get("transcriptPath").and_then(Value::as_str))
}

/// Resolve Cursor session cwd: prefer `workspace_roots[0]` over raw `cwd`
/// (which is often `"."` or the hook runner's working directory).
fn resolve_cursor_cwd(payload: &Value) -> String {
    if let Some(roots) = payload.get("workspace_roots").and_then(Value::as_array) {
        if let Some(first) = roots.first().and_then(Value::as_str) {
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }
    if let Some(roots) = payload.get("workspaceRoots").and_then(Value::as_array) {
        if let Some(first) = roots.first().and_then(Value::as_str) {
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }
    payload
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or(".")
        .to_string()
}

/// Determine SessionHost for a Claude session.
///
/// Priority: peer process tree → transcript path → Claude Desktop running check → frontmost/cwd detection.
fn detect_host_for_claude_hook(
    state: &AppState,
    stream: &TcpStream,
    cwd: &str,
    transcript_path: &Option<String>,
) -> platform::SessionHost {
    eprintln!("[Atoll:host-detect] === Claude session host detection ===");
    eprintln!("[Atoll:host-detect] cwd={cwd:?}");
    eprintln!("[Atoll:host-detect] transcript_path={transcript_path:?}");

    let peer_host = hook_peer_session_host(stream);
    eprintln!("[Atoll:host-detect] peer_process_tree → {peer_host:?}");
    if peer_host != platform::SessionHost::Unknown {
        eprintln!("[Atoll:host-detect] RESULT: {peer_host:?} (from peer process tree)");
        return peer_host;
    }

    if let Some(path) = transcript_path.as_deref() {
        if is_desktop_transcript_path(path) {
            eprintln!(
                "[Atoll:host-detect] RESULT: ClaudeDesktop (transcript path matched Desktop)"
            );
            return platform::SessionHost::ClaudeDesktop;
        }
        if is_cli_transcript_path(path) {
            if !is_claude_desktop_app_running() {
                eprintln!("[Atoll:host-detect] RESULT: ClaudeCli (CLI path + Desktop NOT running)");
                return platform::SessionHost::ClaudeCli;
            }
            eprintln!("[Atoll:host-detect] CLI-style path but Desktop IS running, need further signals...");
        }
    } else {
        eprintln!("[Atoll:host-detect] transcript_path is None");
    }

    let desktop_running = is_claude_desktop_app_running();
    eprintln!("[Atoll:host-detect] claude_desktop_running={desktop_running}");
    if desktop_running {
        let prev_pid = state.previous_app_pid.lock().ok().and_then(|g| *g);
        eprintln!("[Atoll:host-detect] previous_app_pid={prev_pid:?}");
        let hint = prev_pid.map(|p| p as u32);
        if let Some(pid) = hint {
            let from_pid = platform::detect_session_host_from_peer_pid(pid);
            eprintln!("[Atoll:host-detect] detect_from_previous_pid({pid}) → {from_pid:?}");
            if from_pid == platform::SessionHost::ClaudeDesktop {
                eprintln!(
                    "[Atoll:host-detect] RESULT: ClaudeDesktop (previous_app_pid in Desktop tree)"
                );
                return platform::SessionHost::ClaudeDesktop;
            }
        }
        let terminal_front = is_any_terminal_frontmost();
        eprintln!("[Atoll:host-detect] terminal_frontmost={terminal_front}");
        if !terminal_front {
            eprintln!("[Atoll:host-detect] RESULT: ClaudeDesktop (Desktop running + no terminal frontmost)");
            return platform::SessionHost::ClaudeDesktop;
        }
    }

    let prev_pid = state.previous_app_pid.lock().ok().and_then(|g| *g);
    let fallback = platform::detect_claude_session_host_at_hook(cwd, prev_pid);
    eprintln!("[Atoll:host-detect] RESULT: {fallback:?} (final fallback, prev_pid={prev_pid:?})");
    fallback
}

/// Determine SessionHost for a Codex session.
///
/// Priority: peer process tree → transcript path → Codex Desktop running check → frontmost/cwd detection.
fn detect_host_for_codex_hook(
    state: &AppState,
    stream: &TcpStream,
    cwd: &str,
    transcript_path: &Option<String>,
) -> platform::SessionHost {
    eprintln!("[Atoll:host-detect] === Codex session host detection ===");
    eprintln!("[Atoll:host-detect] cwd={cwd:?}");
    eprintln!("[Atoll:host-detect] transcript_path={transcript_path:?}");

    let peer_host = hook_peer_codex_session_host(stream);
    eprintln!("[Atoll:host-detect] peer_process_tree → {peer_host:?}");
    if peer_host != platform::SessionHost::Unknown {
        eprintln!("[Atoll:host-detect] RESULT: {peer_host:?} (from peer process tree)");
        return peer_host;
    }

    if let Some(path) = transcript_path.as_deref() {
        if is_codex_desktop_transcript_path(path) {
            eprintln!("[Atoll:host-detect] RESULT: CodexDesktop (transcript path matched Desktop)");
            return platform::SessionHost::CodexDesktop;
        }
        if is_codex_cli_transcript_path(path) {
            if !is_codex_desktop_app_running() {
                eprintln!("[Atoll:host-detect] RESULT: CodexCli (CLI path + Desktop NOT running)");
                return platform::SessionHost::CodexCli;
            }
            eprintln!("[Atoll:host-detect] CLI-style path but Desktop IS running, need further signals...");
        }
    } else {
        eprintln!("[Atoll:host-detect] transcript_path is None");
    }

    let desktop_running = is_codex_desktop_app_running();
    eprintln!("[Atoll:host-detect] codex_desktop_running={desktop_running}");
    if desktop_running {
        let prev_pid = state.previous_app_pid.lock().ok().and_then(|g| *g);
        eprintln!("[Atoll:host-detect] previous_app_pid={prev_pid:?}");
        if let Some(pid) = prev_pid.map(|p| p as u32) {
            let from_pid = platform::detect_codex_session_host_from_peer_pid(pid);
            eprintln!("[Atoll:host-detect] detect_from_previous_pid({pid}) → {from_pid:?}");
            if from_pid == platform::SessionHost::CodexDesktop {
                eprintln!(
                    "[Atoll:host-detect] RESULT: CodexDesktop (previous_app_pid in Desktop tree)"
                );
                return platform::SessionHost::CodexDesktop;
            }
        }
        let terminal_front = is_any_terminal_frontmost();
        eprintln!("[Atoll:host-detect] terminal_frontmost={terminal_front}");
        if !terminal_front {
            eprintln!("[Atoll:host-detect] RESULT: CodexDesktop (Desktop running + no terminal frontmost)");
            return platform::SessionHost::CodexDesktop;
        }
    }

    let prev_pid = state.previous_app_pid.lock().ok().and_then(|g| *g);
    let fallback = platform::detect_codex_session_host_at_hook(cwd, prev_pid);
    eprintln!("[Atoll:host-detect] RESULT: {fallback:?} (final fallback, prev_pid={prev_pid:?})");
    fallback
}

/// Identify the Claude session host by tracing the hook HTTP peer's process tree.
fn hook_peer_session_host(stream: &TcpStream) -> platform::SessionHost {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("[Atoll:host-detect] peer_addr() failed: {e}");
            return platform::SessionHost::Unknown;
        }
    };
    let port = peer.port();
    let own_pid = std::process::id();
    eprintln!("[Atoll:host-detect] peer port={port}, own_pid={own_pid}");

    let output = match std::process::Command::new("lsof")
        .args(["-i", &format!("TCP@127.0.0.1:{port}"), "-n", "-P", "-t"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[Atoll:host-detect] lsof exec failed: {e}");
            return platform::SessionHost::Unknown;
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    eprintln!("[Atoll:host-detect] lsof stdout={text:?}, stderr={stderr_text:?}");
    for line in text.lines() {
        if let Ok(pid) = line.trim().parse::<u32>() {
            if pid != own_pid {
                let result = platform::detect_session_host_from_peer_pid(pid);
                eprintln!("[Atoll:host-detect] peer_pid={pid} → {result:?}");
                return result;
            }
        }
    }
    platform::SessionHost::Unknown
}

fn is_cli_transcript_path(path: &str) -> bool {
    path.contains("/.claude/")
        || (path.contains("/claude/projects/") && !path.contains("/Application Support/"))
}

fn is_desktop_transcript_path(path: &str) -> bool {
    if path.contains("/Application Support/") && !path.contains("/.claude/") {
        return true;
    }
    path.contains("Claude-3p")
        || path.contains("local-agent-mode-sessions")
        || path.contains("com.anthropic.claude")
        || path.contains("agent-sessions")
}

fn is_claude_desktop_app_running() -> bool {
    platform::is_claude_desktop_app_running()
}

fn is_any_terminal_frontmost() -> bool {
    platform::frontmost_is_terminal()
}

/// Identify the Codex session host by tracing the hook HTTP peer's process tree.
fn hook_peer_codex_session_host(stream: &TcpStream) -> platform::SessionHost {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("[Atoll:host-detect] peer_addr() failed: {e}");
            return platform::SessionHost::Unknown;
        }
    };
    let port = peer.port();
    let own_pid = std::process::id();
    eprintln!("[Atoll:host-detect] peer port={port}, own_pid={own_pid}");

    let output = match std::process::Command::new("lsof")
        .args(["-i", &format!("TCP@127.0.0.1:{port}"), "-n", "-P", "-t"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[Atoll:host-detect] lsof exec failed: {e}");
            return platform::SessionHost::Unknown;
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    eprintln!("[Atoll:host-detect] lsof stdout={text:?}, stderr={stderr_text:?}");
    for line in text.lines() {
        if let Ok(pid) = line.trim().parse::<u32>() {
            if pid != own_pid {
                let result = platform::detect_codex_session_host_from_peer_pid(pid);
                eprintln!("[Atoll:host-detect] peer_pid={pid} → {result:?}");
                return result;
            }
        }
    }
    platform::SessionHost::Unknown
}

fn detect_host_for_cursor_hook(stream: &TcpStream) -> platform::SessionHost {
    let peer_host = hook_peer_cursor_session_host(stream);
    if peer_host != platform::SessionHost::Unknown {
        return peer_host;
    }
    if platform::is_cursor_app_running() {
        return platform::SessionHost::CursorIde;
    }
    platform::SessionHost::Unknown
}

fn detect_host_for_cursor_non_permission_hook(stream: Option<&TcpStream>) -> platform::SessionHost {
    if let Some(stream) = stream {
        let peer_host = hook_peer_cursor_session_host(stream);
        if peer_host != platform::SessionHost::Unknown {
            return peer_host;
        }
    }
    if platform::is_cursor_app_running() {
        return platform::SessionHost::CursorIde;
    }
    platform::SessionHost::Unknown
}

/// Identify the Cursor session host by tracing the hook HTTP peer's process tree.
fn hook_peer_cursor_session_host(stream: &TcpStream) -> platform::SessionHost {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => return platform::SessionHost::Unknown,
    };
    let port = peer.port();
    let own_pid = std::process::id();

    let output = match std::process::Command::new("lsof")
        .args(["-i", &format!("TCP@127.0.0.1:{port}"), "-n", "-P", "-t"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return platform::SessionHost::Unknown,
    };

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Ok(pid) = line.trim().parse::<u32>() {
            if pid != own_pid {
                let result = platform::detect_cursor_session_host_from_peer_pid(pid);
                if result != platform::SessionHost::Unknown {
                    return result;
                }
            }
        }
    }
    platform::SessionHost::Unknown
}

fn is_codex_cli_transcript_path(path: &str) -> bool {
    path.contains("/.codex/sessions/") || path.contains("/.codex/")
}

fn is_codex_desktop_transcript_path(path: &str) -> bool {
    path.contains("com.openai.codex")
        || (path.contains("/Application Support/") && path.contains("codex"))
}

fn is_codex_desktop_app_running() -> bool {
    platform::is_codex_desktop_app_running()
}

/// Detect Claude session host for non-permission hooks (Stop, PostToolUse, SubagentStop).
fn detect_host_for_claude_non_permission_hook(
    stream: Option<&TcpStream>,
    cwd: &str,
    transcript_path: Option<&str>,
) -> platform::SessionHost {
    eprintln!("[Atoll:host-detect] === Claude non-permission hook detection ===");
    eprintln!("[Atoll:host-detect] cwd={cwd:?}, transcript_path={transcript_path:?}");

    if let Some(stream) = stream {
        let peer_host = hook_peer_session_host(stream);
        eprintln!("[Atoll:host-detect] peer_process_tree → {peer_host:?}");
        if peer_host != platform::SessionHost::Unknown {
            eprintln!("[Atoll:host-detect] RESULT: {peer_host:?} (from peer process tree)");
            return peer_host;
        }
    }

    if let Some(path) = transcript_path {
        if is_desktop_transcript_path(path) {
            eprintln!(
                "[Atoll:host-detect] RESULT: ClaudeDesktop (transcript path matched Desktop)"
            );
            return platform::SessionHost::ClaudeDesktop;
        }
        if is_cli_transcript_path(path) {
            if !is_claude_desktop_app_running() {
                eprintln!("[Atoll:host-detect] RESULT: ClaudeCli (CLI path + Desktop NOT running)");
                return platform::SessionHost::ClaudeCli;
            }
            eprintln!(
                "[Atoll:host-detect] CLI-style path but Desktop IS running, checking further..."
            );
        }
    }

    let desktop_running = is_claude_desktop_app_running();
    eprintln!("[Atoll:host-detect] claude_desktop_running={desktop_running}");
    if desktop_running && !is_any_terminal_frontmost() {
        eprintln!(
            "[Atoll:host-detect] RESULT: ClaudeDesktop (Desktop running + no terminal frontmost)"
        );
        return platform::SessionHost::ClaudeDesktop;
    }

    let fallback = platform::detect_claude_session_host(cwd);
    eprintln!("[Atoll:host-detect] RESULT: {fallback:?} (CWD fallback)");
    fallback
}

/// Detect Codex session host for non-permission hooks (Stop, PostToolUse, SubagentStop).
fn detect_host_for_codex_non_permission_hook(
    stream: Option<&TcpStream>,
    cwd: &str,
    transcript_path: Option<&str>,
) -> platform::SessionHost {
    eprintln!("[Atoll:host-detect] === Codex non-permission hook detection ===");
    eprintln!("[Atoll:host-detect] cwd={cwd:?}, transcript_path={transcript_path:?}");

    if let Some(stream) = stream {
        let peer_host = hook_peer_codex_session_host(stream);
        eprintln!("[Atoll:host-detect] peer_process_tree → {peer_host:?}");
        if peer_host != platform::SessionHost::Unknown {
            eprintln!("[Atoll:host-detect] RESULT: {peer_host:?} (from peer process tree)");
            return peer_host;
        }
    }

    if let Some(path) = transcript_path {
        if is_codex_desktop_transcript_path(path) {
            eprintln!("[Atoll:host-detect] RESULT: CodexDesktop (transcript path matched Desktop)");
            return platform::SessionHost::CodexDesktop;
        }
        if is_codex_cli_transcript_path(path) {
            if !is_codex_desktop_app_running() {
                eprintln!("[Atoll:host-detect] RESULT: CodexCli (CLI path + Desktop NOT running)");
                return platform::SessionHost::CodexCli;
            }
            eprintln!(
                "[Atoll:host-detect] CLI-style path but Desktop IS running, checking further..."
            );
        }
    }

    let desktop_running = is_codex_desktop_app_running();
    eprintln!("[Atoll:host-detect] codex_desktop_running={desktop_running}");
    if desktop_running && !is_any_terminal_frontmost() {
        eprintln!(
            "[Atoll:host-detect] RESULT: CodexDesktop (Desktop running + no terminal frontmost)"
        );
        return platform::SessionHost::CodexDesktop;
    }

    let fallback = platform::detect_codex_session_host(cwd);
    eprintln!("[Atoll:host-detect] RESULT: {fallback:?} (CWD fallback)");
    fallback
}

pub(crate) fn mark_matching_pending_request_complete(
    requests: &mut [PermissionRequest],
    payload: &Value,
    completed_suffix: &str,
) -> Option<String> {
    let payload_tool_use_id = payload.get("tool_use_id").and_then(Value::as_str);
    let payload_session = payload.get("session_id").and_then(Value::as_str);
    let payload_tool_name = payload.get("tool_name").and_then(Value::as_str);
    let payload_tool_input = payload.get("tool_input").cloned().unwrap_or(Value::Null);
    let payload_command =
        payload_tool_name.map(|tool_name| command_label(tool_name, &payload_tool_input));

    let matched_index = requests.iter().position(|request| {
        if request.status != PermissionStatus::Pending {
            return false;
        }

        if let (Some(request_tool_use_id), Some(payload_tool_use_id)) =
            (request.tool_use_id.as_deref(), payload_tool_use_id)
        {
            return request_tool_use_id == payload_tool_use_id;
        }

        let session_matches = payload_session
            .map(|session| request.session == session)
            .unwrap_or(false);
        let command_matches = payload_command
            .as_ref()
            .map(|command| request.command == *command)
            .unwrap_or(false);

        session_matches && command_matches
    });

    let fallback_index = matched_index
        .or_else(|| unique_pending_request_index(requests, payload_session))
        .or_else(|| latest_pending_request_index(requests, payload_session));
    let request = requests.get_mut(fallback_index?)?;

    request.status = PermissionStatus::Approved;
    if !request.detail.contains(completed_suffix) {
        request.detail = format!("{} {completed_suffix}", request.detail);
    }
    Some(request.id.clone())
}

fn unique_pending_request_index(
    requests: &[PermissionRequest],
    payload_session: Option<&str>,
) -> Option<usize> {
    let mut candidates = requests
        .iter()
        .enumerate()
        .filter(|(_, request)| request.status == PermissionStatus::Pending)
        .filter(|(_, request)| {
            payload_session
                .map(|session| request.session == session)
                .unwrap_or(true)
        });

    let (index, _) = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }

    Some(index)
}

fn latest_pending_request_index(
    requests: &[PermissionRequest],
    payload_session: Option<&str>,
) -> Option<usize> {
    let session = payload_session?;
    requests
        .iter()
        .enumerate()
        .find(|(_, request)| {
            request.status == PermissionStatus::Pending && request.session == session
        })
        .map(|(index, _)| index)
}

fn fallback_hook_response(hook_event_name: &str, reason: &str) -> Value {
    hook_defer_response(hook_event_name, reason)
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|error| format!("Failed to read hook request line: {error}"))?;

    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "Missing hook request method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "Missing hook request path".to_string())?
        .to_string();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(|error| format!("Failed to read hook request header: {error}"))?;
        let trimmed = header.trim_end();
        if trimmed.is_empty() {
            break;
        }

        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| format!("Invalid content-length: {error}"))?;
            }
        }
    }

    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|error| format!("Failed to read hook request body: {error}"))?;

    Ok(HttpRequest { method, path, body })
}

fn write_json_response(stream: &mut TcpStream, body: Value) -> std::io::Result<()> {
    let body = serde_json::to_string(&body).unwrap_or_else(|_| "{}".into());
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

fn command_label(tool_name: &str, tool_input: &Value) -> String {
    if tool_name == "Bash" || tool_name == "Shell" || tool_name == "exec_command" {
        if let Some(command) = tool_input.get("command").and_then(Value::as_str) {
            return format!("Bash: {command}");
        }
    }

    if tool_name == "apply_patch" {
        if let Some(command) = tool_input.get("command").and_then(Value::as_str) {
            return format!("Edit: {command}");
        }
    }

    if let Some(file_path) = tool_input.get("file_path").and_then(Value::as_str) {
        return format!("{tool_name}: {file_path}");
    }

    tool_name.to_string()
}

fn detail_label(tool_name: &str, tool_input: &Value) -> String {
    if let Some(description) = tool_input.get("description").and_then(Value::as_str) {
        return description.to_string();
    }

    if let Some(command) = tool_input.get("command").and_then(Value::as_str) {
        return command.to_string();
    }

    if let Some(file_path) = tool_input.get("file_path").and_then(Value::as_str) {
        return format!("{tool_name} wants to access {file_path}.");
    }

    format!("{tool_name} is requesting approval.")
}

#[cfg(test)]
mod bridge_bind_tests {
    use super::*;

    #[test]
    fn fallback_port_range_starts_after_default() {
        assert_eq!(HOOK_FALLBACK_PORT_START, DEFAULT_HOOK_PORT + 1);
        assert!(HOOK_FALLBACK_PORT_END >= HOOK_FALLBACK_PORT_START);
    }

    #[test]
    fn write_bridge_config_json_shape() {
        let temp = std::env::temp_dir().join(format!("atoll-bridge-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).expect("temp dir");
        let config_path = temp.join("bridge.json");

        let port = 47_778_u16;
        let config = json!({
            "port": port,
            "claudeUrl": format!("http://{HOOK_BIND_HOST}:{port}/claude/pre-tool-use"),
            "codexUrl": format!("http://{HOOK_BIND_HOST}:{port}/codex/hook"),
            "cursorUrl": format!("http://{HOOK_BIND_HOST}:{port}/cursor/hook"),
        });
        std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");

        let parsed: Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed.get("port").and_then(Value::as_u64),
            Some(port as u64)
        );
        assert!(parsed
            .get("claudeUrl")
            .and_then(Value::as_str)
            .unwrap()
            .contains("/claude/pre-tool-use"));
        assert!(parsed
            .get("codexUrl")
            .and_then(Value::as_str)
            .unwrap()
            .contains("/codex/hook"));
        assert!(parsed
            .get("cursorUrl")
            .and_then(Value::as_str)
            .unwrap()
            .contains("/cursor/hook"));

        let _ = std::fs::remove_dir_all(temp);
    }
}

#[cfg(test)]
mod payload_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exec_command_uses_bash_label() {
        let input = json!({"command": "printf hello"});
        assert_eq!(command_label("exec_command", &input), "Bash: printf hello");
    }

    #[test]
    fn bash_command_label_unchanged() {
        let input = json!({"command": "ls -la"});
        assert_eq!(command_label("Bash", &input), "Bash: ls -la");
    }

    #[test]
    fn shell_command_label_matches_bash() {
        let input = json!({"command": "npm test"});
        assert_eq!(command_label("Shell", &input), "Bash: npm test");
    }

    #[test]
    fn cursor_permission_response_allow() {
        let response = cursor_permission_hook_response(Decision::Approved, "", None);
        assert_eq!(response.get("permission").and_then(Value::as_str), Some("allow"));
    }

    #[test]
    fn cursor_permission_response_deny() {
        let response = cursor_permission_hook_response(Decision::Denied, "blocked", None);
        assert_eq!(response.get("permission").and_then(Value::as_str), Some("deny"));
    }

    #[test]
    fn cursor_payload_builds_permission_request() {
        let payload = json!({
            "hook_event_name": "preToolUse",
            "conversation_id": "conv-123",
            "cwd": "/tmp/project",
            "tool_name": "Shell",
            "tool_input": { "command": "echo hi" },
            "tool_use_id": "tool-1"
        });
        let request = permission_request_from_cursor_payload(
            "req-1".into(),
            payload,
            "2026-01-01T00:00:00Z".into(),
        )
        .expect("cursor request");
        assert_eq!(request.session, "conv-123");
        assert!(matches!(request.agent, AgentKind::Cursor));
    }
}
