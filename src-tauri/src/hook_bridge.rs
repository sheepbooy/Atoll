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
    build_snapshot, iso_timestamp_now, is_codex_internal_session, platform, purge_tracked_session,
    refresh_session_token_usage, register_known_session, roll_over_token_usage_if_needed,
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
    });
    std::fs::write(path, serde_json::to_string_pretty(&config)?)
}

pub(crate) fn refresh_bridge_config_file(app: &AppHandle) -> std::io::Result<()> {
    let port = app
        .state::<AppState>()
        .bridge_port
        .load(Ordering::SeqCst);
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
        format!(
            "no available hook bridge port in {DEFAULT_HOOK_PORT}..{HOOK_FALLBACK_PORT_END}"
        ),
    ))
}

fn bridge_socket_addr(port: u16) -> Option<SocketAddr> {
    format!("{HOOK_BIND_HOST}:{port}").parse().ok()
}

fn bridge_port_from_config_file() -> Option<u16> {
    let path = bridge_config_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;
    value.get("port").and_then(Value::as_u64).and_then(|port| u16::try_from(port).ok())
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
    let stored_port = app
        .state::<AppState>()
        .bridge_port
        .load(Ordering::SeqCst);
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

    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or(".");
    if is_codex_internal_session(&AgentKind::Codex, cwd) {
        return None;
    }

    permission_request_from_tool_payload(id, payload, requested_at, AgentKind::Codex, false)
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
        session: payload
            .get("session_id")
            .and_then(Value::as_str)
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
    })
}

pub(crate) fn permission_hook_response(hook_event_name: &str, decision: Decision, note: &str) -> Value {
    if hook_event_name == "PermissionRequest" {
        let decision = match decision {
            Decision::Approved => json!({ "behavior": "allow" }),
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
    json!({
        "hookSpecificOutput": {
            "hookEventName": hook_event_name,
            "permissionDecision": permission_decision,
            "permissionDecisionReason": reason
        }
    })
}

pub(crate) fn hook_defer_response(hook_event_name: &str, reason: &str) -> Value {
    if matches!(
        hook_event_name,
        "PermissionRequest"
            | "PostToolUse"
            | "PostToolUseFailure"
            | "Stop"
            | "StopFailure"
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

fn route_request(app: AppHandle, request: HttpRequest, stream: &TcpStream) -> Result<Value, String> {
    if let Some(response) = crate::capture::route_http(&app, &request.path) {
        return Ok(response);
    }

    if request.method != "POST" {
        return Err("Unsupported Atoll hook endpoint".into());
    }

    match request.path.as_str() {
        "/claude/pre-tool-use" => route_claude_request(app, request, stream),
        "/codex/hook" => route_codex_request(app, request, stream),
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
        )
        .or_else(|error| Ok(hook_defer_response(&hook_event_name, &error))),
        "PostToolUse" | "PostToolUseFailure" => {
            sync_tool_completion(app, payload, AgentKind::Claude)?;
            Ok(json!({}))
        }
        "Stop" | "StopFailure" => {
            sync_turn_completion(app, payload, AgentKind::Claude, true)?;
            Ok(json!({}))
        }
        "SubagentStop" => {
            sync_turn_completion(app, payload, AgentKind::Claude, false)?;
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
        )
        .or_else(|error| Ok(hook_defer_response(&hook_event_name, &error))),
        "PostToolUse" => {
            sync_tool_completion(app, payload, AgentKind::Codex)?;
            Ok(json!({}))
        }
        "Stop" => {
            sync_turn_completion(app, payload, AgentKind::Codex, true)?;
            Ok(json!({}))
        }
        "SubagentStop" => {
            sync_turn_completion(app, payload, AgentKind::Codex, false)?;
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
        return Ok(permission_hook_response(hook_event_name, Decision::Approved, ""));
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

    {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        touch_session_activity(&state, &request.session);
        requests.insert(0, request);
        roll_over_token_usage_if_needed(&state);
    }
    if matches!(session_agent, AgentKind::Claude) {
        let host = platform::detect_claude_session_host_at_hook(&session_cwd);
        if host != platform::SessionHost::Unknown {
            crate::store_claude_session_host(&state, &session_id, host);
        } else {
            let _ = crate::claude_session_host(&state, &session_id, &session_cwd);
        }
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    show_main_window_for_approval(&app);

    let deadline = Instant::now() + HOOK_RESPONSE_TIMEOUT;
    loop {
        match receiver.recv_timeout(HOOK_POLL_INTERVAL) {
            Ok(DecisionWithNote { decision, note }) => {
                return Ok(permission_hook_response(hook_event_name, decision, &note))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                remove_pending_waiter(&state, &request_id);
                return Ok(hook_defer_response(hook_event_name, "Atoll internal error"));
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
                    return Ok(hook_defer_response(
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
        (resolved_session_id, resolved_transcript_path, resolved_agent)
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
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let agent_label = agent_resolved_label(&agent);
    let completed_suffix = format!("Completed in {agent_label}.");
    let mut completed_session_id = payload_session_id(&payload).map(str::to_string);
    let mut completed_transcript_path = payload_transcript_path(&payload).map(str::to_string);
    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or(".");
    let codex_internal =
        matches!(agent, AgentKind::Codex) && is_codex_internal_session(&agent, cwd);

    if let Some(session_id) = completed_session_id.as_deref() {
        if codex_internal {
            purge_tracked_session(
                &state,
                session_id,
                completed_transcript_path.as_deref(),
            );
        } else {
            register_known_session(
                &state,
                session_id,
                agent.clone(),
                cwd,
                completed_transcript_path.as_deref(),
            );
        }
    }

    let completed_request_id = {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        let completed_request_id =
            mark_matching_pending_request_complete(&mut requests, &payload, &completed_suffix);

        if let Some(request_id) = completed_request_id.as_deref() {
            if let Some(completed_request) = requests.iter().find(|request| request.id == request_id) {
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
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let agent_label = agent_resolved_label(&agent);
    let completed_suffix = format!("Completed in {agent_label}.");
    let session_id = payload_session_id(&payload).map(str::to_string);
    let transcript_path = payload_transcript_path(&payload).map(str::to_string);
    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or(".");
    let mut completed_request_id: Option<String> = None;
    let codex_internal =
        matches!(agent, AgentKind::Codex) && is_codex_internal_session(&agent, cwd);

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
                cwd,
                transcript_path.as_deref(),
            );
            if let Err(error) = refresh_session_token_usage(
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
}

fn payload_transcript_path(payload: &Value) -> Option<&str> {
    payload
        .get("transcript_path")
        .and_then(Value::as_str)
        .or_else(|| payload.get("transcriptPath").and_then(Value::as_str))
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
        .find(|(_, request)| request.status == PermissionStatus::Pending && request.session == session)
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
    if tool_name == "Bash" {
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
        let temp = std::env::temp_dir().join(format!(
            "atoll-bridge-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp).expect("temp dir");
        let config_path = temp.join("bridge.json");

        let port = 47_778_u16;
        let config = json!({
            "port": port,
            "claudeUrl": format!("http://{HOOK_BIND_HOST}:{port}/claude/pre-tool-use"),
            "codexUrl": format!("http://{HOOK_BIND_HOST}:{port}/codex/hook"),
        });
        std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");

        let parsed: Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(parsed.get("port").and_then(Value::as_u64), Some(port as u64));
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

        let _ = std::fs::remove_dir_all(temp);
    }
}
