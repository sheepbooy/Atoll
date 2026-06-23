//! Platform-specific window, focus, and shell integration.

use tauri::{App, AppHandle, LogicalPosition, Monitor, PhysicalSize, WebviewWindow};

use crate::{AppState, HomeWindowBounds, NotchMetrics};

/// Where a Claude session is running — used to restore focus correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionHost {
    #[default]
    Unknown,
    ClaudeDesktop,
    ClaudeCli,
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// AppKit screen frame metadata (macOS) or unused defaults elsewhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenGeometry {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub origin_y: f64,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub height: f64,
}

pub fn setup_app(app: &mut App) -> bool {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    #[cfg(target_os = "windows")]
    {
        if !windows::ensure_single_instance() {
            return false;
        }
    }
    true
}

pub fn apply_island_window_style(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    macos::apply_island_window_style(window);
    #[cfg(target_os = "windows")]
    windows::apply_island_window_style(window);
}

pub fn detect_notch_metrics(
    window: &WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
) -> NotchMetrics {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_notch_metrics(window, monitor_x, monitor_width);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (window, monitor_x, monitor_width);
        return NotchMetrics::default();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (window, monitor_x, monitor_width);
        NotchMetrics::default()
    }
}

pub fn screen_geometry_for_monitor(
    window: &WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
) -> Option<ScreenGeometry> {
    #[cfg(target_os = "macos")]
    {
        return macos::screen_geometry_for_monitor(window, monitor_x, monitor_width);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, monitor_x, monitor_width);
        None
    }
}

/// Top Y coordinate (logical) where the island should align.
pub fn monitor_top_y(_window: &WebviewWindow, monitor: &Monitor) -> f64 {
    #[cfg(target_os = "windows")]
    {
        return windows::monitor_work_area_top(_window, monitor);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let scale_factor = monitor.scale_factor();
        monitor.position().to_logical::<f64>(scale_factor).y
    }
}

pub fn set_island_cursor_events_ignored(window: &WebviewWindow, ignore: bool) {
    #[cfg(target_os = "macos")]
    {
        macos::set_island_cursor_events_ignored(window, ignore);
    }
    #[cfg(target_os = "windows")]
    {
        windows::set_island_cursor_events_ignored(window, ignore);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = window.set_ignore_cursor_events(ignore);
    }
}

#[cfg(target_os = "windows")]
pub fn sync_cursor_pass_through(window: &WebviewWindow, hovering: bool) {
    windows::sync_cursor_pass_through(window, hovering);
}

#[cfg(target_os = "windows")]
pub fn is_island_expanded() -> bool {
    windows::is_island_expanded()
}

#[cfg(target_os = "windows")]
pub fn compact_hover_expand_dwell() -> std::time::Duration {
    std::time::Duration::from_millis(windows::COMPACT_HOVER_EXPAND_DWELL_MS)
}

pub fn set_island_window_frame_now(
    window: &WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    home: HomeWindowBounds,
) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        return macos::set_island_window_frame_now(window, position, size, scale_factor, home);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::set_island_window_frame_now(window, position, size, scale_factor, home);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        window.set_size(size)?;
        window.set_position(position)
    }
}

pub fn set_island_window_frame(
    window: &WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    home: Option<HomeWindowBounds>,
) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        return macos::set_island_window_frame(window, position, size, scale_factor, home);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::set_island_window_frame(window, position, size, scale_factor, home);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        window.set_size(size)?;
        window.set_position(position)
    }
}

pub fn detect_claude_session_host(cwd: &str) -> SessionHost {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_claude_session_host(cwd);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_claude_session_host(cwd);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = cwd;
        SessionHost::Unknown
    }
}

/// Prefer frontmost-app snapshot while the hook fires, before Atoll takes focus.
pub fn detect_claude_session_host_at_hook(cwd: &str) -> SessionHost {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_claude_session_host_at_hook(cwd);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_claude_session_host(cwd);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = cwd;
        SessionHost::Unknown
    }
}

fn clear_previous_app_pid(state: &AppState) {
    let _ = state
        .previous_app_pid
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
}

fn restore_terminal_focus(state: &AppState, cwd: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        if macos::activate_previous_app_if_terminal(state) {
            return true;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if windows::activate_previous_app_if_terminal(state) {
            return true;
        }
    }
    open_in_terminal(cwd).is_ok()
}

pub fn restore_focus_after_approval(
    app: &AppHandle,
    state: &AppState,
    agent: Option<&str>,
    session_id: Option<&str>,
    cwd: Option<&str>,
) {
    if agent == Some("claude") {
        if let (Some(session_id), Some(cwd)) = (session_id, cwd) {
            let host = crate::claude_session_host(state, session_id, cwd);
            match host {
                SessionHost::ClaudeDesktop => {
                    let _ = focus_claude_app(app);
                    clear_previous_app_pid(state);
                    return;
                }
                SessionHost::ClaudeCli => {
                    if restore_terminal_focus(state, cwd) {
                        return;
                    }
                }
                SessionHost::Unknown => {}
            }
        }
    }

    #[cfg(target_os = "macos")]
    macos::restore_previous_app_focus(state);
    #[cfg(target_os = "windows")]
    windows::restore_foreground_window(state);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = state;
    }
}

pub fn open_agent_app(
    app: &AppHandle,
    state: &AppState,
    agent: &str,
    cwd: &str,
    session_id: Option<&str>,
) -> Result<(), String> {
    if agent == "claude" {
        let host = session_id
            .map(|id| crate::claude_session_host(state, id, cwd))
            .unwrap_or_else(|| detect_claude_session_host(cwd));
        return match host {
            SessionHost::ClaudeDesktop => focus_claude_app(app),
            SessionHost::ClaudeCli => open_in_terminal(cwd),
            SessionHost::Unknown => {
                let detected = detect_claude_session_host(cwd);
                match detected {
                    SessionHost::ClaudeDesktop => focus_claude_app(app),
                    SessionHost::ClaudeCli => open_in_terminal(cwd),
                    SessionHost::Unknown => focus_claude_app(app),
                }
            }
        };
    }

    open_in_terminal(cwd)
}

pub fn focus_claude_app(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::focus_claude_app(app);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        return windows::focus_claude_app();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err("focus_claude_app is not supported on this platform".to_string())
    }
}

pub fn finish_show_for_approval(window: &WebviewWindow, app: &AppHandle, request_focus: bool) {
    #[cfg(target_os = "macos")]
    macos::finish_show_for_approval(window, app, request_focus);
    #[cfg(target_os = "windows")]
    windows::finish_show_for_approval(window, app, request_focus);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = window.show();
        if request_focus {
            let _ = window.set_focus();
        }
        let _ = app;
    }
}

pub fn tray_icon(app: &AppHandle) -> Option<tauri::image::Image<'static>> {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        windows::tray_icon()
    }
    #[cfg(not(target_os = "windows"))]
    {
        app.default_window_icon().map(|icon| {
            tauri::image::Image::new_owned(icon.rgba().to_vec(), icon.width(), icon.height())
        })
    }
}

pub fn open_url(app: &AppHandle, url: &str) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("Failed to open URL: {error}"))
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::open_in_terminal(cwd);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::open_in_terminal(cwd);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = cwd;
        Err("open_in_terminal is not supported on this platform".to_string())
    }
}
