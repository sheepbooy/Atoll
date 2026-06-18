use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    build_snapshot, iso_timestamp_now, refresh_session_token_usage, register_known_session,
    roll_over_token_usage_if_needed, show_main_window_for_approval, touch_session_last_seen,
    AgentKind, AppState, Decision, DecisionWithNote, PermissionRequest, PermissionStatus,
};

const HOOK_BIND_ADDR: &str = "127.0.0.1:47777";
const HOOK_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const BRIDGE_PROBE_TIMEOUT: Duration = Duration::from_millis(200);

/// True when the local hook bridge accepts TCP connections on its bind address.
pub(crate) fn is_bridge_reachable() -> bool {
    HOOK_BIND_ADDR
        .parse::<SocketAddr>()
        .ok()
        .and_then(|addr| TcpStream::connect_timeout(&addr, BRIDGE_PROBE_TIMEOUT).ok())
        .is_some()
}

pub(crate) fn start_server(app: AppHandle) {
    thread::spawn(move || {
        let listener = match TcpListener::bind(HOOK_BIND_ADDR) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("Atoll hook bridge failed to bind {HOOK_BIND_ADDR}: {error}");
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

    let tool_name = payload.get("tool_name")?.as_str()?.to_string();
    let tool_input = payload.get("tool_input").cloned().unwrap_or(Value::Null);
    let command = command_label(&tool_name, &tool_input);
    let detail = detail_label(&tool_name, &tool_input);

    Some(PermissionRequest {
        id,
        tool_use_id: payload
            .get("tool_use_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        agent: AgentKind::Claude,
        session: payload
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("claude-code")
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
        supports_always: payload
            .get("permission_suggestions")
            .and_then(Value::as_array)
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        transcript_path: payload
            .get("transcript_path")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

pub(crate) fn claude_hook_response(hook_event_name: &str, decision: Decision, note: &str) -> Value {
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

pub(crate) fn claude_hook_ask_response(hook_event_name: &str, reason: &str) -> Value {
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

    if request.method != "POST" || request.path != "/claude/pre-tool-use" {
        return Err("Unsupported Atoll hook endpoint".into());
    }

    let payload: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("Invalid Claude hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("PreToolUse")
        .to_string();

    match hook_event_name.as_str() {
        "PreToolUse" | "PermissionRequest" => {
            submit_claude_pre_tool_request(app, payload, stream)
                .or_else(|error| Ok(fallback_hook_response(&hook_event_name, &error)))
        }
        "PostToolUse" | "PostToolUseFailure" => {
            sync_claude_tool_completion(app, payload)?;
            Ok(json!({}))
        }
        "Stop" | "StopFailure" | "SubagentStop" => {
            sync_claude_turn_completion(app, payload)?;
            Ok(json!({}))
        }
        _ => Ok(json!({})),
    }
}

const HOOK_POLL_INTERVAL: Duration = Duration::from_millis(180);

fn submit_claude_pre_tool_request(
    app: AppHandle,
    payload: Value,
    stream: &TcpStream,
) -> Result<Value, String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("PreToolUse")
        .to_string();
    let request =
        permission_request_from_claude_payload(request_id.clone(), payload, iso_timestamp_now())
            .ok_or_else(|| "Unsupported Claude hook event".to_string())?;
    let state = app.state::<AppState>();

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
            touch_session_last_seen(&state, &auto_request.session);
            requests.insert(0, auto_request);
            roll_over_token_usage_if_needed(&state);
        }
        let snapshot = build_snapshot(&app, &state);
        let _ = app.emit("snapshot-changed", &snapshot);
        return Ok(claude_hook_response(&hook_event_name, Decision::Approved, ""));
    }

    let (sender, receiver) = mpsc::sync_channel(1);

    {
        let mut waiters = state
            .hook_waiters
            .lock()
            .map_err(|error| error.to_string())?;
        waiters.insert(request_id.clone(), sender);
    }

    {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        touch_session_last_seen(&state, &request.session);
        requests.insert(0, request);
        roll_over_token_usage_if_needed(&state);
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    show_main_window_for_approval(&app);

    let deadline = Instant::now() + HOOK_RESPONSE_TIMEOUT;
    loop {
        match receiver.recv_timeout(HOOK_POLL_INTERVAL) {
            Ok(DecisionWithNote { decision, note }) => {
                return Ok(claude_hook_response(&hook_event_name, decision, &note))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                remove_pending_waiter(&state, &request_id);
                return Ok(claude_hook_ask_response(
                    &hook_event_name,
                    "Atoll internal error",
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if is_peer_disconnected(stream) {
                    remove_pending_waiter(&state, &request_id);
                    mark_request_completed_externally(&state, &app, &request_id);
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
                    return Ok(claude_hook_ask_response(
                        &hook_event_name,
                        "Atoll approval timed out",
                    ));
                }
            }
        }
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
) {
    let (resolved_session_id, resolved_transcript_path) = {
        let Ok(mut requests) = state.requests.lock() else {
            return;
        };

        let mut resolved_session_id: Option<String> = None;
        let mut resolved_transcript_path: Option<String> = None;
        if let Some(request) = requests.iter_mut().find(|r| r.id == request_id) {
            if request.status == PermissionStatus::Pending {
                request.status = PermissionStatus::Approved;
                if !request.detail.contains("Resolved in Claude.") {
                    request.detail = format!("{} Resolved in Claude.", request.detail);
                }
                touch_session_last_seen(state, &request.session);
                resolved_session_id = Some(request.session.clone());
                resolved_transcript_path = request.transcript_path.clone();
            }
        }
        roll_over_token_usage_if_needed(state);
        (resolved_session_id, resolved_transcript_path)
    };

    if let Some(session_id) = resolved_session_id.as_deref() {
        if let Err(error) =
            refresh_session_token_usage(state, session_id, resolved_transcript_path.as_deref())
        {
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
            touch_session_last_seen(state, &request.session);
        }

        roll_over_token_usage_if_needed(state);
    }

    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
}

fn sync_claude_tool_completion(app: AppHandle, payload: Value) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut completed_session_id = payload_session_id(&payload).map(str::to_string);
    let mut completed_transcript_path = payload_transcript_path(&payload).map(str::to_string);

    if let Some(session_id) = completed_session_id.as_deref() {
        let cwd = payload
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(".");
        register_known_session(
            &state,
            session_id,
            AgentKind::Claude,
            cwd,
            completed_transcript_path.as_deref(),
        );
    }

    let completed_request_id = {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        let completed_request_id = mark_matching_pending_request_complete(&mut requests, &payload);

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
        touch_session_last_seen(&state, session_id);
        if let Err(error) = refresh_session_token_usage(
            &state,
            session_id,
            completed_transcript_path.as_deref(),
        ) {
            eprintln!("Atoll token usage refresh failed: {error}");
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

fn sync_claude_turn_completion(app: AppHandle, payload: Value) -> Result<(), String> {
    let state = app.state::<AppState>();
    let session_id = payload_session_id(&payload).map(str::to_string);
    let transcript_path = payload_transcript_path(&payload).map(str::to_string);
    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or(".");
    let mut completed_request_id: Option<String> = None;

    if let Some(session_id) = session_id.as_deref() {
        {
            let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
            if let Some(index) = latest_pending_request_index(&requests, Some(session_id)) {
                let request = requests
                    .get_mut(index)
                    .expect("index from latest_pending_request_index should be valid");
                request.status = PermissionStatus::Approved;
                if !request.detail.contains("Completed in Claude.") {
                    request.detail = format!("{} Completed in Claude.", request.detail);
                }
                completed_request_id = Some(request.id.clone());
            }
        }

        touch_session_last_seen(&state, session_id);
        register_known_session(
            &state,
            session_id,
            AgentKind::Claude,
            cwd,
            transcript_path.as_deref(),
        );
        if let Err(error) =
            refresh_session_token_usage(&state, session_id, transcript_path.as_deref())
        {
            eprintln!("Atoll token usage refresh failed: {error}");
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
    if !request.detail.contains("Completed in Claude.") {
        request.detail = format!("{} Completed in Claude.", request.detail);
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
    claude_hook_ask_response(hook_event_name, reason)
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
