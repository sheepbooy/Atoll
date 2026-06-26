//! Platform-specific window, focus, and shell integration.

use tauri::{App, AppHandle, LogicalPosition, Monitor, PhysicalSize, WebviewWindow};

use crate::{AppState, HomeWindowBounds, NotchMetrics};

/// Where an agent session is running — used to restore focus correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionHost {
    #[default]
    Unknown,
    ClaudeDesktop,
    ClaudeCli,
    CodexDesktop,
    CodexCli,
    CursorIde,
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub mod autostart;

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
        autostart::migrate_legacy_if_needed();
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

/// Re-assert the island's topmost state before expanding. macOS keeps its
/// NSMainMenuWindowLevel persistently so this is a no-op there; Windows topmost
/// can drift during a large unfocused resize, so it re-applies WS_EX_TOPMOST.
pub fn ensure_island_on_top(window: &WebviewWindow) {
    #[cfg(target_os = "windows")]
    windows::ensure_island_on_top(window);
    #[cfg(not(target_os = "windows"))]
    let _ = window;
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
/// Falls back to `previous_app_pid` when Atoll is already frontmost (rapid-fire approvals).
pub fn detect_claude_session_host_at_hook(cwd: &str, previous_app_pid: Option<i64>) -> SessionHost {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_claude_session_host_at_hook(cwd, previous_app_pid);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = previous_app_pid;
        return windows::detect_claude_session_host(cwd);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (cwd, previous_app_pid);
        SessionHost::Unknown
    }
}

/// Determine Claude session host by checking the hook peer process's ancestry.
pub fn detect_session_host_from_peer_pid(pid: u32) -> SessionHost {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_session_host_from_peer_pid(pid);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_claude_session_host_from_peer_pid(pid);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = pid;
        SessionHost::Unknown
    }
}

/// Determine Codex session host by checking the hook peer process's ancestry.
pub fn detect_codex_session_host_from_peer_pid(pid: u32) -> SessionHost {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_codex_session_host_from_peer_pid(pid);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_codex_session_host_from_peer_pid(pid);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = pid;
        SessionHost::Unknown
    }
}

pub fn detect_codex_session_host(cwd: &str) -> SessionHost {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_codex_session_host(cwd);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_codex_session_host(cwd);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = cwd;
        SessionHost::Unknown
    }
}

pub fn detect_codex_session_host_at_hook(cwd: &str, previous_app_pid: Option<i64>) -> SessionHost {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_codex_session_host_at_hook(cwd, previous_app_pid);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = previous_app_pid;
        return windows::detect_codex_session_host(cwd);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (cwd, previous_app_pid);
        SessionHost::Unknown
    }
}

/// Check if Claude Desktop app is currently running on the system.
pub fn is_claude_desktop_app_running() -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos::is_claude_desktop_app_running();
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Check if Codex Desktop app is currently running on the system.
pub fn is_codex_desktop_app_running() -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos::is_codex_desktop_app_running();
    }
    #[cfg(target_os = "windows")]
    {
        return windows::is_codex_desktop_app_running();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// Check if the current frontmost app is a terminal.
pub fn frontmost_is_terminal() -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos::frontmost_is_terminal();
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn restore_remembered_app_focus(state: &AppState) -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos::try_restore_previous_app_focus(state);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::try_restore_foreground_window(state);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = state;
        false
    }
}

fn deactivate_atoll_app(state: &AppState) {
    #[cfg(target_os = "macos")]
    {
        let _ = state;
        macos::deactivate_atoll_app();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = state;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = state;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalFocusFallback {
    OpenTerminal,
    ActivateClaude,
    ActivateCodex,
    DeactivateOnly,
}

fn claude_approval_focus_fallback(host: SessionHost) -> ApprovalFocusFallback {
    match host {
        SessionHost::ClaudeCli => ApprovalFocusFallback::OpenTerminal,
        SessionHost::ClaudeDesktop => ApprovalFocusFallback::ActivateClaude,
        SessionHost::Unknown => ApprovalFocusFallback::DeactivateOnly,
        _ => ApprovalFocusFallback::DeactivateOnly,
    }
}

fn codex_approval_focus_fallback(host: SessionHost) -> ApprovalFocusFallback {
    match host {
        SessionHost::CodexCli => ApprovalFocusFallback::OpenTerminal,
        SessionHost::CodexDesktop => ApprovalFocusFallback::ActivateCodex,
        SessionHost::Unknown => ApprovalFocusFallback::DeactivateOnly,
        _ => ApprovalFocusFallback::DeactivateOnly,
    }
}

pub fn restore_focus_after_approval(
    app: &AppHandle,
    state: &AppState,
    agent: Option<&str>,
    session_id: Option<&str>,
    cwd: Option<&str>,
) {
    if restore_remembered_app_focus(state) {
        return;
    }

    if agent == Some("claude") {
        if let (Some(session_id), Some(cwd)) = (session_id, cwd) {
            let host = crate::claude_session_host(state, session_id, cwd);
            match claude_approval_focus_fallback(host) {
                ApprovalFocusFallback::OpenTerminal => {
                    if open_in_terminal(cwd).is_ok() {
                        return;
                    }
                }
                ApprovalFocusFallback::ActivateClaude => {
                    if activate_claude_app(app).is_ok() {
                        return;
                    }
                }
                ApprovalFocusFallback::ActivateCodex | ApprovalFocusFallback::DeactivateOnly => {}
            }
        }
    }

    if agent == Some("codex") {
        if let (Some(session_id), Some(cwd)) = (session_id, cwd) {
            let host = crate::codex_session_host(state, session_id, cwd);
            match codex_approval_focus_fallback(host) {
                ApprovalFocusFallback::OpenTerminal => {
                    if open_in_terminal(cwd).is_ok() {
                        return;
                    }
                }
                ApprovalFocusFallback::ActivateCodex => {
                    if activate_codex_app(app).is_ok() {
                        return;
                    }
                }
                ApprovalFocusFallback::ActivateClaude | ApprovalFocusFallback::DeactivateOnly => {}
            }
        }
    }

    deactivate_atoll_app(state);
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
            SessionHost::Unknown => focus_claude_app(app),
            _ => open_in_terminal(cwd),
        };
    }

    if agent == "codex" {
        let host = session_id
            .map(|id| crate::codex_session_host(state, id, cwd))
            .unwrap_or_else(|| detect_codex_session_host(cwd));
        return match host {
            SessionHost::CodexDesktop => focus_codex_app(app),
            SessionHost::CodexCli => open_in_terminal(cwd),
            SessionHost::Unknown => focus_codex_app(app),
            _ => open_in_terminal(cwd),
        };
    }

    if agent == "cursor" {
        let host = session_id
            .map(|id| crate::cursor_session_host(state, id))
            .unwrap_or_else(|| detect_cursor_session_host());
        return match host {
            SessionHost::CursorIde => focus_cursor_app(app),
            SessionHost::Unknown => focus_cursor_app(app),
            _ => focus_cursor_app(app),
        };
    }

    open_in_terminal(cwd)
}

pub fn activate_claude_app(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::activate_claude_app(app);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        return windows::focus_claude_app();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err("activate_claude_app is not supported on this platform".to_string())
    }
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

pub fn activate_codex_app(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::activate_codex_app(app);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        return windows::focus_codex_app();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err("activate_codex_app is not supported on this platform".to_string())
    }
}

pub fn detect_cursor_session_host() -> SessionHost {
    if is_cursor_app_running() {
        return SessionHost::CursorIde;
    }
    SessionHost::Unknown
}

pub fn detect_cursor_session_host_from_peer_pid(pid: u32) -> SessionHost {
    #[cfg(target_os = "macos")]
    {
        return macos::detect_cursor_session_host_from_peer_pid(pid);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_cursor_session_host_from_peer_pid(pid);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = pid;
        SessionHost::Unknown
    }
}

pub fn is_cursor_app_running() -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos::is_cursor_app_running();
    }
    #[cfg(target_os = "windows")]
    {
        return windows::is_cursor_app_running();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

pub fn focus_cursor_app(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::focus_cursor_app(app);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        return windows::focus_cursor_app();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err("focus_cursor_app is not supported on this platform".to_string())
    }
}

pub fn focus_codex_app(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::focus_codex_app(app);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        return windows::focus_codex_app();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err("focus_codex_app is not supported on this platform".to_string())
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

const TRAY_ICON_CANVAS: u32 = 64;
const TRAY_ICON_FILL: f32 = 0.95;

pub fn tray_icon(app: &AppHandle) -> Option<tauri::image::Image<'static>> {
    let _ = app;
    enlarged_tray_icon_from_png(include_bytes!("../../icons/icon.png"))
}

/// Crop transparent padding and scale the logo to fill the menu-bar tray canvas.
fn enlarged_tray_icon_from_png(bytes: &[u8]) -> Option<tauri::image::Image<'static>> {
    use image::imageops;
    use image::RgbaImage;

    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            if pixel[0] > 16 || pixel[1] > 16 || pixel[2] > 16 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if min_x >= width || min_y >= height || max_x < min_x || max_y < min_y {
        return None;
    }

    let cropped =
        imageops::crop_imm(&img, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1).to_image();
    let (crop_w, crop_h) = cropped.dimensions();
    let target = (TRAY_ICON_CANVAS as f32 * TRAY_ICON_FILL) as u32;
    let scale = (target as f32 / crop_w as f32).min(target as f32 / crop_h as f32);
    let scaled_w = (crop_w as f32 * scale).round().max(1.0) as u32;
    let scaled_h = (crop_h as f32 * scale).round().max(1.0) as u32;
    let scaled = imageops::resize(&cropped, scaled_w, scaled_h, imageops::FilterType::Lanczos3);

    let mut canvas = RgbaImage::from_pixel(
        TRAY_ICON_CANVAS,
        TRAY_ICON_CANVAS,
        image::Rgba([0, 0, 0, 0]),
    );
    let offset_x = (TRAY_ICON_CANVAS - scaled_w) / 2;
    let offset_y = (TRAY_ICON_CANVAS - scaled_h) / 2;
    imageops::overlay(
        &mut canvas,
        &scaled,
        i64::from(offset_x),
        i64::from(offset_y),
    );

    Some(tauri::image::Image::new_owned(
        canvas.into_raw(),
        TRAY_ICON_CANVAS,
        TRAY_ICON_CANVAS,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_approval_fallback_prefers_terminal_for_cli_host() {
        assert_eq!(
            claude_approval_focus_fallback(SessionHost::ClaudeCli),
            ApprovalFocusFallback::OpenTerminal
        );
    }

    #[test]
    fn claude_approval_fallback_activates_existing_desktop_without_launch() {
        assert_eq!(
            claude_approval_focus_fallback(SessionHost::ClaudeDesktop),
            ApprovalFocusFallback::ActivateClaude
        );
    }

    #[test]
    fn claude_approval_fallback_deactivates_when_host_unknown() {
        assert_eq!(
            claude_approval_focus_fallback(SessionHost::Unknown),
            ApprovalFocusFallback::DeactivateOnly
        );
    }

    #[test]
    fn codex_approval_fallback_prefers_terminal_for_cli_host() {
        assert_eq!(
            codex_approval_focus_fallback(SessionHost::CodexCli),
            ApprovalFocusFallback::OpenTerminal
        );
    }

    #[test]
    fn codex_approval_fallback_activates_existing_desktop_without_launch() {
        assert_eq!(
            codex_approval_focus_fallback(SessionHost::CodexDesktop),
            ApprovalFocusFallback::ActivateCodex
        );
    }

    #[test]
    fn codex_approval_fallback_deactivates_when_host_unknown() {
        assert_eq!(
            codex_approval_focus_fallback(SessionHost::Unknown),
            ApprovalFocusFallback::DeactivateOnly
        );
    }
}
