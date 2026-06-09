use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    iso_timestamp_now, show_main_window, snapshot_from, AgentKind, AppState, Decision,
    PermissionRequest, PermissionStatus,
};

const HOOK_BIND_ADDR: &str = "127.0.0.1:47777";
const HOOK_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

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
    })
}

pub(crate) fn claude_hook_response(hook_event_name: &str, decision: Decision) -> Value {
    if hook_event_name == "PermissionRequest" {
        let decision = match decision {
            Decision::Approved => json!({ "behavior": "allow" }),
            Decision::Denied => json!({
                "behavior": "deny",
                "message": "Denied from Atoll"
            }),
        };

        return json!({
            "hookSpecificOutput": {
                "hookEventName": hook_event_name,
                "decision": decision
            }
        });
    }

    let (permission_decision, reason) = match decision {
        Decision::Approved => ("allow", "Approved from Atoll"),
        Decision::Denied => ("deny", "Denied from Atoll"),
    };
    json!({
        "hookSpecificOutput": {
            "hookEventName": hook_event_name,
            "permissionDecision": permission_decision,
            "permissionDecisionReason": reason
        }
    })
}

fn claude_hook_ask_response(hook_event_name: &str, reason: &str) -> Value {
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
        .and_then(|request| route_request(app, request))
        .unwrap_or_else(|error| claude_hook_ask_response("PreToolUse", &error));

    let _ = write_json_response(&mut stream, result);
}

fn route_request(app: AppHandle, request: HttpRequest) -> Result<Value, String> {
    if request.method != "POST" || request.path != "/claude/pre-tool-use" {
        return Err("Unsupported Atoll hook endpoint".into());
    }

    let payload: Value = serde_json::from_slice(&request.body)
        .map_err(|error| format!("Invalid Claude hook payload: {error}"))?;

    submit_claude_pre_tool_request(app, payload)
}

fn submit_claude_pre_tool_request(app: AppHandle, payload: Value) -> Result<Value, String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("PreToolUse")
        .to_string();
    let request =
        permission_request_from_claude_payload(request_id.clone(), payload, iso_timestamp_now())
            .ok_or_else(|| "Unsupported Claude hook event".to_string())?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let state = app.state::<AppState>();

    {
        let mut waiters = state
            .hook_waiters
            .lock()
            .map_err(|error| error.to_string())?;
        waiters.insert(request_id.clone(), sender);
    }

    {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        requests.insert(0, request);
        let snapshot = snapshot_from(&requests);
        app.emit("snapshot-changed", &snapshot)
            .map_err(|error| error.to_string())?;
    }

    show_main_window(&app);

    match receiver.recv_timeout(HOOK_RESPONSE_TIMEOUT) {
        Ok(decision) => Ok(claude_hook_response(&hook_event_name, decision)),
        Err(_) => {
            remove_pending_waiter(&state, &request_id);
            mark_request_denied(
                &state,
                &app,
                &request_id,
                "Timed out waiting for Atoll approval.",
            );
            Ok(claude_hook_ask_response(
                &hook_event_name,
                "Atoll approval timed out",
            ))
        }
    }
}

fn remove_pending_waiter(state: &AppState, request_id: &str) {
    if let Ok(mut waiters) = state.hook_waiters.lock() {
        waiters.remove(request_id);
    }
}

fn mark_request_denied(state: &AppState, app: &AppHandle, request_id: &str, note: &str) {
    let Ok(mut requests) = state.requests.lock() else {
        return;
    };

    if let Some(request) = requests.iter_mut().find(|request| request.id == request_id) {
        request.status = PermissionStatus::Denied;
        request.detail = format!("{} {note}", request.detail);
    }

    let snapshot = snapshot_from(&requests);
    let _ = app.emit("snapshot-changed", &snapshot);
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
