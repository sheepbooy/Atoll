use std::collections::HashMap;
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

mod hook_bridge;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionRequest {
    id: String,
    agent: AgentKind,
    session: String,
    command: String,
    detail: String,
    cwd: String,
    requested_at: String,
    status: PermissionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IslandSnapshot {
    online: bool,
    pending_count: usize,
    active_request: Option<PermissionRequest>,
    recent: Vec<PermissionRequest>,
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

struct AppState {
    requests: Mutex<Vec<PermissionRequest>>,
    hook_waiters: Mutex<HashMap<String, SyncSender<Decision>>>,
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> IslandSnapshot {
    snapshot_from(&state.requests.lock().expect("state mutex poisoned"))
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

    let waiter = state
        .hook_waiters
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&id);
    if let Some(waiter) = waiter {
        let _ = waiter.send(decision);
    }

    let snapshot = snapshot_from(&requests);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn simulate_permission_request(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    requests.insert(
        0,
        PermissionRequest {
            id: Uuid::new_v4().to_string(),
            agent: AgentKind::Claude,
            session: "local-demo".into(),
            command: "Bash: git status --short".into(),
            detail: "A demo approval request is waiting for confirmation.".into(),
            cwd: std::env::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| ".".into()),
            requested_at: iso_timestamp_now(),
            status: PermissionStatus::Pending,
        },
    );

    let snapshot = snapshot_from(&requests);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    show_main_window(&app);
    Ok(snapshot)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            requests: Mutex::new(seed_requests()),
            hook_waiters: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            resolve_permission_request,
            simulate_permission_request
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            hook_bridge::start_server(app.handle().clone());

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                let _ = window.set_shadow(false);
                let _ = window.set_skip_taskbar(true);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Atoll");
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Atoll", true, None::<&str>)?;
    let demo = MenuItem::with_id(app, "demo", "Create Demo Request", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &demo, &quit])?;

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "demo" => {
                let state = app.state::<AppState>();
                let _ = simulate_permission_request(app.clone(), state);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(&tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn snapshot_from(requests: &[PermissionRequest]) -> IslandSnapshot {
    let pending_count = requests
        .iter()
        .filter(|request| request.status == PermissionStatus::Pending)
        .count();
    let active_request = requests
        .iter()
        .find(|request| request.status == PermissionStatus::Pending)
        .cloned();

    IslandSnapshot {
        online: true,
        pending_count,
        active_request,
        recent: requests.iter().take(12).cloned().collect(),
    }
}

fn seed_requests() -> Vec<PermissionRequest> {
    Vec::new()
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
    }

    #[test]
    fn encodes_hook_decision_for_claude_hook_event() {
        let approved =
            crate::hook_bridge::claude_hook_response("PermissionRequest", crate::Decision::Approved);
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
            crate::hook_bridge::claude_hook_response("PermissionRequest", crate::Decision::Denied);
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
    fn encodes_hook_decision_for_claude_pre_tool_use() {
        let approved =
            crate::hook_bridge::claude_hook_response("PreToolUse", crate::Decision::Approved);
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
}
