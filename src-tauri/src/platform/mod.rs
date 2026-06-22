//! Platform-specific window, focus, and shell integration.

use tauri::{App, AppHandle, LogicalPosition, Monitor, PhysicalSize, WebviewWindow};

use crate::{AppState, HomeWindowBounds, NotchMetrics};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// AppKit screen frame metadata (macOS) or unused defaults elsewhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenGeometry {
    pub origin_y: f64,
    pub height: f64,
}

pub fn setup_app(app: &mut App) -> bool {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
    #[cfg(target_os = "windows")]
    {
        if !windows::ensure_single_instance() {
            return false;
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window.set_ignore_cursor_events(ignore);
    }
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

pub fn restore_focus_after_approval(state: &AppState) {
    #[cfg(target_os = "macos")]
    macos::deactivate_atoll(state);
    #[cfg(target_os = "windows")]
    windows::restore_foreground_window(state);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = state;
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
