use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::ImageReader;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    build_snapshot, iso_timestamp_now, touch_session_activity, AgentKind, AppState,
    PermissionRequest, PermissionStatus, TokenUsage,
};

static FORCE_HOOK_UNINSTALLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static SCREENSHOT_SLOT: OnceLock<(Mutex<Option<Vec<u8>>>, Condvar)> = OnceLock::new();

fn screenshot_slot() -> &'static (Mutex<Option<Vec<u8>>>, Condvar) {
    SCREENSHOT_SLOT.get_or_init(|| (Mutex::new(None), Condvar::new()))
}

pub fn enabled() -> bool {
    std::env::var("ATOLL_CAPTURE").ok().as_deref() == Some("1")
}

pub fn seed_approval_demo(app: &AppHandle, state: &AppState) {
    FORCE_HOOK_UNINSTALLED.store(false, std::sync::atomic::Ordering::SeqCst);
    let now = iso_timestamp_now();

    let pending = PermissionRequest {
        id: "capture-pending".into(),
        tool_use_id: Some("tool-capture-1".into()),
        agent: AgentKind::Claude,
        session: "session-atoll".into(),
        command: "Bash: npm test -- --run".into(),
        detail: "Run the project test suite in the current workspace.".into(),
        cwd: "~/code/my-app".into(),
        requested_at: now.clone(),
        status: PermissionStatus::Pending,
        archived: false,
        supports_always: true,
        transcript_path: None,
        tool_input: None,
    };

    let history = [
        (
            "capture-codex-1",
            "session-api",
            AgentKind::Codex,
            "Bash: cargo check",
            "~/code/api-server",
        ),
        (
            "capture-gemini-1",
            "session-docs",
            AgentKind::Gemini,
            "Read: README.md",
            "~/code/docs-site",
        ),
    ];

    {
        let mut requests = state.requests.lock().expect("state mutex poisoned");
        requests.clear();
        requests.push(pending.clone());
        for (id, session, agent, command, cwd) in history {
            requests.push(PermissionRequest {
                id: id.into(),
                tool_use_id: None,
                agent,
                session: session.into(),
                command: command.into(),
                detail: String::new(),
                cwd: cwd.into(),
                requested_at: now.clone(),
                status: PermissionStatus::Approved,
                archived: false,
                supports_always: false,
                transcript_path: None,
                tool_input: None,
            });
        }
    }

    {
        let mut pinned = state
            .pinned_sessions
            .lock()
            .expect("state mutex poisoned");
        pinned.clear();
        pinned.insert("session-atoll".into());
    }

    {
        let mut tokens = state
            .session_token_usage
            .lock()
            .expect("state mutex poisoned");
        tokens.clear();
        tokens.insert(
            "session-atoll".into(),
            TokenUsage {
                input_tokens: 128_400,
                output_tokens: 42_180,
                cache_read_tokens: 890_000,
                cache_creation_tokens: 12_400,
            },
        );
    }

    for session in ["session-atoll", "session-api", "session-docs"] {
        touch_session_activity(state, session);
    }

    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
}

pub fn seed_idle_demo(app: &AppHandle, state: &AppState) {
    FORCE_HOOK_UNINSTALLED.store(true, std::sync::atomic::Ordering::SeqCst);
    {
        let mut requests = state.requests.lock().expect("state mutex poisoned");
        requests.clear();
    }
    {
        let mut pinned = state
            .pinned_sessions
            .lock()
            .expect("state mutex poisoned");
        pinned.clear();
    }
    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
}

pub fn force_hook_uninstalled() -> bool {
    enabled() && FORCE_HOOK_UNINSTALLED.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn listening_online() -> bool {
    enabled() && !FORCE_HOOK_UNINSTALLED.load(std::sync::atomic::Ordering::SeqCst)
}

fn capture_main_window_png(app: &AppHandle) -> Result<(Vec<u8>, u32, u32), String> {
    let (mutex, cv) = screenshot_slot();
    {
        let mut guard = mutex.lock().map_err(|error| error.to_string())?;
        *guard = None;
    }

    app.emit("capture-screenshot-requested", ())
        .map_err(|error| error.to_string())?;

    let guard = mutex.lock().map_err(|error| error.to_string())?;
    let (guard, timeout) = cv
        .wait_timeout(guard, Duration::from_secs(8))
        .map_err(|error| error.to_string())?;

    let png = guard
        .as_ref()
        .cloned()
        .ok_or_else(|| {
            if timeout.timed_out() {
                "screenshot timed out (frontend did not respond)".to_string()
            } else {
                "screenshot unavailable".to_string()
            }
        })?;

    let img = ImageReader::new(std::io::Cursor::new(&png))
        .with_guessed_format()
        .map_err(|error| error.to_string())?
        .decode()
        .map_err(|error| error.to_string())?;

    Ok((png, img.width(), img.height()))
}

fn screenshot_json(app: &AppHandle) -> Value {
    match capture_main_window_png(app) {
        Ok((png, width, height)) => json!({
            "png_base64": STANDARD.encode(png),
            "width": width,
            "height": height,
        }),
        Err(error) => json!({ "error": error }),
    }
}

#[tauri::command]
pub fn capture_provide_screenshot(png_base64: String) -> Result<(), String> {
    if !enabled() {
        return Err("capture mode disabled".into());
    }

    let png = STANDARD
        .decode(png_base64.trim())
        .map_err(|error| error.to_string())?;
    if png.is_empty() {
        return Err("empty screenshot payload".into());
    }

    let (mutex, cv) = screenshot_slot();
    {
        let mut guard = mutex.lock().map_err(|error| error.to_string())?;
        *guard = Some(png);
    }
    cv.notify_one();
    Ok(())
}

pub fn route_http(app: &AppHandle, path: &str) -> Option<Value> {
    if !enabled() {
        return None;
    }

    let state = app.state::<AppState>();
    match path {
        "/capture/screenshot" => Some(screenshot_json(app)),
        "/capture/expand" => {
            let _ = app.emit("island-open-requested", ());
            Some(json!({ "ok": true }))
        }
        "/capture/collapse" => {
            let _ = app.emit("capture-collapse", ());
            Some(json!({ "ok": true }))
        }
        "/capture/approval" => {
            seed_approval_demo(app, &state);
            Some(json!({ "ok": true }))
        }
        "/capture/idle" => {
            seed_idle_demo(app, &state);
            Some(json!({ "ok": true }))
        }
        "/capture/hooks" => {
            seed_idle_demo(app, &state);
            let _ = app.emit("capture-open-hooks", ());
            Some(json!({ "ok": true }))
        }
        _ => None,
    }
}
