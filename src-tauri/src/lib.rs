use std::collections::HashMap;
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::utils::config::Color;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, State};
use uuid::Uuid;

mod hook_bridge;

const COMPACT_WINDOW_WIDTH: f64 = 132.0;
const COMPACT_WINDOW_HEIGHT: f64 = 28.0;
const EXPANDED_WINDOW_WIDTH: f64 = 620.0;
const EXPANDED_WINDOW_HEIGHT: f64 = 360.0;

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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum IslandWindowMode {
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IslandHoverChanged {
    hovering: bool,
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
            tool_use_id: None,
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

#[tauri::command]
fn set_island_presentation(app: AppHandle, mode: IslandWindowMode) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    apply_island_window_mode(&window, mode).map_err(|error| error.to_string())
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
            simulate_permission_request,
            set_island_presentation
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            hook_bridge::start_server(app.handle().clone());
            start_island_hover_monitor(app.handle().clone());

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                let _ = window.set_shadow(false);
                let _ = window.set_skip_taskbar(true);
                let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
                apply_macos_island_window_style(&window);
                let _ = apply_island_window_mode(&window, IslandWindowMode::Compact);
                let _ = window.show();
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

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = apply_island_window_mode(&window, IslandWindowMode::Expanded);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("island-open-requested", ());
    }
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
) -> tauri::Result<()> {
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };

    apply_macos_island_window_style(window);
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    window.set_size(island_window_logical_size(mode))?;
    let _ = window.set_ignore_cursor_events(matches!(mode, IslandWindowMode::Compact));

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let window_size = island_window_physical_size(mode, monitor.scale_factor());
    let centered_x =
        monitor_position.x + (monitor_size.width as i32 - window_size.width as i32).max(0) / 2;
    let centered_y = island_top_y(window, monitor_position.y, monitor.work_area().position.y);

    window.set_position(PhysicalPosition::new(centered_x, centered_y))?;
    Ok(())
}

fn island_window_logical_size(mode: IslandWindowMode) -> LogicalSize<f64> {
    match mode {
        IslandWindowMode::Compact => LogicalSize::new(COMPACT_WINDOW_WIDTH, COMPACT_WINDOW_HEIGHT),
        IslandWindowMode::Expanded => {
            LogicalSize::new(EXPANDED_WINDOW_WIDTH, EXPANDED_WINDOW_HEIGHT)
        }
    }
}

fn island_window_physical_size(mode: IslandWindowMode, scale_factor: f64) -> PhysicalSize<u32> {
    let logical_size = island_window_logical_size(mode);

    PhysicalSize::new(
        (logical_size.width * scale_factor).round() as u32,
        (logical_size.height * scale_factor).round() as u32,
    )
}

fn island_top_y(window: &tauri::WebviewWindow, monitor_y: i32, work_area_y: i32) -> i32 {
    let visible_offset = (work_area_y - monitor_y).max(0);
    let menu_offset = macos_menu_bar_offset_physical(window).unwrap_or(0);

    monitor_y - menu_offset.saturating_sub(visible_offset)
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
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_macos_island_window_style(_window: &tauri::WebviewWindow) {}

#[cfg(target_os = "macos")]
fn macos_menu_bar_offset_physical(window: &tauri::WebviewWindow) -> Option<i32> {
    use objc2_app_kit::NSWindow;

    let ns_window = window.ns_window().ok()?;
    if ns_window.is_null() {
        return None;
    }

    unsafe {
        let ns_window = &*(ns_window.cast::<NSWindow>());
        let screen = ns_window.screen()?;
        let frame = screen.frame();
        let visible_frame = screen.visibleFrame();
        let scale = screen.backingScaleFactor();
        let offset = (frame.origin.y + frame.size.height)
            - (visible_frame.origin.y + visible_frame.size.height);

        Some((offset.max(0.0) * scale).round() as i32)
    }
}

#[cfg(not(target_os = "macos"))]
fn macos_menu_bar_offset_physical(_window: &tauri::WebviewWindow) -> Option<i32> {
    None
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
    fn leaves_ambiguous_pending_requests_when_post_tool_use_has_no_match_fields() {
        let mut requests = vec![
            crate::PermissionRequest {
                id: "request-1".into(),
                tool_use_id: None,
                agent: crate::AgentKind::Claude,
                session: "session-123".into(),
                command: "Bash: echo one".into(),
                detail: "one".into(),
                cwd: "/tmp/project".into(),
                requested_at: "2026-06-09T09:00:00Z".into(),
                status: crate::PermissionStatus::Pending,
            },
            crate::PermissionRequest {
                id: "request-2".into(),
                tool_use_id: None,
                agent: crate::AgentKind::Claude,
                session: "session-123".into(),
                command: "Bash: echo two".into(),
                detail: "two".into(),
                cwd: "/tmp/project".into(),
                requested_at: "2026-06-09T09:00:01Z".into(),
                status: crate::PermissionStatus::Pending,
            },
        ];

        let payload = json!({
            "session_id": "session-123",
            "hook_event_name": "PostToolUse"
        });

        let completed_id =
            crate::hook_bridge::mark_matching_pending_request_complete(&mut requests, &payload);

        assert_eq!(completed_id, None);
        assert_eq!(requests[0].status, crate::PermissionStatus::Pending);
        assert_eq!(requests[1].status, crate::PermissionStatus::Pending);
    }

    #[test]
    fn encodes_hook_decision_for_claude_hook_event() {
        let approved = crate::hook_bridge::claude_hook_response(
            "PermissionRequest",
            crate::Decision::Approved,
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
